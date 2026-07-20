impl Orchestrator {
    pub async fn execution_node_list(&self) -> Result<Vec<ExecutionNode>, OrchestratorError> {
        let rows = sqlx::query("SELECT id,name,host,port,username,work_root,enabled,status,platform,git_version,problem,last_checked_at FROM execution_nodes ORDER BY name")
            .fetch_all(self.store.pool()).await?;
        rows.into_iter().map(execution_node_from_row).collect()
    }

    pub async fn execution_node_upsert(
        &self,
        mut node: ExecutionNode,
    ) -> Result<ExecutionNode, OrchestratorError> {
        validate_execution_node(&node)?;
        if node.id.trim().is_empty() {
            node.id = Uuid::now_v7().to_string();
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO execution_nodes(id,name,host,port,username,work_root,enabled,status,platform,git_version,problem,last_checked_at,created_at,updated_at) VALUES(?,?,?,?,?,?,?,'unknown',NULL,NULL,NULL,NULL,?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,host=excluded.host,port=excluded.port,username=excluded.username,work_root=excluded.work_root,enabled=excluded.enabled,status='unknown',problem=NULL,updated_at=excluded.updated_at")
            .bind(&node.id).bind(node.name.trim()).bind(node.host.trim()).bind(i64::from(node.port))
            .bind(node.username.trim()).bind(node.work_root.trim()).bind(i64::from(node.enabled))
            .bind(&now).bind(&now).execute(self.store.pool()).await?;
        self.execution_node_get(&node.id).await
    }

    pub async fn execution_node_delete(&self, node_id: &str) -> Result<(), OrchestratorError> {
        let referenced: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM task_policies WHERE execution_node_id=?",
        )
        .bind(node_id).fetch_one(self.store.pool()).await?;
        if referenced > 0 {
            return Err(OrchestratorError::InvalidState(
                "execution node is referenced by existing tasks; disable it instead".into(),
            ));
        }
        sqlx::query("DELETE FROM execution_nodes WHERE id=?")
            .bind(node_id).execute(self.store.pool()).await?;
        Ok(())
    }

    pub async fn execution_node_check(
        &self,
        node_id: &str,
    ) -> Result<ExecutionNode, OrchestratorError> {
        let node = self.execution_node_get(node_id).await?;
        let destination = format!("{}@{}", node.username, node.host);
        let root = shell_quote(&node.work_root);
        let remote = format!(
            "set -eu; {}; mkdir -p {root}; test -w {root}; command -v git >/dev/null; command -v tar >/dev/null; uname -srm; git --version",
            remote_environment_prelude(),
        );
        let result = tokio::time::timeout(
            Duration::from_secs(12),
            Command::new("ssh")
                .args(ssh_base_args(&node))
                .arg(destination)
                .arg(remote)
                .output(),
        )
        .await;
        let checked = Utc::now().to_rfc3339();
        let (status, platform, git_version, problem) = match result {
            Ok(Ok(output)) if output.status.success() => {
                let text = String::from_utf8_lossy(&output.stdout);
                let mut lines = text.lines();
                (
                    NodeStatus::Online,
                    lines.next().map(str::to_string),
                    lines.next().map(str::to_string),
                    None,
                )
            }
            Ok(Ok(output)) => (
                NodeStatus::Offline,
                None,
                None,
                Some(agentflow_process_supervisor::redact(
                    String::from_utf8_lossy(&output.stderr).chars().take(1000).collect(),
                )),
            ),
            Ok(Err(error)) => (NodeStatus::Offline, None, None, Some(error.to_string())),
            Err(_) => (NodeStatus::Offline, None, None, Some("SSH health check timed out".into())),
        };
        sqlx::query("UPDATE execution_nodes SET status=?,platform=?,git_version=?,problem=?,last_checked_at=?,updated_at=? WHERE id=?")
            .bind(status.to_string()).bind(platform).bind(git_version).bind(problem)
            .bind(&checked).bind(&checked).bind(node_id).execute(self.store.pool()).await?;
        self.execution_node_get(node_id).await
    }

    async fn execution_node_get(&self, node_id: &str) -> Result<ExecutionNode, OrchestratorError> {
        let row = sqlx::query("SELECT id,name,host,port,username,work_root,enabled,status,platform,git_version,problem,last_checked_at FROM execution_nodes WHERE id=?")
            .bind(node_id).fetch_one(self.store.pool()).await?;
        execution_node_from_row(row)
    }

    async fn execute_validation(
        &self,
        task: &TaskRow,
        worktree: &Path,
        steps: &[ValidateStep],
    ) -> Result<TestReport, OrchestratorError> {
        if let Some(node_id) = task.policy.execution_node_id.as_deref() {
            let node = self.execution_node_check(node_id).await?;
            if !node.enabled || node.status != NodeStatus::Online {
                return Err(OrchestratorError::RemoteNodeUnavailable(
                    node.problem.unwrap_or_else(|| "REMOTE_NODE_UNAVAILABLE".into()),
                ));
            }
            self.execute_remote_validation(task, &node, steps).await
        } else {
            execute_local_validation(worktree, steps, self.remaining_time_budget(&task.id).await?).await
        }
    }

    async fn execute_remote_validation(
        &self,
        task: &TaskRow,
        node: &ExecutionNode,
        steps: &[ValidateStep],
    ) -> Result<TestReport, OrchestratorError> {
        if steps.iter().any(|step| step.argv.is_empty()) {
            return Err(OrchestratorError::ValidationInfra("empty validation argv".into()));
        }
        let project = self.project(&task.project_id).await?;
        let sha: String = sqlx::query_scalar(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(&task.id).bind(task.revision).fetch_one(self.store.pool()).await?;
        let archive = self.git.archive(&project.repo, &sha).await?;
        let remote_dir = format!(
            "{}/agentflow/{}/r{}-{}",
            node.work_root.trim_end_matches('/'),
            task.id,
            task.revision,
            &sha[..sha.len().min(12)]
        );
        if let Err(error) = upload_remote_archive(node, &remote_dir, &archive).await {
            // A failed stream can still leave a partially extracted directory.
            // Best-effort cleanup keeps repeated runs deterministic.
            let _ = cleanup_remote_dir(node, &remote_dir).await;
            return Err(error);
        }
        let destination = format!("{}@{}", node.username, node.host);
        let mut report = TestReport { schema_version: 1, passed: true, steps: Vec::new() };
        let mut remaining = self.remaining_time_budget(&task.id).await?;
        let mut transport_error = None;
        for step in steps {
            let allowed = remaining.map_or(step.timeout_secs, |value| value.min(step.timeout_secs)).max(1);
            let command = remote_validation_command(&remote_dir, &step.argv);
            let started = Instant::now();
            let output = tokio::time::timeout(
                Duration::from_secs(allowed),
                Command::new("ssh").args(ssh_base_args(node)).arg(&destination).arg(command).output(),
            ).await;
            let elapsed = started.elapsed();
            remaining = remaining.map(|value| value.saturating_sub(elapsed.as_secs()));
            match output {
                Ok(Ok(output)) => {
                    let passed = output.status.success();
                    report.passed &= passed;
                    report.steps.push(TestStepReport {
                        name: step.name.clone(), argv: step.argv.clone(), exit_code: output.status.code(),
                        duration_ms: elapsed.as_millis() as u64, stdout_tail: tail(&output.stdout), stderr_tail: tail(&output.stderr),
                    });
                }
                Ok(Err(error)) => {
                    transport_error = Some(OrchestratorError::RemoteNodeUnavailable(error.to_string()));
                    break;
                }
                Err(_) => {
                    report.passed = false;
                    report.steps.push(TestStepReport {
                        name: step.name.clone(), argv: step.argv.clone(), exit_code: None,
                        duration_ms: elapsed.as_millis() as u64, stdout_tail: String::new(), stderr_tail: "timed out".into(),
                    });
                }
            }
        }
        // Reports and hashes are persisted locally; the remote checkout is an
        // ephemeral fixed-commit sandbox and is removed after the run.
        let _ = cleanup_remote_dir(node, &remote_dir).await;
        transport_error.map_or(Ok(report), Err)
    }
}

async fn execute_local_validation(
    worktree: &Path,
    steps: &[ValidateStep],
    mut remaining: Option<u64>,
) -> Result<TestReport, OrchestratorError> {
    let mut report = TestReport { schema_version: 1, passed: true, steps: Vec::new() };
    for step in steps {
        if step.argv.is_empty() {
            return Err(OrchestratorError::ValidationInfra("empty validation argv".into()));
        }
        let allowed = remaining.map_or(step.timeout_secs, |value| value.min(step.timeout_secs)).max(1);
        let started = Instant::now();
        let output = tokio::time::timeout(
            Duration::from_secs(allowed),
            Command::new(&step.argv[0]).args(&step.argv[1..]).current_dir(worktree).output(),
        ).await;
        let elapsed = started.elapsed();
        remaining = remaining.map(|value| value.saturating_sub(elapsed.as_secs()));
        match output {
            Ok(Ok(output)) => {
                let passed = output.status.success();
                report.passed &= passed;
                report.steps.push(TestStepReport {
                    name: step.name.clone(), argv: step.argv.clone(), exit_code: output.status.code(),
                    duration_ms: elapsed.as_millis() as u64, stdout_tail: tail(&output.stdout), stderr_tail: tail(&output.stderr),
                });
            }
            Ok(Err(error)) => return Err(OrchestratorError::ValidationInfra(error.to_string())),
            Err(_) => {
                report.passed = false;
                report.steps.push(TestStepReport {
                    name: step.name.clone(), argv: step.argv.clone(), exit_code: None,
                    duration_ms: elapsed.as_millis() as u64, stdout_tail: String::new(), stderr_tail: "timed out".into(),
                });
            }
        }
    }
    Ok(report)
}

async fn upload_remote_archive(
    node: &ExecutionNode,
    remote_dir: &str,
    archive: &[u8],
) -> Result<(), OrchestratorError> {
    let destination = format!("{}@{}", node.username, node.host);
    let command = format!(
        "mkdir -p {} && tar -xf - -C {}",
        shell_quote(remote_dir), shell_quote(remote_dir)
    );
    let mut child = Command::new("ssh")
        .args(ssh_base_args(node)).arg(destination).arg(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|error| OrchestratorError::RemoteNodeUnavailable(error.to_string()))?;
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(archive).await?;
    }
    let output = tokio::time::timeout(Duration::from_secs(120), child.wait_with_output())
        .await
        .map_err(|_| OrchestratorError::RemoteNodeUnavailable("remote upload timed out".into()))?
        .map_err(|error| OrchestratorError::RemoteNodeUnavailable(error.to_string()))?;
    if !output.status.success() {
        return Err(OrchestratorError::RemoteNodeUnavailable(
            agentflow_process_supervisor::redact(
                String::from_utf8_lossy(&output.stderr).chars().take(1000).collect(),
            ),
        ));
    }
    Ok(())
}

fn ssh_base_args(node: &ExecutionNode) -> Vec<String> {
    vec![
        "-o".into(), "BatchMode=yes".into(),
        "-o".into(), "ConnectTimeout=8".into(),
        "-p".into(), node.port.to_string(),
    ]
}

/// SSH executes commands through a non-interactive shell, so user toolchains
/// installed by rustup or Bun are commonly absent from PATH even when they work
/// in an interactive terminal. Keep the prelude fixed and do not source shell
/// profiles: profiles are mutable code and would make validation less reproducible.
fn remote_environment_prelude() -> &'static str {
    "export PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:$PATH\"; export CI=1"
}

