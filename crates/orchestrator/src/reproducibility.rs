#[derive(Debug, Clone, Serialize)]
struct ReproducibilityCapture {
    tool_versions: std::collections::BTreeMap<String, String>,
    environment_variables: std::collections::BTreeMap<String, String>,
    system_dependencies: std::collections::BTreeMap<String, String>,
    container_image_digests: std::collections::BTreeMap<String, String>,
    git_submodules: std::collections::BTreeMap<String, String>,
    git_lfs_objects: Vec<String>,
    external_dependencies: std::collections::BTreeMap<String, String>,
    limitations: Vec<String>,
    level: ReproducibilityLevel,
}

impl ReproducibilityCapture {
    fn sha256(&self) -> String {
        sha256_hex(&serde_json::to_vec(self).unwrap_or_default())
    }
}

impl Orchestrator {
    async fn capture_reproducibility_environment(
        &self,
        worktree: &Path,
        config: &ProjectConfig,
    ) -> Result<ReproducibilityCapture, OrchestratorError> {
        let mut tool_versions = std::collections::BTreeMap::new();
        for (name, program, args) in [
            ("git", "git", &["--version"][..]),
            ("bun", "bun", &["--version"][..]),
            ("node", "node", &["--version"][..]),
            ("cargo", "cargo", &["--version"][..]),
            ("rustc", "rustc", &["--version"][..]),
            ("python", "python3", &["--version"][..]),
        ] {
            tool_versions.insert(name.into(), captured_version(worktree, program, args).await);
        }

        let mut limitations = Vec::new();
        let mut system_dependencies = std::collections::BTreeMap::new();
        for step in &config.validate.steps {
            let Some(program) = step.argv.first() else {
                continue;
            };
            if system_dependencies.contains_key(program) {
                continue;
            }
            let version = captured_version(worktree, program, &["--version"]).await;
            if version == "unavailable" {
                limitations.push(format!("validation dependency unavailable: {program}"));
            }
            system_dependencies.insert(program.clone(), version);
        }

        let mut environment_variables = std::collections::BTreeMap::new();
        let mut env_names = vec!["CI".to_string(), "LANG".into(), "LC_ALL".into(), "TZ".into()];
        env_names.extend(config.reproducibility.env_allowlist.iter().cloned());
        env_names.sort();
        env_names.dedup();
        for name in env_names {
            if sensitive_environment_name(&name) {
                limitations.push(format!("sensitive environment variable was not captured: {name}"));
                continue;
            }
            let value = std::env::var(&name).unwrap_or_else(|_| "<unset>".into());
            environment_variables.insert(name, sha256_hex(value.as_bytes()));
        }

        let (git_submodules, submodule_limitations) = capture_submodules(worktree).await;
        limitations.extend(submodule_limitations);
        let (git_lfs_objects, lfs_limitation) = capture_lfs(worktree).await;
        if let Some(limitation) = lfs_limitation {
            limitations.push(limitation);
        }

        let container_image_digests = config.reproducibility.container_images.clone();
        for (image, digest) in &container_image_digests {
            if !valid_sha256_digest(digest) {
                limitations.push(format!("container image is not digest-pinned: {image}"));
            }
        }
        let external_dependencies = config.reproducibility.external_dependencies.clone();
        for (name, state) in &external_dependencies {
            if state.trim().is_empty() {
                limitations.push(format!("external dependency has no immutable state: {name}"));
            }
        }
        limitations.sort();
        limitations.dedup();
        let level = if config.reproducibility.hermetic
            && config.reproducibility.lock_environment
            && !container_image_digests.is_empty()
            && limitations.is_empty()
        {
            ReproducibilityLevel::Hermetic
        } else if config.reproducibility.lock_environment && limitations.is_empty() {
            ReproducibilityLevel::EnvironmentLocked
        } else {
            ReproducibilityLevel::FixedCommit
        };
        Ok(ReproducibilityCapture {
            tool_versions,
            environment_variables,
            system_dependencies,
            container_image_digests,
            git_submodules,
            git_lfs_objects,
            external_dependencies,
            limitations,
            level,
        })
    }
}

async fn captured_version(cwd: &Path, program: &str, args: &[&str]) -> String {
    let output = tokio::time::timeout(
        Duration::from_secs(3),
        Command::new(program).args(args).current_dir(cwd).output(),
    )
    .await
    .ok()
    .and_then(Result::ok)
    .filter(|output| output.status.success());
    let Some(output) = output else {
        return "unavailable".into();
    };
    let text = if output.stdout.is_empty() {
        &output.stderr
    } else {
        &output.stdout
    };
    String::from_utf8_lossy(text).trim().to_string()
}

