impl Orchestrator {
    pub async fn approve(
        &self,
        task_id: &str,
        revision: i64,
        sha: &str,
        diff_sha: &str,
    ) -> Result<TaskSummary, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task.status != TaskStatus::WaitingForHumanApproval || task.revision != revision {
            return Err(OrchestratorError::InvalidState("TASK_INVALID_STATE".into()));
        }
        let project = self.project(&task.project_id).await?;
        let config = load_config(&project.repo).await?;
        let actual = self
            .git
            .diff(
                &required_path(&task.worktree_path)?,
                task.base_commit.as_deref().unwrap_or(""),
                sha,
                &config.review.exclude_globs,
                config.review.max_patch_bytes,
            )
            .await?;
        if actual.diff_sha256 != diff_sha {
            return Err(OrchestratorError::DiffStale);
        }
        sqlx::query("INSERT INTO approvals(id,task_id,revision,commit_sha,diff_sha256,action,created_at) VALUES(?,?,?,?,?,'approve',?)").bind(Uuid::now_v7().to_string()).bind(task_id).bind(revision).bind(sha).bind(diff_sha).bind(Utc::now().to_rfc3339()).execute(self.store.pool()).await?;
        self.store
            .transition(
                task_id,
                &[TaskStatus::WaitingForHumanApproval],
                TaskStatus::Approved,
                None,
                Actor::Human,
                "human:approve",
                &json!({"commit_sha":sha}),
            )
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }
    pub async fn events_export(&self, project_id: &str) -> Result<PathBuf, OrchestratorError> {
        let dir = self.app_data.join("exports");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(format!(
            "events-{}-{}.jsonl",
            project_id,
            Utc::now().format("%Y%m%dT%H%M%SZ")
        ));
        let rows=sqlx::query("SELECT e.id,e.task_id,e.run_id,e.revision,e.actor,e.event_type,e.payload_json,e.created_at FROM events e LEFT JOIN tasks t ON e.task_id=t.id WHERE t.project_id=? OR e.task_id IS NULL ORDER BY e.id").bind(project_id).fetch_all(self.store.pool()).await?;
        let mut file = tokio::fs::File::create(&path).await?;
        for row in rows {
            let value = json!({"id":row.get::<i64,_>("id"),"task_id":row.get::<Option<String>,_>("task_id"),"run_id":row.get::<Option<String>,_>("run_id"),"revision":row.get::<Option<i64>,_>("revision"),"actor":row.get::<String,_>("actor"),"event_type":row.get::<String,_>("event_type"),"payload":serde_json::from_str::<Value>(&row.get::<String,_>("payload_json")).unwrap_or(Value::Null),"created_at":row.get::<String,_>("created_at")});
            file.write_all(serde_json::to_string(&value).unwrap_or_default().as_bytes())
                .await?;
            file.write_all(b"\n").await?
        }
        Ok(path)
    }

    pub async fn storage_report(&self) -> Result<StorageReport, OrchestratorError> {
        let app_data = self.app_data.clone();
        let mut report = tokio::task::spawn_blocking(move || storage_report_sync(&app_data))
            .await
            .map_err(|error| OrchestratorError::InvalidState(error.to_string()))??;
        report.trash_entries = sqlx::query_scalar("SELECT COUNT(*) FROM trash_items")
            .fetch_one(self.store.pool())
            .await?;
        Ok(report)
    }

    pub async fn storage_cleanup(
        &self,
        automatic: bool,
    ) -> Result<CleanupResult, OrchestratorError> {
        let policy = self.settings_get().await?.storage;
        if automatic && !policy.auto_cleanup {
            return Ok(CleanupResult::default());
        }
        let mut result = CleanupResult::default();
        let cutoff = Utc::now() - chrono::Duration::days(i64::from(policy.raw_logs_days));
        let rows = sqlx::query(
            "SELECT r.task_id,r.run_dir FROM agent_runs r JOIN tasks t ON t.id=r.task_id WHERE t.deleted_at IS NULL AND t.status IN ('MERGED','ROLLED_BACK','CANCELLED') AND r.finished_at IS NOT NULL AND r.finished_at<?",
        )
        .bind(cutoff.to_rfc3339())
        .fetch_all(self.store.pool())
        .await?;
        let mut cleaned_tasks = HashSet::new();
        for row in rows {
            let task_id: String = row.get("task_id");
            let run_dir = PathBuf::from(row.get::<String, _>("run_dir"));
            let cleaned = remove_raw_run_files(&run_dir).await?;
            if cleaned.files_removed > 0 {
                cleaned_tasks.insert(task_id);
            }
            merge_cleanup(&mut result, cleaned);
        }
        for task_id in cleaned_tasks {
            sqlx::query("UPDATE reviews SET raw_path='' WHERE task_id=?")
                .bind(task_id)
                .execute(self.store.pool())
                .await?;
        }

        let expired: Vec<String> =
            sqlx::query_scalar("SELECT task_id FROM trash_items WHERE purge_after<=?")
                .bind(Utc::now().to_rfc3339())
                .fetch_all(self.store.pool())
                .await?;
        for task_id in expired {
            let purged = self.purge_task(&task_id).await?;
            merge_cleanup(&mut result, purged);
        }
        let cache_cleanup =
            trim_cache(&self.app_data.join("cache"), policy.cache_max_bytes).await?;
        merge_cleanup(&mut result, cache_cleanup);
        Ok(result)
    }

    pub async fn task_cleanup(
        &self,
        task_id: &str,
        scope: StorageCleanupScope,
    ) -> Result<CleanupResult, OrchestratorError> {
        let task = self.task(task_id).await?;
        if task_is_running(task.status) {
            return Err(OrchestratorError::InvalidState(
                "running tasks cannot be cleaned or deleted".into(),
            ));
        }
        if scope == StorageCleanupScope::Everything {
            return self.trash_task(&task).await;
        }
        let rows: Vec<String> =
            sqlx::query_scalar("SELECT run_dir FROM agent_runs WHERE task_id=?")
                .bind(task_id)
                .fetch_all(self.store.pool())
                .await?;
        let mut result = CleanupResult::default();
        for run_dir in rows {
            merge_cleanup(
                &mut result,
                remove_raw_run_files(Path::new(&run_dir)).await?,
            );
        }
        sqlx::query("UPDATE reviews SET raw_path='' WHERE task_id=?")
            .bind(task_id)
            .execute(self.store.pool())
            .await?;
        if scope == StorageCleanupScope::Runtime {
            let project = self.project(&task.project_id).await?;
            if let Some(worktree) = task.worktree_path.as_ref()
                && worktree.exists()
            {
                self.remove_owned_worktree(&project, worktree).await?;
            }
            sqlx::query("UPDATE tasks SET worktree_path=NULL WHERE id=?")
                .bind(task_id)
                .execute(self.store.pool())
                .await?;
        }
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,'human','storage:cleanup',?,?)")
            .bind(task_id)
            .bind(task.revision)
            .bind(json!({"scope": scope, "bytesReclaimed": result.bytes_reclaimed}).to_string())
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        Ok(result)
    }

    async fn trash_task(&self, task: &TaskRow) -> Result<CleanupResult, OrchestratorError> {
        let project = self.project(&task.project_id).await?;
        if let Some(worktree) = task.worktree_path.as_ref()
            && worktree.exists()
        {
            self.remove_owned_worktree(&project, worktree).await?;
        }
        let source = self.task_dir(&task.id);
        let trash_root = self.app_data.join(".trash");
        tokio::fs::create_dir_all(&trash_root).await?;
        let destination = trash_root.join(format!(
            "{}-{}",
            task.id,
            Utc::now().format("%Y%m%dT%H%M%S")
        ));
        let bytes = directory_stats(&source).await?.0;
        let moved = source.exists();
        if moved {
            tokio::fs::rename(&source, &destination).await?;
        }
        let settings = self.settings_get().await?;
        let trashed_at = Utc::now();
        let purge_after =
            trashed_at + chrono::Duration::days(i64::from(settings.storage.trash_days));
        let mut tx = self.store.pool().begin().await?;
        let stored = async {
            sqlx::query("DELETE FROM daemon_queue WHERE task_id=?")
                .bind(&task.id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE tasks SET deleted_at=?,worktree_path=NULL,updated_at=? WHERE id=?")
                .bind(trashed_at.to_rfc3339())
                .bind(trashed_at.to_rfc3339())
                .bind(&task.id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO trash_items(task_id,original_path,trashed_path,bytes,trashed_at,purge_after) VALUES(?,?,?,?,?,?)")
                .bind(&task.id)
                .bind(source.to_string_lossy().as_ref())
                .bind(destination.to_string_lossy().as_ref())
                .bind(bytes as i64)
                .bind(trashed_at.to_rfc3339())
                .bind(purge_after.to_rfc3339())
                .execute(&mut *tx)
                .await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        if let Err(error) = stored {
            let _ = tx.rollback().await;
            if moved {
                let _ = tokio::fs::rename(&destination, &source).await;
            }
            return Err(error.into());
        }
        tx.commit().await?;
        Ok(CleanupResult {
            tasks_trashed: 1,
            ..Default::default()
        })
    }

    pub async fn trash_list(&self) -> Result<Vec<TrashEntry>, OrchestratorError> {
        let rows = sqlx::query("SELECT x.task_id,t.title,x.bytes,x.trashed_at,x.purge_after FROM trash_items x JOIN tasks t ON t.id=x.task_id ORDER BY x.trashed_at DESC")
            .fetch_all(self.store.pool()).await?;
        Ok(rows
            .into_iter()
            .map(|row| TrashEntry {
                task_id: row.get("task_id"),
                title: row.get("title"),
                bytes: row.get::<i64, _>("bytes").max(0) as u64,
                trashed_at: row.get("trashed_at"),
                purge_after: row.get("purge_after"),
            })
            .collect())
    }

    pub async fn task_restore(&self, task_id: &str) -> Result<TaskSummary, OrchestratorError> {
        let row = sqlx::query("SELECT original_path,trashed_path FROM trash_items WHERE task_id=?")
            .bind(task_id)
            .fetch_one(self.store.pool())
            .await?;
        let original = PathBuf::from(row.get::<String, _>("original_path"));
        let trashed = PathBuf::from(row.get::<String, _>("trashed_path"));
        if original.exists() {
            return Err(OrchestratorError::InvalidState(
                "cannot restore because the original task directory already exists".into(),
            ));
        }
        if trashed.exists() {
            if let Some(parent) = original.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::rename(&trashed, &original).await?;
        }
        let moved = original.exists();
        let mut tx = self.store.pool().begin().await?;
        let restored = async {
            sqlx::query("DELETE FROM trash_items WHERE task_id=?")
                .bind(task_id)
                .execute(&mut *tx)
                .await?;
            sqlx::query("UPDATE tasks SET deleted_at=NULL,updated_at=? WHERE id=?")
                .bind(Utc::now().to_rfc3339())
                .bind(task_id)
                .execute(&mut *tx)
                .await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        if let Err(error) = restored {
            let _ = tx.rollback().await;
            if moved {
                let _ = tokio::fs::rename(&original, &trashed).await;
            }
            return Err(error.into());
        }
        tx.commit().await?;
        sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'human','storage:restored','{}',?)")
            .bind(task_id)
            .bind(Utc::now().to_rfc3339())
            .execute(self.store.pool())
            .await?;
        self.store.task_summary(task_id).await.map_err(Into::into)
    }

    pub async fn trash_empty(&self) -> Result<CleanupResult, OrchestratorError> {
        let ids: Vec<String> = sqlx::query_scalar("SELECT task_id FROM trash_items")
            .fetch_all(self.store.pool())
            .await?;
        let mut result = CleanupResult::default();
        for task_id in ids {
            merge_cleanup(&mut result, self.purge_task(&task_id).await?);
        }
        Ok(result)
    }

    async fn purge_task(&self, task_id: &str) -> Result<CleanupResult, OrchestratorError> {
        let row = sqlx::query("SELECT trashed_path,bytes FROM trash_items WHERE task_id=?")
            .bind(task_id)
            .fetch_one(self.store.pool())
            .await?;
        let path = PathBuf::from(row.get::<String, _>("trashed_path"));
        let bytes = row.get::<i64, _>("bytes").max(0) as u64;
        let purge_path = path.with_extension(format!("purging-{}", Uuid::now_v7()));
        let moved = path.exists();
        if moved {
            tokio::fs::rename(&path, &purge_path).await?;
        }
        let mut tx = self.store.pool().begin().await?;
        let deleted = async {
            sqlx::query(
                "DELETE FROM review_issues WHERE review_id IN (SELECT id FROM reviews WHERE task_id=?)",
            )
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
            for table in [
                "reviews",
                "approvals",
                "events",
                "artifacts",
                "task_revisions",
                "agent_runs",
                "daemon_queue",
                "trash_items",
            ] {
                sqlx::query(&format!("DELETE FROM {table} WHERE task_id=?"))
                    .bind(task_id)
                    .execute(&mut *tx)
                    .await?;
            }
            sqlx::query("DELETE FROM tasks WHERE id=?")
                .bind(task_id)
                .execute(&mut *tx)
                .await?;
            Ok::<(), sqlx::Error>(())
        }
        .await;
        if let Err(error) = deleted {
            let _ = tx.rollback().await;
            if moved {
                let _ = tokio::fs::rename(&purge_path, &path).await;
            }
            return Err(error.into());
        }
        if let Err(error) = tx.commit().await {
            if moved {
                let _ = tokio::fs::rename(&purge_path, &path).await;
            }
            return Err(error.into());
        }
        if moved {
            tokio::fs::remove_dir_all(&purge_path).await?;
        }
        Ok(CleanupResult {
            bytes_reclaimed: bytes,
            tasks_purged: 1,
            ..Default::default()
        })
    }

    async fn remove_owned_worktree(
        &self,
        project: &ProjectRow,
        worktree: &Path,
    ) -> Result<(), OrchestratorError> {
        if !worktree.starts_with(&project.worktree_root) {
            return Err(OrchestratorError::InvalidState(format!(
                "refusing to remove worktree outside AgentFlow root: {}",
                worktree.display()
            )));
        }
        match self.git.worktree_remove(&project.repo, worktree).await {
            Ok(()) => Ok(()),
            Err(error) if !project.repo.exists() => {
                tokio::fs::remove_dir_all(worktree).await?;
                let _ = error;
                Ok(())
            }
            Err(error) => Err(error.into()),
        }
    }

    pub async fn notify_task(&self, summary: &TaskSummary) {
        let Ok(settings) = self.settings_get().await else {
            return;
        };
        if !settings.notifications.enabled {
            return;
        }
        let message = match summary.status {
            TaskStatus::WaitingForHumanApproval if settings.notifications.on_attention => {
                Some("代码和审查已完成，正在等待你的批准")
            }
            TaskStatus::Blocked if settings.notifications.on_attention => {
                Some("任务已暂停，需要你查看阻塞原因")
            }
            TaskStatus::Merged if settings.notifications.on_completion => Some("任务已合并完成"),
            TaskStatus::Cancelled if settings.notifications.on_completion => Some("任务已取消"),
            _ => None,
        };
        if let Some(message) = message {
            self.send_notification(&format!("AgentFlow · {}", summary.title), message)
                .await;
        }
    }

    pub async fn notify_daemon_failure(&self, task_id: &str, detail: &str) {
        let Ok(settings) = self.settings_get().await else {
            return;
        };
        if settings.notifications.enabled && settings.notifications.on_attention {
            self.send_notification(
                "AgentFlow 后台任务失败",
                &format!(
                    "任务 {task_id}：{}",
                    detail.chars().take(180).collect::<String>()
                ),
            )
            .await;
        }
    }

    async fn send_notification(&self, title: &str, message: &str) {
        send_desktop_notification(title, message).await;
    }

}
