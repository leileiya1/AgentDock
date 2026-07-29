const ROLLBACK_FILE_PREVIEW_LIMIT: usize = 50;
const ROLLBACK_COMMIT_PREVIEW_LIMIT: usize = 10;

impl Orchestrator {
    /// Builds an authoritative, read-only impact preview for the desktop.
    /// `task_rollback` repeats the checks immediately before changing Git state.
    pub async fn task_rollback_preflight(
        &self,
        task_id: &str,
    ) -> Result<RollbackPreflight, OrchestratorError> {
        let task = self.task(task_id).await?;
        let project = self.project(&task.project_id).await?;
        let delivery = self.delivery_record(task_id).await?;
        let delivery_mode = delivery
            .as_ref()
            .map(|record| record.mode)
            .unwrap_or(task.policy.delivery_mode);
        let current_branch = self.git.current_branch(&project.repo).await?;
        let head_commit = self.git.resolve(&project.repo, "HEAD").await.ok();
        let working_tree_clean = self.git.is_clean(&project.repo).await?;
        let merge_commit = delivery
            .as_ref()
            .and_then(|record| record.merge_commit.clone());
        let pre_merge_commit = delivery
            .as_ref()
            .and_then(|record| record.pre_merge_commit.clone());

        let merge_reachable = match (merge_commit.as_deref(), head_commit.as_deref()) {
            (Some(merge), Some(head)) => self.git.is_ancestor(&project.repo, merge, head).await?,
            _ => false,
        };
        let (later_commit_count, later_commits) = match (
            merge_commit.as_deref(),
            head_commit.as_deref(),
            merge_reachable,
        ) {
            (Some(merge), Some(head), true) => {
                let (count, commits) = self
                    .git
                    .commits_between(&project.repo, merge, head, ROLLBACK_COMMIT_PREVIEW_LIMIT)
                    .await?;
                (
                    count,
                    commits
                        .into_iter()
                        .map(|(sha, subject)| RollbackCommitPreview { sha, subject })
                        .collect(),
                )
            }
            _ => (0, Vec::new()),
        };

        let impact_base = match (pre_merge_commit.clone(), merge_commit.as_deref()) {
            (Some(before), _) => Some(before),
            (None, Some(merge)) => self
                .git
                .resolve(&project.repo, &format!("{merge}^1"))
                .await
                .ok(),
            (None, None) => None,
        };
        let affected_paths = match (impact_base.as_deref(), merge_commit.as_deref()) {
            (Some(before), Some(merge)) => self
                .git
                .changed_paths_between(&project.repo, before, merge)
                .await?,
            _ => Vec::new(),
        };
        let affected_file_count = i64::try_from(affected_paths.len()).unwrap_or(i64::MAX);
        let affected_files_truncated = affected_paths.len() > ROLLBACK_FILE_PREVIEW_LIMIT;
        let affected_files = affected_paths
            .into_iter()
            .take(ROLLBACK_FILE_PREVIEW_LIMIT)
            .collect();

        let mut shared_blockers = Vec::new();
        if task.status != TaskStatus::Merged {
            shared_blockers.push(RollbackBlocker::TaskNotMerged);
        }
        if delivery.is_none() {
            shared_blockers.push(RollbackBlocker::DeliveryRecordMissing);
        }
        if delivery_mode != DeliveryMode::LocalMerge {
            shared_blockers.push(RollbackBlocker::RemoteDeliveryUnsupported);
        }
        if current_branch.as_deref() != Some(task.target_branch.as_str()) {
            shared_blockers.push(RollbackBlocker::TargetBranchNotCheckedOut);
        }
        if !working_tree_clean {
            shared_blockers.push(RollbackBlocker::DirtyWorkingTree);
        }
        if merge_commit.is_none() {
            shared_blockers.push(RollbackBlocker::MergeCommitMissing);
        }

        let mut undo_blockers = shared_blockers.clone();
        if pre_merge_commit.is_none() {
            undo_blockers.push(RollbackBlocker::PreMergeCommitMissing);
        }
        if later_commit_count > 0 {
            undo_blockers.push(RollbackBlocker::LaterCommitsExist);
        }
        if merge_commit.is_some() && !merge_reachable {
            undo_blockers.push(RollbackBlocker::MergeNotInHead);
        }
        if head_commit.as_deref() != merge_commit.as_deref() && later_commit_count == 0 {
            push_blocker(&mut undo_blockers, RollbackBlocker::MergeNotInHead);
        }

        let mut revert_blockers = shared_blockers;
        if merge_commit.is_some() && !merge_reachable {
            revert_blockers.push(RollbackBlocker::MergeNotInHead);
        }
        let can_undo = undo_blockers.is_empty();
        let can_revert = revert_blockers.is_empty();
        let (recommended_strategy, recommendation_reason) = if can_undo {
            (
                Some(RollbackStrategy::Undo),
                RollbackRecommendationReason::UndoExactHead,
            )
        } else if can_revert {
            (
                Some(RollbackStrategy::Revert),
                RollbackRecommendationReason::RevertPreservesLaterCommits,
            )
        } else {
            (None, RollbackRecommendationReason::NoSafeStrategy)
        };

        Ok(RollbackPreflight {
            task_id: task.id,
            delivery_mode,
            target_branch: task.target_branch,
            current_branch,
            head_commit,
            merge_commit,
            pre_merge_commit,
            working_tree_clean,
            later_commit_count,
            later_commits,
            affected_file_count,
            affected_files,
            affected_files_truncated,
            can_undo,
            undo_blockers,
            can_revert,
            revert_blockers,
            recommended_strategy,
            recommendation_reason,
            generated_at: Utc::now().to_rfc3339(),
        })
    }