fn remote_validation_command(remote_dir: &str, argv: &[String]) -> String {
    format!(
        "set -eu; {}; cd {} && {}",
        remote_environment_prelude(),
        shell_quote(remote_dir),
        argv.iter().map(|value| shell_quote(value)).collect::<Vec<_>>().join(" "),
    )
}

fn validate_execution_node(node: &ExecutionNode) -> Result<(), OrchestratorError> {
    let safe = |value: &str| !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':'));
    let root = Path::new(node.work_root.trim());
    let safe_root = root.is_absolute()
        && root != Path::new("/")
        && root.components().all(|component| {
            matches!(component, std::path::Component::RootDir | std::path::Component::Normal(_))
        });
    if node.name.trim().is_empty() || !safe(node.host.trim()) || !safe(node.username.trim())
        || node.port == 0 || !safe_root || node.work_root.len() > 512
        || node.work_root.contains(['\n', '\r', '\0']) {
        return Err(OrchestratorError::Config("invalid execution node fields".into()));
    }
    Ok(())
}

async fn cleanup_remote_dir(node: &ExecutionNode, remote_dir: &str) -> Result<(), OrchestratorError> {
    let destination = format!("{}@{}", node.username, node.host);
    let output = tokio::time::timeout(
        Duration::from_secs(20),
        Command::new("ssh")
            .args(ssh_base_args(node))
            .arg(destination)
            .arg(format!("rm -rf -- {}", shell_quote(remote_dir)))
            .output(),
    )
    .await
    .map_err(|_| OrchestratorError::RemoteNodeUnavailable("remote cleanup timed out".into()))?
    .map_err(|error| OrchestratorError::RemoteNodeUnavailable(error.to_string()))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(OrchestratorError::RemoteNodeUnavailable(
            agentflow_process_supervisor::redact(
                String::from_utf8_lossy(&output.stderr).chars().take(1000).collect(),
            ),
        ))
    }
}

