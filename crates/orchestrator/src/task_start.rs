impl Orchestrator {
    pub async fn task_start(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::Draft {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        if !project.repo.exists() {
            return Err(OrchestratorError::InvalidState(
                "PROJECT_RELOCATED: re-import the repository from its new path".into(),
            ));
        }
        // Validate trust and Git compatibility before creating a branch or worktree, so a failed
        // preflight leaves no partial repository state behind.
        self.load_trusted_config(&project).await?;
        let base = self.git.resolve(&project.repo, &task.target_branch).await?;
        let compatibility = self.git.compatibility_report(&project.repo).await?;
        if !compatibility.blockers.is_empty() {
            return Err(OrchestratorError::InvalidState(format!(
                "GIT_INCOMPATIBLE: {}",
                compatibility.blockers.join("; ")
            )));
        }
        let suffix = task.id.replace('-', "").chars().take(8).collect::<String>();
        let branch = format!("agentflow/TASK-{}-{suffix}", task.seq);
        let wt = isolated_worktree_path(&project, &task);
        let to = if task.policy.require_plan_approval {
            TaskStatus::Planning
        } else {
            TaskStatus::ReadyForDevelopment
        };
        let intent = StartTaskIntent {
            base_commit: base,
            branch,
            worktree_path: wt.to_string_lossy().into_owned(),
            target_status: to.to_string(),
        };
        let (operation_id, stored_intent) = self.begin_start_operation(&task, &intent).await?;
        self.continue_start_operation(task_id, &operation_id, &stored_intent)
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
}