    pub async fn task_rollback(
        &self,
        task_id: &str,
        strategy: RollbackStrategy,
    ) -> Result<TaskSummary, OrchestratorError> {
        let preflight = self.task_rollback_preflight(task_id).await?;
        let allowed = match strategy {
            RollbackStrategy::Undo => preflight.can_undo,
            RollbackStrategy::Revert => preflight.can_revert,
        };
        if !allowed {
            let blockers = match strategy {
                RollbackStrategy::Undo => &preflight.undo_blockers,
                RollbackStrategy::Revert => &preflight.revert_blockers,
            };
            return Err(OrchestratorError::RollbackUnsafe(
                blockers
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join(","),
            ));
        }

        let task = self.task(task_id).await?;
        let project = self.project(&task.project_id).await?;
        let delivery = self
            .delivery_record(task_id)
            .await?
            .ok_or_else(|| OrchestratorError::RollbackUnsafe("delivery_record_missing".into()))?;
        if delivery.mode != DeliveryMode::LocalMerge
            || self.git.current_branch(&project.repo).await?.as_deref()
                != Some(task.target_branch.as_str())
            || !self.git.is_clean(&project.repo).await?
        {
            return Err(OrchestratorError::RollbackUnsafe(
                "rollback_preflight_changed".into(),
            ));
        }
        let merge = delivery
            .merge_commit
            .as_deref()
            .ok_or_else(|| OrchestratorError::RollbackUnsafe("merge_commit_missing".into()))?;
        let current_head = self.git.resolve(&project.repo, "HEAD").await?;
        let rollback_commit = match strategy {
            RollbackStrategy::Undo => {
                let before = delivery.pre_merge_commit.as_deref().ok_or_else(|| {
                    OrchestratorError::RollbackUnsafe("pre_merge_commit_missing".into())
                })?;
                if current_head != merge {
                    return Err(OrchestratorError::RollbackUnsafe(
                        "later_commits_exist".into(),
                    ));
                }
                self.git.reset_branch_head(&project.repo, before).await?;
                before.to_string()
            }
            RollbackStrategy::Revert => {
                if !self
                    .git
                    .is_ancestor(&project.repo, merge, &current_head)
                    .await?
                {
                    return Err(OrchestratorError::RollbackUnsafe(
                        "merge_not_in_head".into(),
                    ));
                }
                self.git
                    .revert_merge(
                        &project.repo,
                        merge,
                        &format!("[agentflow] rollback TASK-{}: {}", task.seq, task.title),
                    )
                    .await?
            }
        };
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE delivery_records SET state='rolled_back',rollback_commit=?,updated_at=? WHERE task_id=?")
            .bind(&rollback_commit).bind(&now).bind(task_id).execute(self.store.pool()).await?;
        self.store.transition(
            task_id, &[TaskStatus::Merged], TaskStatus::RolledBack, None, Actor::Human,
            "human:rollback", &json!({"strategy":strategy,"merge_commit":merge,"rollback_commit":rollback_commit}),
        ).await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
}

fn push_blocker(blockers: &mut Vec<RollbackBlocker>, blocker: RollbackBlocker) {
    if !blockers.contains(&blocker) {
        blockers.push(blocker);
    }
}
