#[derive(Debug, Serialize, Deserialize)]
struct IntegrityGuardReport {
    schema_version: u8,
    baseline_commit: String,
    candidate_commit: String,
    baseline_test_files: usize,
    candidate_test_files: usize,
    baseline_test_markers: usize,
    candidate_test_markers: usize,
    removed_test_files: Vec<String>,
    sensitive_paths: Vec<String>,
    hard_violations: Vec<String>,
    requires_security_review: bool,
    baseline_validation: Option<TestReport>,
    limitations: Vec<String>,
}

impl Orchestrator {
    async fn integrity_guard(
        &self,
        task: &TaskRow,
        config: &ProjectConfig,
    ) -> Result<IntegrityGuardReport, OrchestratorError> {
        let project = self.project(&task.project_id).await?;
        let worktree = required_path(&task.worktree_path)?;
        let baseline = task
            .base_commit
            .clone()
            .ok_or_else(|| OrchestratorError::InvalidState("base commit missing".into()))?;
        let candidate = self.revision_commit_sha(&task.id, task.revision).await?;
        let baseline_files = git_lines(&worktree, &["ls-tree", "-r", "--name-only", &baseline]).await?;
        let candidate_files =
            git_lines(&worktree, &["ls-tree", "-r", "--name-only", &candidate]).await?;
        let baseline_tests = baseline_files
            .iter()
            .filter(|path| is_test_path(path))
            .cloned()
            .collect::<HashSet<_>>();
        let candidate_tests = candidate_files
            .iter()
            .filter(|path| is_test_path(path))
            .cloned()
            .collect::<HashSet<_>>();
        let mut removed_test_files = baseline_tests
            .difference(&candidate_tests)
            .cloned()
            .collect::<Vec<_>>();
        removed_test_files.sort();
        let baseline_markers = test_marker_count(&worktree, &baseline, &baseline_tests).await;
        let candidate_markers = test_marker_count(&worktree, &candidate, &candidate_tests).await;
        let changes = git_lines(
            &worktree,
            &["diff", "--name-status", &baseline, &candidate],
        )
        .await?;
        let mut sensitive_paths = changes
            .iter()
            .filter_map(|line| line.split_whitespace().last())
            .filter(|path| is_security_sensitive(path))
            .map(str::to_string)
            .collect::<Vec<_>>();
        sensitive_paths.sort();
        sensitive_paths.dedup();

        let mut hard_violations = Vec::new();
        if !removed_test_files.is_empty() {
            hard_violations.push(format!(
                "删除了 {} 个基线测试文件：{}",
                removed_test_files.len(),
                removed_test_files.join("、")
            ));
        }
        if candidate_markers < baseline_markers {
            hard_violations.push(format!(
                "可识别测试用例从 {baseline_markers} 减少到 {candidate_markers}"
            ));
        }
        let deleted_control = changes
            .iter()
            .filter(|line| line.starts_with("D\t"))
            .filter_map(|line| line.split_once('\t').map(|(_, path)| path))
            .filter(|path| is_quality_control_path(path))
            .collect::<Vec<_>>();
        if !deleted_control.is_empty() {
            hard_violations.push(format!(
                "删除了质量/CI 控制文件：{}",
                deleted_control.join("、")
            ));
        }
        let baseline_controls =
            quality_control_strength(&worktree, &baseline, &baseline_files).await;
        let candidate_controls =
            quality_control_strength(&worktree, &candidate, &candidate_files).await;
        if candidate_controls < baseline_controls {
            hard_violations.push(format!(
                "测试、lint、类型检查或覆盖率控制项从 {baseline_controls} 减少到 {candidate_controls}"
            ));
        }

        let requires_security_review = !sensitive_paths.is_empty();
        let mut limitations = Vec::new();
        let baseline_validation = if requires_security_review
            && !config.validate.steps.is_empty()
            && task.policy.execution_node_id.is_none()
        {
            match self
                .run_baseline_validation(task, &project, &baseline, &config.validate.steps)
                .await
            {
                Ok(report) => Some(report),
                Err(error) => {
                    limitations.push(format!("基线验证不可用：{error}"));
                    None
                }
            }
        } else {
            if requires_security_review && task.policy.execution_node_id.is_some() {
                limitations.push("远程节点模式暂不重复运行本地基线；仍执行文件与测试清单比较".into());
            }
            None
        };
        let report = IntegrityGuardReport {
            schema_version: 1,
            baseline_commit: baseline,
            candidate_commit: candidate,
            baseline_test_files: baseline_tests.len(),
            candidate_test_files: candidate_tests.len(),
            baseline_test_markers: baseline_markers,
            candidate_test_markers: candidate_markers,
            removed_test_files,
            sensitive_paths,
            hard_violations,
            requires_security_review,
            baseline_validation,
            limitations,
        };
        let path = self
            .task_dir(&task.id)
            .join("artifacts")
            .join(format!("r{}-integrity.json", task.revision));
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        tokio::fs::write(
            path,
            serde_json::to_vec_pretty(&report)
                .map_err(|error| OrchestratorError::Config(error.to_string()))?,
        )
        .await?;
        Ok(report)
    }

