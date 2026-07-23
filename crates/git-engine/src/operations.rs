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
            self.output(
                repo,
                &["log", &max_count, "--format=%H%x09%s", &range],
            )
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
        self.output(repo, &["archive", "--format=tar", commit]).await
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

    pub async fn reset_branch_head(
        &self,
        repo: &Path,
        commit: &str,
    ) -> Result<(), GitError> {
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