async fn capture_submodules(
    worktree: &Path,
) -> (
    std::collections::BTreeMap<String, String>,
    Vec<String>,
) {
    let mut modules = std::collections::BTreeMap::new();
    let mut limitations = Vec::new();
    let Ok(output) = Command::new("git")
        .args(["submodule", "status", "--recursive"])
        .current_dir(worktree)
        .output()
        .await
    else {
        return (modules, vec!["git submodule inventory unavailable".into()]);
    };
    if !output.status.success() {
        return (modules, vec!["git submodule inventory failed".into()]);
    }
    for line in String::from_utf8_lossy(&output.stdout).lines() {
        let marker = line.chars().next().unwrap_or(' ');
        let body = line.trim_start_matches([' ', '-', '+', 'U']);
        let mut parts = body.split_whitespace();
        if let (Some(sha), Some(path)) = (parts.next(), parts.next()) {
            modules.insert(path.into(), format!("{marker}{sha}"));
            if marker != ' ' {
                limitations.push(format!("submodule is not at its recorded clean commit: {path}"));
            }
        }
    }
    (modules, limitations)
}

async fn capture_lfs(worktree: &Path) -> (Vec<String>, Option<String>) {
    let output = Command::new("git")
        .args(["lfs", "ls-files", "--all", "--long"])
        .current_dir(worktree)
        .output()
        .await;
    if let Ok(output) = output
        && output.status.success()
    {
        let mut objects = String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        objects.sort();
        return (objects, None);
    }
    let uses_lfs = tokio::fs::read_to_string(worktree.join(".gitattributes"))
        .await
        .is_ok_and(|text| text.contains("filter=lfs"));
    (
        Vec::new(),
        uses_lfs.then(|| "repository uses Git LFS but git-lfs inventory is unavailable".into()),
    )
}

fn sensitive_environment_name(name: &str) -> bool {
    let upper = name.to_ascii_uppercase();
    ["KEY", "TOKEN", "SECRET", "PASSWORD", "CREDENTIAL", "AUTH"]
        .iter()
        .any(|part| upper.contains(part))
}

fn valid_sha256_digest(value: &str) -> bool {
    value
        .strip_prefix("sha256:")
        .is_some_and(|digest| digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()))
}

fn reproducibility_drift(
    manifest: &ReproducibilityManifest,
    current: &ReproducibilityCapture,
) -> Vec<String> {
    let mut drift = Vec::new();
    if manifest.tool_versions != current.tool_versions {
        drift.push("tool versions changed".into());
    }
    if manifest.environment_variables != current.environment_variables {
        drift.push("environment variables changed".into());
    }
    if manifest.system_dependencies != current.system_dependencies {
        drift.push("system dependencies changed".into());
    }
    if manifest.container_image_digests != current.container_image_digests {
        drift.push("container image digests changed".into());
    }
    if manifest.git_submodules != current.git_submodules {
        drift.push("git submodule commits changed".into());
    }
    if manifest.git_lfs_objects != current.git_lfs_objects {
        drift.push("git lfs objects changed".into());
    }
    if manifest.external_dependencies != current.external_dependencies {
        drift.push("external dependency snapshots changed".into());
    }
    drift
}

#[cfg(test)]
mod reproducibility_unit_tests {
    use super::*;

    #[test]
    fn locked_replay_detects_tool_and_external_state_drift() {
        let capture = ReproducibilityCapture {
            tool_versions: [("git".into(), "git 1".into())].into(),
            environment_variables: std::collections::BTreeMap::new(),
            system_dependencies: std::collections::BTreeMap::new(),
            container_image_digests: std::collections::BTreeMap::new(),
            git_submodules: std::collections::BTreeMap::new(),
            git_lfs_objects: Vec::new(),
            external_dependencies: [("db".into(), "snapshot-2".into())].into(),
            limitations: Vec::new(),
            level: ReproducibilityLevel::EnvironmentLocked,
        };
        let manifest = ReproducibilityManifest {
            task_id: "task".into(),
            revision: 1,
            commit_sha: "abc".into(),
            manifest_sha256: "manifest".into(),
            environment: std::collections::BTreeMap::new(),
            reproducibility_level: ReproducibilityLevel::EnvironmentLocked,
            tool_versions: [("git".into(), "git 0".into())].into(),
            environment_variables: std::collections::BTreeMap::new(),
            system_dependencies: std::collections::BTreeMap::new(),
            container_image_digests: std::collections::BTreeMap::new(),
            git_submodules: std::collections::BTreeMap::new(),
            git_lfs_objects: Vec::new(),
            external_dependencies: [("db".into(), "snapshot-1".into())].into(),
            limitations: Vec::new(),
            environment_sha256: "env".into(),
            input_sha256: "input".into(),
            patch_sha256: "patch".into(),
            validation_config_sha256: "validation".into(),
            created_at: "now".into(),
        };
        assert_eq!(
            reproducibility_drift(&manifest, &capture),
            vec!["tool versions changed", "external dependency snapshots changed"]
        );
    }
}
