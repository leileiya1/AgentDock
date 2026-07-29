impl Orchestrator {
    pub async fn execution_node_list(&self) -> Result<Vec<ExecutionNode>, OrchestratorError> {
        let rows = sqlx::query("SELECT id,name,host,port,username,work_root,identity_file,deny_network,enabled,status,platform,git_version,problem,last_checked_at,diagnostics_json FROM execution_nodes ORDER BY name")
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
        sqlx::query("INSERT INTO execution_nodes(id,name,host,port,username,work_root,identity_file,deny_network,enabled,status,platform,git_version,problem,last_checked_at,diagnostics_json,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,'unknown',NULL,NULL,NULL,NULL,'[]',?,?) ON CONFLICT(id) DO UPDATE SET name=excluded.name,host=excluded.host,port=excluded.port,username=excluded.username,work_root=excluded.work_root,identity_file=excluded.identity_file,deny_network=excluded.deny_network,enabled=excluded.enabled,status='unknown',problem=NULL,diagnostics_json='[]',updated_at=excluded.updated_at")
            .bind(&node.id).bind(node.name.trim()).bind(node.host.trim()).bind(i64::from(node.port))
            .bind(node.username.trim()).bind(node.work_root.trim()).bind(node.identity_file.as_deref())
            .bind(i64::from(node.deny_network)).bind(i64::from(node.enabled))
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
        let probe = probe_execution_node(&node).await;
        let checked = Utc::now().to_rfc3339();
        let diagnostics_json = serde_json::to_string(&probe.diagnostics)
            .map_err(|error| OrchestratorError::Config(error.to_string()))?;
        sqlx::query("UPDATE execution_nodes SET status=?,platform=?,git_version=?,problem=?,last_checked_at=?,diagnostics_json=?,updated_at=? WHERE id=?")
            .bind(probe.status.to_string()).bind(probe.platform).bind(probe.git_version).bind(probe.problem)
            .bind(&checked).bind(diagnostics_json)
            .bind(&checked).bind(node_id).execute(self.store.pool()).await?;
        self.execution_node_get(node_id).await
    }

    async fn execution_node_get(&self, node_id: &str) -> Result<ExecutionNode, OrchestratorError> {
        let row = sqlx::query("SELECT id,name,host,port,username,work_root,identity_file,deny_network,enabled,status,platform,git_version,problem,last_checked_at,diagnostics_json FROM execution_nodes WHERE id=?")
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
            let command = remote_validation_command(node, &remote_dir, &step.argv, allowed);
            let started = Instant::now();
            let mut ssh = Command::new("ssh");
            ssh.args(ssh_base_args(node)).arg(&destination).arg(command);
            let output = output_with_deadline(ssh, Duration::from_secs(allowed)).await;
            let elapsed = started.elapsed();
            remaining = remaining.map(|value| value.saturating_sub(elapsed.as_secs()));
            match output {
                Ok(Some(output)) => {
                    let passed = output.status.success();
                    report.passed &= passed;
                    report.steps.push(TestStepReport {
                        name: step.name.clone(), argv: step.argv.clone(), exit_code: output.status.code(),
                        duration_ms: elapsed.as_millis() as u64, stdout_tail: tail(&output.stdout), stderr_tail: tail(&output.stderr),
                    });
                }
                Err(error) => {
                    transport_error = Some(OrchestratorError::RemoteNodeUnavailable(error.to_string()));
                    break;
                }
                Ok(None) => {
                    report.passed = false;
                    report.steps.push(TestStepReport {
                        name: step.name.clone(), argv: step.argv.clone(), exit_code: None,
                        duration_ms: elapsed.as_millis() as u64, stdout_tail: String::new(), stderr_tail: "timed out (local ssh terminated; remote watchdog reaps the step)".into(),
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

/// Run one command with a hard deadline. The child gets its own process group, and on timeout
/// the whole group is terminated before returning `None` — a expired validation step must never
/// keep mutating the worktree (or racing remote cleanup) behind the scheduler's back.
async fn output_with_deadline(
    mut command: Command,
    allowed: Duration,
) -> Result<Option<std::process::Output>, std::io::Error> {
    use tokio::io::AsyncReadExt;
    #[cfg(unix)]
    command.process_group(0);
    command
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn()?;
    let pid = child.id().unwrap_or(0);
    let mut stdout_pipe = child.stdout.take();
    let mut stderr_pipe = child.stderr.take();
    // Both pipes are drained concurrently with the wait: a step that fills one pipe while the
    // other is idle must not deadlock, and a timeout must not lose already-produced output.
    let drain = tokio::spawn(async move {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        tokio::join!(
            async {
                if let Some(pipe) = stdout_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut stdout).await;
                }
            },
            async {
                if let Some(pipe) = stderr_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut stderr).await;
                }
            }
        );
        (stdout, stderr)
    });
    match tokio::time::timeout(allowed, child.wait()).await {
        Ok(status) => {
            let status = status?;
            let (stdout, stderr) = drain.await.unwrap_or_default();
            Ok(Some(std::process::Output { status, stdout, stderr }))
        }
        Err(_) => {
            let _ = agentflow_process_supervisor::terminate_spawned_group(&mut child, pid).await;
            let _ = child.wait().await;
            drain.abort();
            Ok(None)
        }
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
        let mut command = Command::new(&step.argv[0]);
        command.args(&step.argv[1..]).current_dir(worktree);
        let output = output_with_deadline(command, Duration::from_secs(allowed)).await;
        let elapsed = started.elapsed();
        remaining = remaining.map(|value| value.saturating_sub(elapsed.as_secs()));
        match output {
            Ok(Some(output)) => {
                let passed = output.status.success();
                report.passed &= passed;
                report.steps.push(TestStepReport {
                    name: step.name.clone(), argv: step.argv.clone(), exit_code: output.status.code(),
                    duration_ms: elapsed.as_millis() as u64, stdout_tail: tail(&output.stdout), stderr_tail: tail(&output.stderr),
                });
            }
            Err(error) => return Err(OrchestratorError::ValidationInfra(error.to_string())),
            Ok(None) => {
                report.passed = false;
                report.steps.push(TestStepReport {
                    name: step.name.clone(), argv: step.argv.clone(), exit_code: None,
                    duration_ms: elapsed.as_millis() as u64, stdout_tail: String::new(), stderr_tail: "timed out (process group terminated)".into(),
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
    let mut ssh = Command::new("ssh");
    ssh.args(ssh_base_args(node)).arg(destination).arg(command)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    #[cfg(unix)]
    ssh.process_group(0);
    let mut child = ssh
        .spawn()
        .map_err(|error| OrchestratorError::RemoteNodeUnavailable(error.to_string()))?;
    let pid = child.id().unwrap_or(0);
    let mut stdin = child.stdin.take();
    let mut stderr_pipe = child.stderr.take();
    let archive = archive.to_vec();
    // Streaming the archive and draining stderr run beside the wait so a timeout can still
    // terminate the ssh process group instead of leaking it mid-upload.
    let io_task = tokio::spawn(async move {
        use tokio::io::AsyncReadExt;
        let mut stderr = Vec::new();
        tokio::join!(
            async {
                if let Some(mut stdin) = stdin.take() {
                    let _ = stdin.write_all(&archive).await;
                    let _ = stdin.shutdown().await;
                }
            },
            async {
                if let Some(pipe) = stderr_pipe.as_mut() {
                    let _ = pipe.read_to_end(&mut stderr).await;
                }
            }
        );
        stderr
    });
    match tokio::time::timeout(Duration::from_secs(120), child.wait()).await {
        Ok(Ok(status)) => {
            let stderr = io_task.await.unwrap_or_default();
            if status.success() {
                Ok(())
            } else {
                Err(OrchestratorError::RemoteNodeUnavailable(
                    agentflow_process_supervisor::redact(
                        String::from_utf8_lossy(&stderr).chars().take(1000).collect(),
                    ),
                ))
            }
        }
        Ok(Err(error)) => {
            io_task.abort();
            Err(OrchestratorError::RemoteNodeUnavailable(error.to_string()))
        }
        Err(_) => {
            let _ = agentflow_process_supervisor::terminate_spawned_group(&mut child, pid).await;
            let _ = child.wait().await;
            io_task.abort();
            Err(OrchestratorError::RemoteNodeUnavailable("remote upload timed out".into()))
        }
    }
}

fn ssh_base_args(node: &ExecutionNode) -> Vec<String> {
    let mut args = vec![
        "-o".into(), "BatchMode=yes".into(),
        "-o".into(), "ConnectTimeout=8".into(),
        "-p".into(), node.port.to_string(),
    ];
    if let Some(identity_file) = node.identity_file.as_deref() {
        args.extend([
            "-o".into(),
            "IdentitiesOnly=yes".into(),
            "-i".into(),
            identity_file.into(),
        ]);
    }
    args
}

/// SSH executes commands through a non-interactive shell, so user toolchains
/// installed by rustup or Bun are commonly absent from PATH even when they work
/// in an interactive terminal. Keep the prelude fixed and do not source shell
/// profiles: profiles are mutable code and would make validation less reproducible.
fn remote_environment_prelude() -> &'static str {
    "export PATH=\"$HOME/.local/bin:$HOME/.cargo/bin:$HOME/.bun/bin:$PATH\"; export CI=1"
}

fn remote_validation_command(
    node: &ExecutionNode,
    remote_dir: &str,
    argv: &[String],
    allowed_secs: u64,
) -> String {
    // The detached watchdog kills the whole remote session group two seconds after the local
    // deadline: killing the local ssh alone would leave the remote step running and racing the
    // rm -rf cleanup. Its file descriptors are detached so sshd can close the session as soon
    // as the main shell exits on the normal path.
    let command = argv
        .iter()
        .map(|value| shell_quote(value))
        .collect::<Vec<_>>()
        .join(" ");
    let command = if node.deny_network {
        format!("sudo -n /usr/local/sbin/agentflow-offline -- {command}")
    } else {
        command
    };
    format!(
        "( sleep {}; kill -KILL -- -$$ ) >/dev/null 2>&1 </dev/null & set -eu; {}; cd {} && {}",
        allowed_secs.saturating_add(2),
        remote_environment_prelude(),
        shell_quote(remote_dir),
        command,
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
    if let Some(identity_file) = node.identity_file.as_deref() {
        let identity = Path::new(identity_file);
        if identity_file.len() > 512
            || identity_file.contains(['\n', '\r', '\0'])
            || !identity.is_absolute()
            || !identity.is_file()
        {
            return Err(OrchestratorError::Config(
                "SSH identity file must be an existing absolute file".into(),
            ));
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if identity.metadata()?.permissions().mode() & 0o077 != 0 {
                return Err(OrchestratorError::Config(
                    "SSH identity file must not be readable by group or other users".into(),
                ));
            }
        }
    }
    Ok(())
}

async fn cleanup_remote_dir(node: &ExecutionNode, remote_dir: &str) -> Result<(), OrchestratorError> {
    let destination = format!("{}@{}", node.username, node.host);
    let mut ssh = Command::new("ssh");
    ssh.args(ssh_base_args(node))
        .arg(destination)
        .arg(format!("rm -rf -- {}", shell_quote(remote_dir)));
    match output_with_deadline(ssh, Duration::from_secs(20)).await {
        Ok(Some(output)) if output.status.success() => Ok(()),
        Ok(Some(output)) => Err(OrchestratorError::RemoteNodeUnavailable(
            agentflow_process_supervisor::redact(
                String::from_utf8_lossy(&output.stderr).chars().take(1000).collect(),
            ),
        )),
        Ok(None) => Err(OrchestratorError::RemoteNodeUnavailable(
            "remote cleanup timed out".into(),
        )),
        Err(error) => Err(OrchestratorError::RemoteNodeUnavailable(error.to_string())),
    }
}

fn execution_node_from_row(row: sqlx::sqlite::SqliteRow) -> Result<ExecutionNode, OrchestratorError> {
    Ok(ExecutionNode {
        id: row.get("id"), name: row.get("name"), host: row.get("host"),
        port: u16::try_from(row.get::<i64, _>("port")).map_err(|_|OrchestratorError::Config("invalid SSH port".into()))?,
        username: row.get("username"), work_root: row.get("work_root"),
        identity_file: row.get("identity_file"),
        deny_network: row.get::<i64, _>("deny_network") != 0,
        enabled: row.get::<i64, _>("enabled") != 0, status: parse(row.get("status"))?,
        platform: row.get("platform"), git_version: row.get("git_version"),
        problem: row.get("problem"), last_checked_at: row.get("last_checked_at"),
        diagnostics: serde_json::from_str(row.get::<String, _>("diagnostics_json").as_str())
            .unwrap_or_default(),
    })
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod execution_node_tests {
    use super::*;

    #[test]
    fn ssh_fields_and_shell_quoting_are_bounded() -> Result<(), Box<dyn std::error::Error>> {
        let mut node = ExecutionNode {
            id: String::new(), name: "builder".into(), host: "10.0.0.8".into(), port: 22,
            username: "runner".into(), work_root: "/srv/agent flow".into(), identity_file: None,
            deny_network: false, enabled: true,
            status: NodeStatus::Unknown, platform: None, git_version: None, problem: None,
            last_checked_at: None, diagnostics: Vec::new(),
        };
        assert!(validate_execution_node(&node).is_ok());
        let identity = tempfile::NamedTempFile::new()?;
        node.identity_file = Some(identity.path().to_string_lossy().into_owned());
        assert!(validate_execution_node(&node).is_ok());
        let args = ssh_base_args(&node);
        assert!(args.windows(2).any(|pair| pair == ["-i", identity.path().to_string_lossy().as_ref()]));
        node.identity_file = None;
        assert_eq!(shell_quote("a'b"), "'a'\"'\"'b'");
        let command = remote_validation_command(
            &node,
            "/srv/agent flow",
            &["bun".into(), "test; touch /tmp/escaped".into()],
            60,
        );
        assert!(command.contains("$HOME/.bun/bin"));
        assert!(command.contains("'test; touch /tmp/escaped'"));
        // Remote watchdog: fires after the local deadline and kills the session group.
        assert!(command.contains("sleep 62"));
        assert!(command.contains("kill -KILL -- -$$"));
        node.deny_network = true;
        let isolated = remote_validation_command(&node, "/srv/work", &["/bin/true".into()], 5);
        assert!(isolated.contains("sudo -n /usr/local/sbin/agentflow-offline -- '/bin/true'"));
        let mut unsafe_node = node;
        unsafe_node.host = "host;touch".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
        unsafe_node.host = "runner.local".into();
        unsafe_node.work_root = "/".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
        unsafe_node.work_root = "/srv/../root".into();
        assert!(validate_execution_node(&unsafe_node).is_err());
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn timed_out_validation_step_kills_the_whole_process_group()
    -> Result<(), Box<dyn std::error::Error>> {
        let dir = tempfile::tempdir()?;
        let marker = dir.path().join("survived.marker");
        let mut command = Command::new("/bin/sh");
        // The grandchild would create the marker after the deadline; a group kill must
        // take it down together with its parent shell.
        command.arg("-c").arg(format!(
            "(sleep 2; touch {}) & wait",
            marker.to_string_lossy()
        ));
        let started = Instant::now();
        let output = output_with_deadline(command, Duration::from_millis(300)).await?;
        assert!(output.is_none(), "step must report a timeout");
        assert!(started.elapsed() < Duration::from_secs(5));
        tokio::time::sleep(Duration::from_millis(2_500)).await;
        assert!(
            !marker.exists(),
            "grandchild kept running after the timeout: the process group was not killed"
        );
        Ok(())
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
            identity_file: std::env::var("AGENTFLOW_TEST_SSH_IDENTITY").ok(),
            deny_network: std::env::var("AGENTFLOW_TEST_SSH_DENY_NETWORK").as_deref() == Ok("1"),
            enabled: true,
            status: NodeStatus::Unknown,
            platform: None,
            git_version: None,
            problem: None,
            last_checked_at: None,
            diagnostics: Vec::new(),
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
        let validation = if node.deny_network {
            "test -f probe.txt && test -z \"$(/usr/sbin/ip -4 route show default)\" && test -z \"$(/usr/sbin/ip -6 route show default)\" && ! sudo -n true >/dev/null 2>&1 && printf AGENTFLOW_REMOTE_VALIDATION_OK"
        } else {
            "test -f probe.txt && printf AGENTFLOW_REMOTE_VALIDATION_OK"
        };
        let argv = vec!["/bin/sh".into(), "-c".into(), validation.into()];
        let result = Command::new("ssh")
            .args(ssh_base_args(&node))
            .arg(&destination)
            .arg(remote_validation_command(&node, &remote_dir, &argv, 120))
            .output()
            .await?;
        cleanup_remote_dir(&node, &remote_dir).await?;
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        let output = String::from_utf8_lossy(&result.stdout);
        assert!(output.contains("AGENTFLOW_REMOTE_VALIDATION_OK"), "{output}");

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
