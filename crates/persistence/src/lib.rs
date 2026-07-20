use std::{path::Path, str::FromStr};

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
}

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
}

impl Store {
    pub async fn open(path: &Path) -> Result<Self, PersistenceError> {
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(sqlx::Error::Io)?;
        }
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
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn in_memory() -> Result<Self, PersistenceError> {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")?.foreign_keys(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect_with(options)
            .await?;
        sqlx::migrate!().run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    pub async fn import_project(
        &self,
        name: &str,
        repo_path: &str,
        default_branch: &str,
        worktree_root: &str,
    ) -> Result<Project, PersistenceError> {
        let now = Utc::now().to_rfc3339();
        let id = Uuid::now_v7().to_string();
        let seq: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(seq), 0) + 1 FROM projects")
            .fetch_one(&self.pool)
            .await?;
        let row = sqlx::query("INSERT INTO projects(id,seq,name,repo_path,default_branch,worktree_root,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?) ON CONFLICT(repo_path) DO UPDATE SET name=excluded.name,default_branch=excluded.default_branch,worktree_root=excluded.worktree_root,updated_at=excluded.updated_at RETURNING id,seq,name,repo_path,default_branch,worktree_root,created_at")
            .bind(&id).bind(seq).bind(name).bind(repo_path).bind(default_branch).bind(worktree_root).bind(&now).bind(&now).fetch_one(&self.pool).await?;
        Ok(Project {
            id: row.get("id"),
            seq: row.get("seq"),
            name: row.get("name"),
            repo_path: row.get("repo_path"),
            default_branch: row.get("default_branch"),
            worktree_root: row.get("worktree_root"),
            created_at: row.get("created_at"),
        })
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
        sqlx::query("INSERT INTO task_policies(task_id,require_plan_approval,token_budget,cost_budget_usd,time_budget_secs,minimum_quality_score,delivery_mode,execution_node_id,created_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?)")
            .bind(&id)
            .bind(i64::from(policy.require_plan_approval))
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
mod tests {
    use super::*;
    use serde_json::json;

    #[tokio::test]
    async fn transition_and_event_are_atomic_and_invalid_transition_changes_nothing()
    -> Result<(), Box<dyn std::error::Error>> {
        let store = Store::in_memory().await?;
        let p = store
            .import_project("p", "/tmp/p", "main", "/tmp/w")
            .await?;
        let t = store
            .create_task(
                &p.id,
                "t",
                "d",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "main",
                3,
            )
            .await?;
        store
            .transition(
                &t.id,
                &[TaskStatus::Draft],
                TaskStatus::ReadyForDevelopment,
                None,
                Actor::Human,
                "user:start",
                &json!({}),
            )
            .await?;
        let events = store.events(&t.id, 0, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].revision, Some(0));
        assert!(
            store
                .transition(
                    &t.id,
                    &[TaskStatus::Draft],
                    TaskStatus::Cancelled,
                    None,
                    Actor::Human,
                    "bad",
                    &json!({})
                )
                .await
                .is_err()
        );
        assert_eq!(
            store.task_summary(&t.id).await?.status,
            TaskStatus::ReadyForDevelopment
        );
        assert_eq!(store.events(&t.id, 0, 10).await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn importing_the_same_repo_reuses_the_project() -> Result<(), Box<dyn std::error::Error>>
    {
        let store = Store::in_memory().await?;
        let first = store
            .import_project("p", "/tmp/p", "main", "/tmp/w")
            .await?;
        let second = store
            .import_project("renamed", "/tmp/p", "trunk", "/tmp/w2")
            .await?;
        assert_eq!(first.id, second.id);
        assert_eq!(first.seq, second.seq);
        assert_eq!(second.name, "renamed");
        assert_eq!(second.default_branch, "trunk");
        assert_eq!(store.projects().await?.len(), 1);
        Ok(())
    }

    #[tokio::test]
    async fn legacy_user_event_actor_is_read_as_human() -> Result<(), Box<dyn std::error::Error>> {
        let store = Store::in_memory().await?;
        let p = store
            .import_project("p", "/tmp/p", "main", "/tmp/w")
            .await?;
        let t = store
            .create_task(
                &p.id,
                "t",
                "d",
                AgentKind::ClaudeCode,
                AgentKind::Codex,
                "main",
                3,
            )
            .await?;
        sqlx::query("INSERT INTO events(task_id,revision,actor,event_type,payload_json,created_at) VALUES(?,0,'user','privacy:api_egress_approved','{}','now')")
            .bind(&t.id)
            .execute(store.pool())
            .await?;

        let events = store.events(&t.id, 0, 10).await?;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].actor, Actor::Human);
        Ok(())
    }
}
