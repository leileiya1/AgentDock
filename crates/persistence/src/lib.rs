use std::{
    path::{Path, PathBuf},
    str::FromStr,
    sync::Arc,
};

use agentflow_contracts::{
    Actor, AgentKind, BlockedReason, Project, TaskEvent, TaskPolicy, TaskStatus, TaskSummary,
};
use chrono::Utc;
use serde_json::Value;
use sqlx::{
    Row, SqlitePool,
    sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions},
};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum PersistenceError {
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    #[error("database migration error: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("invalid value in database: {0}")]
    InvalidValue(String),
    #[error("task is in {actual}; expected one of {expected:?}")]
    InvalidState {
        actual: TaskStatus,
        expected: Vec<TaskStatus>,
    },
    #[error("database integrity check failed: {0}")]
    Integrity(String),
    #[error("local data protection error: {0}")]
    Crypto(String),
    #[error("invalid database backup: {0}")]
    InvalidBackup(String),
    #[error("database file I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    path: Option<PathBuf>,
    data_key: Arc<[u8; 32]>,
}

mod protection;

impl Store {
    pub async fn open(path: &Path) -> Result<Self, PersistenceError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(sqlx::Error::Io)?;
        }
        let data_dir = path
            .parent()
            .ok_or_else(|| PersistenceError::InvalidBackup("database has no parent".into()))?;
        let data_key = protection::load_data_key(data_dir).await?;
        let existed = path.exists() && tokio::fs::metadata(path).await?.len() > 0;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        let pre_migration = if existed {
            protection::integrity_check(&pool).await?;
            Some(protection::create_encrypted_backup(&pool, path, data_key.as_ref()).await?)
        } else {
            None
        };
        if let Err(error) = sqlx::migrate!().run(&pool).await {
            if let Some(backup) = pre_migration {
                let _ =
                    protection::restore_encrypted_backup(&pool, path, &backup, data_key.as_ref())
                        .await;
            } else {
                pool.close().await;
            }
            return Err(error.into());
        }
        protection::integrity_check(&pool).await?;
        protection::restrict_file(path).await?;
        if !existed {
            protection::create_encrypted_backup(&pool, path, data_key.as_ref()).await?;
        }
        Ok(Self {
            pool,
            path: Some(path.to_path_buf()),
            data_key,
        })
    }

    pub async fn in_memory() -> Result<Self, PersistenceError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self {
            pool,
            path: None,
            data_key: protection::new_data_key()?,
        })
    }

    /// Opens a desktop/CLI query handle after the authoritative daemon has migrated
    /// the database. Client handles never run migrations or create competing backups.
    pub async fn open_client(path: &Path) -> Result<Self, PersistenceError> {
        let data_dir = path
            .parent()
            .ok_or_else(|| PersistenceError::InvalidBackup("database has no parent".into()))?;
        let data_key = protection::load_data_key(data_dir).await?;
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(false)
            .journal_mode(SqliteJournalMode::Wal)
            .foreign_keys(true)
            .busy_timeout(std::time::Duration::from_secs(5));
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        protection::integrity_check(&pool).await?;
        Ok(Self {
            pool,
            path: Some(path.to_path_buf()),
            data_key,
        })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn integrity_check(&self) -> Result<(), PersistenceError> {
        protection::integrity_check(&self.pool).await
    }

    pub async fn backup_now(&self) -> Result<PathBuf, PersistenceError> {
        let path = self
            .path
            .as_deref()
            .ok_or_else(|| PersistenceError::InvalidBackup("in-memory database".into()))?;
        protection::create_encrypted_backup(&self.pool, path, self.data_key.as_ref()).await
    }

    pub async fn backups(&self) -> Result<Vec<PathBuf>, PersistenceError> {
        let database = self
            .path
            .as_deref()
            .ok_or_else(|| PersistenceError::InvalidBackup("in-memory database".into()))?;
        let directory = database
            .parent()
            .ok_or_else(|| PersistenceError::InvalidBackup("database has no parent".into()))?
            .join("backups");
        let mut entries = tokio::fs::read_dir(directory).await?;
        let mut backups = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) == Some("afbak") {
                backups.push(path);
            }
        }
        backups.sort_by(|left, right| right.cmp(left));
        Ok(backups)
    }

    /// Restores a verified encrypted snapshot and closes this pool. The owner daemon
    /// must restart before performing any further database operation.
    pub async fn restore_backup(&self, backup: &Path) -> Result<PathBuf, PersistenceError> {
        let database = self
            .path
            .as_deref()
            .ok_or_else(|| PersistenceError::InvalidBackup("in-memory database".into()))?;
        protection::restore_encrypted_backup(&self.pool, database, backup, self.data_key.as_ref())
            .await
    }

    pub async fn protect_file(&self, path: &Path) -> Result<(), PersistenceError> {
        if !path.exists() {
            return Ok(());
        }
        let bytes = tokio::fs::read(path).await?;
        if bytes.starts_with(b"AFENC1") {
            return Ok(());
        }
        let protected = protection::encrypt_bytes(self.data_key.as_ref(), &bytes)?;
        let temporary = path.with_extension(format!("protected-{}.tmp", Uuid::now_v7()));
        tokio::fs::write(&temporary, protected).await?;
        protection::restrict_file(&temporary).await?;
        tokio::fs::rename(temporary, path).await?;
        Ok(())
    }

    pub async fn read_protected_file(&self, path: &Path) -> Result<Vec<u8>, PersistenceError> {
        protection::decrypt_bytes(self.data_key.as_ref(), &tokio::fs::read(path).await?)
    }

    pub async fn import_project(
        &self,
        name: &str,
        repo_path: &str,
        default_branch: &str,
        worktree_root: &str,
    ) -> Result<Project, PersistenceError> {
        self.import_project_identified(name, repo_path, default_branch, worktree_root, None)
            .await
    }

    pub async fn import_project_identified(
        &self,
        name: &str,
        repo_path: &str,
        default_branch: &str,
        worktree_root: &str,
        repo_identity: Option<&str>,
    ) -> Result<Project, PersistenceError> {
        let now = Utc::now().to_rfc3339();
        if let Some(identity) = repo_identity
            && let Some(existing_id) =
                sqlx::query_scalar::<_, String>("SELECT id FROM projects WHERE repo_identity=?")
                    .bind(identity)
                    .fetch_optional(&self.pool)
                    .await?
        {
            sqlx::query("UPDATE projects SET name=?,repo_path=?,default_branch=?,worktree_root=?,updated_at=? WHERE id=?")
                .bind(name)
                .bind(repo_path)
                .bind(default_branch)
                .bind(worktree_root)
                .bind(&now)
                .bind(&existing_id)
                .execute(&self.pool)
                .await?;
            let row = sqlx::query("SELECT id,seq,name,repo_path,default_branch,worktree_root,created_at FROM projects WHERE id=?")
                .bind(existing_id)
                .fetch_one(&self.pool)
                .await?;
            return Ok(project_from_row(row));
        }
        let id = Uuid::now_v7().to_string();
        let seq: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM projects")
            .fetch_one(&self.pool)
            .await?;
        let row = sqlx::query("INSERT INTO projects(id,seq,name,repo_path,default_branch,worktree_root,repo_identity,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?) ON CONFLICT(repo_path) DO UPDATE SET name=excluded.name,default_branch=excluded.default_branch,worktree_root=excluded.worktree_root,repo_identity=COALESCE(excluded.repo_identity,projects.repo_identity),updated_at=excluded.updated_at RETURNING id,seq,name,repo_path,default_branch,worktree_root,created_at")
            .bind(&id).bind(seq).bind(name).bind(repo_path).bind(default_branch).bind(worktree_root).bind(repo_identity).bind(&now).bind(&now).fetch_one(&self.pool).await?;
        Ok(project_from_row(row))
    }

    pub async fn projects(&self) -> Result<Vec<Project>, PersistenceError> {
        let rows = sqlx::query("SELECT id,seq,name,repo_path,default_branch,worktree_root,created_at FROM projects ORDER BY seq").fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|r| Project {
                id: r.get("id"),
                seq: r.get("seq"),
                name: r.get("name"),
                repo_path: r.get("repo_path"),
                default_branch: r.get("default_branch"),
                worktree_root: r.get("worktree_root"),
                created_at: r.get("created_at"),
            })
            .collect())
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_task(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: &str,
        max_revisions: i64,
    ) -> Result<TaskSummary, PersistenceError> {
        self.create_task_with_api_egress(
            project_id,
            title,
            description,
            developer,
            reviewer,
            target_branch,
            max_revisions,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_task_with_api_egress(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: &str,
        max_revisions: i64,
        allow_api_egress: bool,
    ) -> Result<TaskSummary, PersistenceError> {
        let policy = TaskPolicy {
            require_plan_approval: false,
            ..TaskPolicy::default()
        };
        self.create_governed_task_with_api_egress(
            project_id,
            title,
            description,
            developer,
            reviewer,
            target_branch,
            max_revisions,
            allow_api_egress,
            &policy,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn create_governed_task_with_api_egress(
        &self,
        project_id: &str,
        title: &str,
        description: &str,
        developer: AgentKind,
        reviewer: AgentKind,
        target_branch: &str,
        max_revisions: i64,
        allow_api_egress: bool,
        policy: &TaskPolicy,
    ) -> Result<TaskSummary, PersistenceError> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::now_v7().to_string();
        let seq: i64 =
            sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM tasks WHERE project_id=?")
                .bind(project_id)
                .fetch_one(&self.pool)
                .await?;
        let mut tx = self.pool.begin().await?;
        sqlx::query("INSERT INTO tasks(id,project_id,seq,title,description,status,developer_agent,reviewer_agent,target_branch,max_revisions,api_egress_approved_at,created_at,updated_at) VALUES(?,?,?,?,?,'DRAFT',?,?,?,?,?,?,?)")
            .bind(&id).bind(project_id).bind(seq).bind(title).bind(description).bind(developer.to_string()).bind(reviewer.to_string()).bind(target_branch).bind(max_revisions).bind(allow_api_egress.then_some(&now)).bind(&now).bind(&now).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO task_policies(task_id,require_plan_approval,priority,token_budget,cost_budget_usd,time_budget_secs,minimum_quality_score,delivery_mode,execution_node_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?)")
            .bind(&id)
            .bind(i64::from(policy.require_plan_approval))
            .bind(i64::from(policy.priority))
            .bind(policy.token_budget)
            .bind(policy.cost_budget_usd)
            .bind(policy.time_budget_secs)
            .bind(i64::from(policy.minimum_quality_score))
            .bind(policy.delivery_mode.to_string())
            .bind(&policy.execution_node_id)
            .bind(&now)
            .bind(&now)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.task_summary(&id).await
    }

    pub async fn task_summary(&self, task_id: &str) -> Result<TaskSummary, PersistenceError> {
        let r = sqlx::query("SELECT id,project_id,seq,title,status,blocked_reason,current_revision,developer_agent,reviewer_agent,updated_at FROM tasks WHERE id=? AND deleted_at IS NULL").bind(task_id).fetch_one(&self.pool).await?;
        Ok(TaskSummary {
            id: r.get("id"),
            project_id: r.get("project_id"),
            seq: r.get("seq"),
            title: r.get("title"),
            status: parse(r.get::<String, _>("status"))?,
            blocked_reason: parse_opt(r.get("blocked_reason"))?,
            current_revision: r.get("current_revision"),
            developer_agent: parse(r.get::<String, _>("developer_agent"))?,
            reviewer_agent: parse(r.get::<String, _>("reviewer_agent"))?,
            updated_at: r.get("updated_at"),
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn transition(
        &self,
        task_id: &str,
        allowed: &[TaskStatus],
        to: TaskStatus,
        blocked_reason: Option<BlockedReason>,
        actor: Actor,
        event_type: &str,
        payload: &Value,
    ) -> Result<TaskEvent, PersistenceError> {
        let mut tx = self.pool.begin().await?;
        let current = sqlx::query("SELECT status,current_revision FROM tasks WHERE id=?")
            .bind(task_id)
            .fetch_one(&mut *tx)
            .await?;
        let actual: TaskStatus = parse(current.get("status"))?;
        let revision: i64 = current.get("current_revision");
        if !allowed.contains(&actual) {
            return Err(PersistenceError::InvalidState {
                actual,
                expected: allowed.to_vec(),
            });
        }
        let now = Utc::now().to_rfc3339();
        sqlx::query("UPDATE tasks SET status=?, blocked_reason=?, updated_at=? WHERE id=?")
            .bind(to.to_string())
            .bind(blocked_reason.map(|v| v.to_string()))
            .bind(&now)
            .bind(task_id)
            .execute(&mut *tx)
            .await?;
        let result = sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,?,?,?,?,?)")
            .bind(task_id).bind(revision).bind(actor.to_string()).bind(event_type).bind(payload.to_string()).bind(&now).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(TaskEvent {
            id: result.last_insert_rowid(),
            task_id: Some(task_id.into()),
            run_id: None,
            revision: Some(revision),
            actor,
            event_type: event_type.into(),
            payload: payload.clone(),
            created_at: now,
        })
    }

    pub async fn events(
        &self,
        task_id: &str,
        after_id: i64,
        limit: i64,
    ) -> Result<Vec<TaskEvent>, PersistenceError> {
        let rows = sqlx::query("SELECT id,task_id,run_id,revision,actor,event_type,payload_json,created_at FROM events WHERE task_id=? AND id>? ORDER BY id LIMIT ?").bind(task_id).bind(after_id).bind(limit.clamp(1, 1000)).fetch_all(&self.pool).await?;
        rows.into_iter()
            .map(|r| {
                Ok(TaskEvent {
                    id: r.get("id"),
                    task_id: r.get("task_id"),
                    run_id: r.get("run_id"),
                    revision: r.get("revision"),
                    // Early API-egress consent events used `user`; keep them readable while all
                    // new writes use the canonical Actor::Human value (`human`).
                    actor: parse_event_actor(r.get::<String, _>("actor"))?,
                    event_type: r.get("event_type"),
                    payload: serde_json::from_str(&r.get::<String, _>("payload_json"))
                        .unwrap_or(Value::Null),
                    created_at: r.get("created_at"),
                })
            })
            .collect()
    }
}

fn project_from_row(row: sqlx::sqlite::SqliteRow) -> Project {
    Project {
        id: row.get("id"),
        seq: row.get("seq"),
        name: row.get("name"),
        repo_path: row.get("repo_path"),
        default_branch: row.get("default_branch"),
        worktree_root: row.get("worktree_root"),
        created_at: row.get("created_at"),
    }
}

fn parse<T: FromStr<Err = String>>(value: String) -> Result<T, PersistenceError> {
    value.parse().map_err(PersistenceError::InvalidValue)
}

fn parse_event_actor(value: String) -> Result<Actor, PersistenceError> {
    if value == "user" {
        return Ok(Actor::Human);
    }
    parse(value)
}
fn parse_opt<T: FromStr<Err = String>>(
    value: Option<String>,
) -> Result<Option<T>, PersistenceError> {
    value.map(parse).transpose()
}

#[cfg(test)]
mod tests;
