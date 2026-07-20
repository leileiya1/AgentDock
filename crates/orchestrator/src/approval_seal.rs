#[derive(Debug)]
struct ApprovalSeal {
    commit_sha: String,
}

impl Orchestrator {
    /// Reads the immutable revision pointer owned by the orchestrator. A SHA supplied by a
    /// desktop or CLI client is never authoritative.
    async fn revision_commit_sha(
        &self,
        task_id: &str,
        revision: i64,
    ) -> Result<String, OrchestratorError> {
        sqlx::query_scalar::<_, Option<String>>(
            "SELECT commit_sha FROM task_revisions WHERE task_id=? AND revision=?",
        )
        .bind(task_id)
        .bind(revision)
        .fetch_optional(self.store.pool())
        .await?
        .flatten()
        .filter(|sha| !sha.trim().is_empty())
        .ok_or_else(|| OrchestratorError::InvalidState("revision commit missing".into()))
    }

    /// Loads the last explicit approval and verifies it still seals the current revision.
    async fn approval_seal(&self, task: &TaskRow) -> Result<ApprovalSeal, OrchestratorError> {
        let approved_sha: Option<String> = sqlx::query_scalar(
            "SELECT commit_sha FROM approvals \
             WHERE task_id=? AND revision=? AND action='approve' \
             ORDER BY created_at DESC,id DESC LIMIT 1",
        )
        .bind(&task.id)
        .bind(task.revision)
        .fetch_optional(self.store.pool())
        .await?;
        let approved_sha = approved_sha
            .ok_or_else(|| OrchestratorError::MergePrecondition("approval seal missing".into()))?;
        let revision_sha = self.revision_commit_sha(&task.id, task.revision).await?;
        if approved_sha != revision_sha {
            return Err(OrchestratorError::MergePrecondition(
                "approved commit no longer matches the task revision".into(),
            ));
        }
        Ok(ApprovalSeal {
            commit_sha: approved_sha,
        })
    }

    /// Verifies the mutable Git refs have not moved since approval.
    async fn verify_sealed_task_heads(
        &self,
        task: &TaskRow,
        project: &ProjectRow,
        seal: &ApprovalSeal,
    ) -> Result<(), OrchestratorError> {
        let branch = task
            .branch
            .as_deref()
            .ok_or_else(|| OrchestratorError::MergePrecondition("task branch missing".into()))?;
        let branch_sha = self.git.resolve(&project.repo, branch).await?;
        if branch_sha != seal.commit_sha {
            return Err(OrchestratorError::MergePrecondition(
                "task branch advanced after approval; review the new revision".into(),
            ));
        }
        let worktree = required_path(&task.worktree_path)?;
        let worktree_sha = self.git.resolve(&worktree, "HEAD").await?;
        if worktree_sha != seal.commit_sha {
            return Err(OrchestratorError::MergePrecondition(
                "task worktree advanced after approval; review the new revision".into(),
            ));
        }
        Ok(())
    }
}
