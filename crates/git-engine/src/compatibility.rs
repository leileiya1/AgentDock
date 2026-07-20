use agentflow_contracts::GitCompatibilityReport;
use std::collections::BTreeMap;
use tokio::io::AsyncWriteExt;

impl Git {
    pub async fn compatibility_report(
        &self,
        repo: &Path,
    ) -> Result<GitCompatibilityReport, GitError> {
        let identity = self.repository_identity(repo).await?;
        let shallow = self
            .optional_text(repo, &["rev-parse", "--is-shallow-repository"])
            .await
            .as_deref()
            == Some("true");
        let sparse_checkout = self
            .optional_text(repo, &["config", "--bool", "core.sparseCheckout"])
            .await
            .as_deref()
            == Some("true");
        let sparse_patterns = if sparse_checkout {
            self.optional_text(repo, &["sparse-checkout", "list"])
                .await
                .unwrap_or_default()
                .lines()
                .map(str::to_string)
                .collect()
        } else {
            Vec::new()
        };
        let submodules = self.submodule_paths(repo).await;
        let attributes = self
            .optional_text(repo, &["grep", "-I", "filter=lfs", "HEAD", "--", "*.gitattributes"])
            .await
            .unwrap_or_default();
        let lfs_tracked = !attributes.trim().is_empty();
        let lfs_available = self.command_succeeds(repo, &["lfs", "version"]).await;
        let remotes = self
            .optional_text(repo, &["remote", "-v"])
            .await
            .unwrap_or_default();
        let ssh_remote = remotes.lines().any(|line| {
            line.contains("git@") || line.contains("ssh://") || line.contains("git+ssh://")
        });
        let ssh_agent_available = if ssh_remote {
            std::env::var_os("SSH_AUTH_SOCK").is_some() && ssh_agent_has_identity().await
        } else {
            true
        };
        let fs_name = filesystem_name(repo).await.unwrap_or_default().to_ascii_lowercase();
        let network_filesystem = ["nfs", "smb", "afp", "sshfs", "fuse", "webdav"]
            .into_iter()
            .any(|kind| fs_name.contains(kind));
        let case_insensitive = self
            .optional_text(repo, &["config", "--bool", "core.ignorecase"])
            .await
            .as_deref()
            == Some("true");
        let case_collisions = self.case_collisions(repo).await?;
        let mut warnings = Vec::new();
        let mut blockers = Vec::new();
        if shallow {
            warnings.push("仓库是 shallow clone；只能使用本地已存在的目标提交，历史比较可能受限".into());
        }
        if sparse_checkout {
            warnings.push("仓库启用了 sparse checkout；AgentFlow 会把相同 pattern 应用到任务 worktree".into());
        }
        if network_filesystem {
            warnings.push(format!("仓库位于网络文件系统 {fs_name}；Git 锁与文件监听可能更慢"));
        }
        if ssh_remote && !ssh_agent_available {
            warnings.push("SSH remote 未检测到可用 ssh-agent identity；本地执行可继续，但 push 可能需要交互认证".into());
        }
        if lfs_tracked && !lfs_available {
            blockers.push("仓库包含 Git LFS 指针，但未安装或无法运行 git-lfs".into());
        }
        if case_insensitive && !case_collisions.is_empty() {
            blockers.push(format!(
                "大小写不敏感文件系统无法安全检出以下冲突：{}",
                case_collisions.join("、")
            ));
        }
        Ok(GitCompatibilityReport {
            repo_path: repo.to_string_lossy().into_owned(),
            repository_identity: identity,
            shallow,
            sparse_checkout,
            sparse_patterns,
            submodules,
            lfs_tracked,
            lfs_available,
            ssh_remote,
            ssh_agent_available,
            network_filesystem,
            case_insensitive,
            case_collisions,
            warnings,
            blockers,
        })
    }

    pub async fn repository_identity(&self, repo: &Path) -> Result<String, GitError> {
        let roots = text(
            self.output(repo, &["rev-list", "--max-parents=0", "--all"])
                .await?,
        )?;
        if roots.is_empty() {
            return Err(GitError::InvalidOutput(
                "repository has no commits and cannot be isolated".into(),
            ));
        }
        let mut roots = roots.lines().collect::<Vec<_>>();
        roots.sort_unstable();
        let remotes = self
            .optional_text(repo, &["remote", "-v"])
            .await
            .unwrap_or_default();
        let fetch_remotes = remotes
            .lines()
            .filter(|line| line.ends_with("(fetch)"))
            .collect::<Vec<_>>();
        let payload = format!("roots={}\nremotes={}", roots.join(","), fetch_remotes.join("\n"));
        Ok(format!("{:x}", Sha256::digest(payload)))
    }

