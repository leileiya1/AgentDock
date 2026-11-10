impl Git {
    pub async fn current_branch(&self, repo: &Path) -> Result<Option<String>, GitError> {
        match self
            .output(repo, &["symbolic-ref", "--quiet", "--short", "HEAD"])
            .await
        {
            Ok(value) => text(value).map(Some),
            Err(GitError::Failed { .. }) => Ok(None),
            Err(error) => Err(error),
        }
    }

    pub async fn changed_paths_between(
        &self,
        repo: &Path,
        base: &str,
        head: &str,
    ) -> Result<Vec<String>, GitError> {
        let raw = self
            .output(repo, &["diff", "--name-only", "-z", base, head])
            .await?;
        raw.split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
            .map(|path| {
                String::from_utf8(path.to_vec())
                    .map_err(|error| GitError::InvalidOutput(error.to_string()))
            })
            .collect()
    }

    pub async fn commits_between(
        &self,
        repo: &Path,
        base: &str,
        head: &str,
        limit: usize,
    ) -> Result<(i64, Vec<(String, String)>), GitError> {
        let range = format!("{base}..{head}");
        let count = text(self.output(repo, &["rev-list", "--count", &range]).await?)?
            .parse::<i64>()
            .map_err(|error| GitError::InvalidOutput(error.to_string()))?;
        let max_count = format!("--max-count={limit}");
        let output = text(
            self.output(repo, &["log", &max_count, "--format=%H%x09%s", &range])
                .await?,
        )?;
        let commits = output
            .lines()
            .filter_map(|line| line.split_once('\t'))
            .map(|(sha, subject)| (sha.to_string(), subject.to_string()))
            .collect();
        Ok((count, commits))
    }

    pub async fn worktree_add_detached(
        &self,
        repo: &Path,
        path: &Path,
        commit: &str,
    ) -> Result<(), GitError> {
        let path = path.to_string_lossy();
        self.output(repo, &["worktree", "add", "--detach", &path, commit])
            .await
            .map(|_| ())
    }

    pub async fn archive(&self, repo: &Path, commit: &str) -> Result<Vec<u8>, GitError> {
        self.output(repo, &["archive", "--format=tar", commit])
            .await
    }

    pub async fn remote_url(&self, repo: &Path, remote: &str) -> Result<String, GitError> {
        text(self.output(repo, &["remote", "get-url", remote]).await?)
    }

    pub async fn push_branch(
        &self,
        repo: &Path,
        remote: &str,
        branch: &str,
    ) -> Result<(), GitError> {
        self.output(repo, &["push", "--set-upstream", remote, branch])
            .await
            .map(|_| ())
    }

    pub async fn reset_branch_head(&self, repo: &Path, commit: &str) -> Result<(), GitError> {
        self.output(repo, &["reset", "--hard", commit])
            .await
            .map(|_| ())
    }

    pub async fn revert_merge(
        &self,
        repo: &Path,
        merge_commit: &str,
        message: &str,
    ) -> Result<String, GitError> {
        self.output(repo, &["revert", "-m", "1", "--no-commit", merge_commit])
            .await?;
        if let Err(error) = self.output(repo, &["commit", "-m", message]).await {
            let _ = self.output(repo, &["revert", "--abort"]).await;
            return Err(error);
        }
        self.resolve(repo, "HEAD").await
    }
}