    async fn run_baseline_validation(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        baseline: &str,
        steps: &[ValidateStep],
    ) -> Result<TestReport, OrchestratorError> {
        let root = self.app_data.join("cache/baselines");
        tokio::fs::create_dir_all(&root).await?;
        let path = root.join(format!("{}-r{}-{}", task.id, task.revision, Uuid::now_v7()));
        let output = Command::new("git")
            .args(["worktree", "add", "--detach"])
            .arg(&path)
            .arg(baseline)
            .current_dir(&project.repo)
            .output()
            .await?;
        if !output.status.success() {
            return Err(OrchestratorError::ValidationInfra(
                String::from_utf8_lossy(&output.stderr).trim().into(),
            ));
        }
        let report = execute_local_validation(&path, steps, self.remaining_time_budget(&task.id).await?).await;
        let cleanup = Command::new("git")
            .args(["worktree", "remove", "--force"])
            .arg(&path)
            .current_dir(&project.repo)
            .output()
            .await;
        if !matches!(cleanup.as_ref(), Ok(value) if value.status.success()) {
            return Err(OrchestratorError::ValidationInfra(
                "failed to remove baseline worktree".into(),
            ));
        }
        report
    }

    async fn stored_integrity_report(
        &self,
        task: &TaskRow,
    ) -> Result<Option<IntegrityGuardReport>, OrchestratorError> {
        let path = self
            .task_dir(&task.id)
            .join("artifacts")
            .join(format!("r{}-integrity.json", task.revision));
        match tokio::fs::read(path).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map(Some)
                .map_err(|error| OrchestratorError::Config(error.to_string())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

async fn git_lines(repo: &Path, args: &[&str]) -> Result<Vec<String>, OrchestratorError> {
    let output = Command::new("git")
        .args(args)
        .current_dir(repo)
        .env("LC_ALL", "C")
        .output()
        .await?;
    if !output.status.success() {
        return Err(OrchestratorError::Git(GitError::Failed {
            operation: args.join(" "),
            stderr: String::from_utf8_lossy(&output.stderr).trim().into(),
        }));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(str::to_string)
        .collect())
}

async fn git_file(repo: &Path, commit: &str, path: &str) -> Option<String> {
    let output = Command::new("git")
        .args(["show", &format!("{commit}:{path}")])
        .current_dir(repo)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).into_owned())
}

fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    lower.starts_with("test/")
        || lower.starts_with("tests/")
        || lower.starts_with("__tests__/")
        || lower.contains("/test/")
        || lower.contains("/tests/")
        || lower.contains("/__tests__/")
        || lower.ends_with("_test.rs")
        || lower.ends_with("_test.py")
        || lower.ends_with(".test.ts")
        || lower.ends_with(".test.tsx")
        || lower.ends_with(".spec.ts")
        || lower.ends_with(".spec.tsx")
        || lower.ends_with("test.java")
        || lower.ends_with("tests.rs")
}

fn is_quality_control_path(path: &str) -> bool {
    path.starts_with(".github/workflows/")
        || path == ".gitlab-ci.yml"
        || path == "package.json"
        || path == "Cargo.toml"
        || path == "pyproject.toml"
        || path == ".agentflow/project.toml"
}

fn is_security_sensitive(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    is_test_path(path)
        || is_quality_control_path(path)
        || lower.contains("auth")
        || lower.contains("permission")
        || lower.contains("security")
        || lower.contains("secret")
        || lower.contains("credential")
        || lower.contains("migration")
}

async fn test_marker_count(repo: &Path, commit: &str, files: &HashSet<String>) -> usize {
    let mut total = 0;
    for path in files {
        if let Some(text) = git_file(repo, commit, path).await {
            total += marker_count(&text);
        }
    }
    total
}

async fn quality_control_strength(repo: &Path, commit: &str, files: &[String]) -> usize {
    let controls = files
        .iter()
        .filter(|path| is_quality_control_path(path))
        .collect::<Vec<_>>();
    let mut total = 0;
    for path in controls {
        if let Some(text) = git_file(repo, commit, path).await {
            let lower = text.to_ascii_lowercase();
            total += ["test", "lint", "coverage", "typecheck", "tsc", "pytest"]
                .into_iter()
                .filter(|token| lower.contains(token))
                .count();
        }
    }
    total
}

fn marker_count(text: &str) -> usize {
    text.lines()
        .filter(|line| {
            let value = line.trim();
            value.starts_with("#[test]")
                || value.starts_with("def test_")
                || value.starts_with("@Test")
                || value.starts_with("func Test")
                || value.contains("test(")
                || value.contains("it(")
        })
        .count()
}