    pub async fn prepare_linked_worktree(
        &self,
        worktree: &Path,
        report: &GitCompatibilityReport,
    ) -> Result<(), GitError> {
        if !report.blockers.is_empty() {
            return Err(GitError::InvalidOutput(report.blockers.join("; ")));
        }
        if report.sparse_checkout && !report.sparse_patterns.is_empty() {
            let mut command = Command::new(&self.executable);
            command
                .args(["sparse-checkout", "set", "--stdin"])
                .current_dir(worktree)
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::piped());
            let mut child = command.spawn()?;
            let mut stdin = child.stdin.take().ok_or_else(|| {
                GitError::InvalidOutput("sparse-checkout command has no stdin".into())
            })?;
            stdin
                .write_all(report.sparse_patterns.join("\n").as_bytes())
                .await?;
            drop(stdin);
            let output = child.wait_with_output().await?;
            if !output.status.success() {
                return Err(GitError::Failed {
                    operation: "sparse-checkout set --stdin".into(),
                    stderr: String::from_utf8_lossy(&output.stderr).trim().into(),
                });
            }
        }
        if !report.submodules.is_empty() {
            self.output(worktree, &["submodule", "sync", "--recursive"])
                .await?;
            let explicitly_allows_file = self
                .optional_text(worktree, &["config", "--get", "protocol.file.allow"])
                .await
                .as_deref()
                == Some("always");
            if explicitly_allows_file {
                // Respect a repository-local user choice; never enable local-path transport
                // merely because a committed .gitmodules file asks for it.
                self.output(
                    worktree,
                    &[
                        "-c",
                        "protocol.file.allow=always",
                        "submodule",
                        "update",
                        "--init",
                        "--recursive",
                        "--checkout",
                    ],
                )
                .await?;
            } else {
                self.output(
                    worktree,
                    &["submodule", "update", "--init", "--recursive", "--checkout"],
                )
                .await?;
            }
        }
        if report.lfs_tracked {
            self.output(worktree, &["lfs", "checkout"]).await?;
        }
        Ok(())
    }

    async fn submodule_paths(&self, repo: &Path) -> Vec<String> {
        self.optional_text(
            repo,
            &["config", "--file", ".gitmodules", "--get-regexp", "path"],
        )
        .await
        .unwrap_or_default()
        .lines()
        .filter_map(|line| line.split_whitespace().nth(1).map(str::to_string))
        .collect()
    }

    async fn case_collisions(&self, repo: &Path) -> Result<Vec<String>, GitError> {
        let files = text(self.output(repo, &["ls-files"]).await?)?;
        Ok(case_collisions_from_paths(files.lines()))
    }

    async fn optional_text(&self, repo: &Path, args: &[&str]) -> Option<String> {
        self.output(repo, args).await.ok().and_then(|bytes| text(bytes).ok())
    }

    async fn command_succeeds(&self, repo: &Path, args: &[&str]) -> bool {
        Command::new(&self.executable)
            .args(args)
            .current_dir(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success())
    }
}

fn case_collisions_from_paths<'a>(paths: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for path in paths {
        grouped
            .entry(path.to_lowercase())
            .or_default()
            .push(path.to_string());
    }
    grouped
        .into_values()
        .filter(|items| items.len() > 1)
        .map(|items| items.join(" / "))
        .collect()
}

async fn ssh_agent_has_identity() -> bool {
    Command::new("ssh-add")
        .arg("-l")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .is_ok_and(|status| status.success())
}

async fn filesystem_name(repo: &Path) -> Option<String> {
    #[cfg(target_os = "macos")]
    let args = ["-f", "%T"];
    #[cfg(not(target_os = "macos"))]
    let args = ["-f", "-c", "%T"];
    let output = Command::new("stat")
        .args(args)
        .arg(repo)
        .output()
        .await
        .ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().into())
}

#[cfg(test)]
mod compatibility_tests {
    use super::case_collisions_from_paths;

    #[test]
    fn finds_case_collisions_deterministically() {
        assert_eq!(
            case_collisions_from_paths(["src/api.rs", "README.md", "src/API.rs"]),
            vec!["src/api.rs / src/API.rs"]
        );
        assert!(case_collisions_from_paths(["src/api.rs", "src/client.rs"]).is_empty());
    }
}
