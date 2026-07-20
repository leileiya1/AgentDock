impl Git {
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