impl Git {
    /// Apply exactly the recorded baseline->result delta, never a whole dependency branch.
    pub async fn apply_team_delta(&self, worktree: &Path, patch: &[u8]) -> Result<(), GitError> {
        if patch.is_empty() {
            return Ok(());
        }
        use tokio::io::AsyncWriteExt;
        let mut child = tokio::process::Command::new(&self.executable)
            .args(["apply", "--index", "--3way", "--whitespace=nowarn", "-"])
            .current_dir(worktree)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()?;
        if let Some(mut input) = child.stdin.take() {
            input.write_all(patch).await?;
        }
        let output = child.wait_with_output().await?;
        if !output.status.success() {
            return Err(GitError::InvalidOutput(
                String::from_utf8_lossy(&output.stderr).into_owned(),
            ));
        }
        Ok(())
    }
    pub async fn team_binary_patch(
        &self,
        repo: &Path,
        base: &str,
        result: &str,
    ) -> Result<Vec<u8>, GitError> {
        self.output(repo, &["diff", "--binary", "--full-index", base, result])
            .await
    }
    pub async fn team_update_ref(
        &self,
        repo: &Path,
        reference: &str,
        sha: &str,
        old: &str,
    ) -> Result<(), GitError> {
        self.output(repo, &["update-ref", reference, sha, old])
            .await
            .map(|_| ())
    }
}

impl Git {
    /// Exact tested-tree local delivery. CAS protects against an externally advanced target;
    /// read-tree refuses to overwrite concurrent local edits. An interrupted checkout is a
    /// reconcilable operation, never grounds for repeating a merge against another baseline.
    pub async fn team_fast_forward_checkout(
        &self,
        repo: &Path,
        target: &str,
        expected: &str,
        candidate: &str,
        operation_id: &str,
    ) -> Result<(), GitError> {
        if self.current_branch(repo).await?.as_deref() != Some(target)
            || !self.is_clean(repo).await?
        {
            return Err(GitError::InvalidOutput(
                "target checkout is not clean/on requested branch".into(),
            ));
        }
        if !self.is_ancestor(repo, expected, candidate).await? {
            return Err(GitError::InvalidOutput(
                "candidate is not based on target baseline".into(),
            ));
        }
        self.team_update_ref(repo, &format!("refs/heads/{target}"), candidate, expected)
            .await?;
        let head_path = text(
            self.output(repo, &["rev-parse", "--git-path", "HEAD"])
                .await?,
        )?;
        let head_path = Path::new(&head_path);
        let head_path = if head_path.is_absolute() {
            head_path.to_path_buf()
        } else {
            repo.join(head_path)
        };
        let lock = head_path.with_file_name("HEAD.lock");
        let mut lock_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock)?;
        std::io::Write::write_all(
            &mut lock_file,
            format!("agentflow-team-delivery:{operation_id}\n").as_bytes(),
        )?;
        lock_file.sync_all()?;
        struct HeadLock(std::path::PathBuf);
        impl Drop for HeadLock {
            fn drop(&mut self) {
                let _ = std::fs::remove_file(&self.0);
            }
        }
        let _guard = HeadLock(lock);
        if self.current_branch(repo).await?.as_deref() != Some(target) {
            return Err(GitError::InvalidOutput(
                "checkout switched during delivery; target ref requires reconciliation".into(),
            ));
        }
        self.output(repo, &["read-tree", "-m", "-u", expected, candidate])
            .await?;
        if !self.is_clean(repo).await? {
            return Err(GitError::InvalidOutput(
                "delivery checkout differs from tested content; reconcile result".into(),
            ));
        }
        Ok(())
    }
}

impl Git {
    /// Only called by sole-owner startup recovery, after the previous daemon exited.
    pub async fn team_recover_head_lock(
        &self,
        repo: &Path,
        operation_id: &str,
    ) -> Result<(), GitError> {
        let raw = text(
            self.output(repo, &["rev-parse", "--git-path", "HEAD"])
                .await?,
        )?;
        let head = Path::new(&raw);
        let head = if head.is_absolute() {
            head.to_path_buf()
        } else {
            repo.join(head)
        };
        let lock = head.with_file_name("HEAD.lock");
        if std::fs::read_to_string(&lock).ok().as_deref()
            == Some(&format!("agentflow-team-delivery:{operation_id}\n"))
        {
            std::fs::remove_file(lock)?;
        }
        Ok(())
    }
}