fn execution_node_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ExecutionNode, OrchestratorError> {
    Ok(ExecutionNode {
        id: row.get("id"), name: row.get("name"), host: row.get("host"),
        port: u16::try_from(row.get::<i64, _>("port")).map_err(|_|OrchestratorError::Config("invalid SSH port".into()))?,
        username: row.get("username"), work_root: row.get("work_root"),
        enabled: row.get::<i64, _>("enabled") != 0, status: parse(row.get("status"))?,
        platform: row.get("platform"), git_version: row.get("git_version"),
        problem: row.get("problem"), last_checked_at: row.get("last_checked_at"),
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod execution_node_tests {
    use super::*;

    #[test]
    fn ssh_fields_and_shell_quoting_are_bounded() {
        let node = ExecutionNode {
            id: String::new(), name: "builder".into(), host: "10.0.0.8".into(), port: 22,
            username: "runner".into(), work_root: "/srv/agent flow".into(), enabled: true,
            status: NodeStatus::Unknown, platform: None, git_version: None, problem: None,
            last_checked_at: None,
        };
        assert!(validate_execution_node(&node).is_ok());
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
        let command = remote_validation_command(
            "/srv/agent flow",
            &["bun".into(), "test; touch /tmp/escaped".into()],
        );
        assert!(command.contains("$HOME/.bun/bin"));
        assert!(command.contains("'test; touch /tmp/escaped'"));
        let mut unsafe_node = node;
        unsafe_node.host = "host;touch".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
        unsafe_node.host = "runner.local".into();
        unsafe_node.work_root = "/".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
        unsafe_node.work_root = "/srv/../root".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
    }

    /// Opt-in smoke test for a real Unix SSH node. It exercises the same archive
    /// upload, non-interactive PATH, validation command, and cleanup functions as
    /// a production remote run without embedding a developer-specific host.
    #[tokio::test]
    #[ignore = "requires AGENTFLOW_TEST_SSH_HOST, USER and ROOT"]
    async fn live_remote_archive_validation_and_cleanup()
    -> Result<(), Box<dyn std::error::Error>> {
        let host = std::env::var("AGENTFLOW_TEST_SSH_HOST")?;
        let username = std::env::var("AGENTFLOW_TEST_SSH_USER")?;
        let work_root = std::env::var("AGENTFLOW_TEST_SSH_ROOT")?;
        let port = std::env::var("AGENTFLOW_TEST_SSH_PORT")
            .unwrap_or_else(|_| "22".into())
            .parse()?;
        let node = ExecutionNode {
            id: "live-test".into(),
            name: "live test node".into(),
            host,
            port,
            username,
            work_root: work_root.clone(),
            enabled: true,
            status: NodeStatus::Unknown,
            platform: None,
            git_version: None,
            problem: None,
            last_checked_at: None,
        };
        validate_execution_node(&node)?;

        let fixture = tempfile::tempdir()?;
        let repo = fixture.path();
        for args in [
            vec!["init", "-q", "-b", "main"],
            vec!["config", "user.email", "test@agentflow.local"],
            vec!["config", "user.name", "AgentFlow Test"],
        ] {
            let output = Command::new("git").args(args).current_dir(repo).output().await?;
            if !output.status.success() {
                return Err(String::from_utf8_lossy(&output.stderr).into_owned().into());
            }
        }
        tokio::fs::write(repo.join("probe.txt"), "fixed revision\n").await?;
        let commit = Command::new("git")
            .args(["add", "probe.txt"])
            .current_dir(repo)
            .output()
            .await?;
        assert!(commit.status.success());
        let commit = Command::new("git")
            .args(["commit", "-q", "-m", "fixed probe"])
            .current_dir(repo)
            .output()
            .await?;
        assert!(commit.status.success());
        let archive = Command::new("git")
            .args(["archive", "--format=tar", "HEAD"])
            .current_dir(repo)
            .output()
            .await?;
        assert!(archive.status.success());

        let remote_dir = format!("{work_root}/agentflow/live-probe-{}", Uuid::now_v7());
        upload_remote_archive(&node, &remote_dir, &archive.stdout).await?;
        let destination = format!("{}@{}", node.username, node.host);
        let argv = vec![
            "/bin/sh".into(),
            "-c".into(),
            "test -f probe.txt && bun --version && cargo --version".into(),
        ];
        let result = Command::new("ssh")
            .args(ssh_base_args(&node))
            .arg(&destination)
            .arg(remote_validation_command(&remote_dir, &argv))
            .output()
            .await?;
        cleanup_remote_dir(&node, &remote_dir).await?;
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(output.lines().count() >= 2, "{output}");

        let removed = Command::new("ssh")
            .args(ssh_base_args(&node))
            .arg(destination)
            .arg(format!("test ! -e {}", shell_quote(&remote_dir)))
            .output()
            .await?;
        assert!(removed.status.success());
        Ok(())
    }
}
