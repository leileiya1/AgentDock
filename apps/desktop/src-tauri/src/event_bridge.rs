use agentflow_contracts::{
    AgentEvent, AgentKind, BlockedReason, ErrorCode, RunRole, RunStatus, RunSummary, TaskStatus,
    TaskSummary,
};
use agentflow_orchestrator::{Orchestrator, OrchestratorError};
use serde::Serialize;
use sqlx::Row;
use std::{collections::HashMap, path::Path, str::FromStr, sync::Arc, time::Duration};
use tauri::{AppHandle, Emitter};
use tokio::io::{AsyncBufReadExt, BufReader};

const POLL_INTERVAL: Duration = Duration::from_millis(300);
const LOG_BATCH_SIZE: usize = 250;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskChangedPayload {
    task: TaskSummary,
    from: TaskStatus,
    to: TaskStatus,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TaskRemovedPayload {
    task_id: String,
    project_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RunLogPayload {
    run_id: String,
    batch: Vec<AgentEvent>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AppErrorPayload {
    scope: &'static str,
    code: ErrorCode,
    message: String,
}

#[derive(Debug)]
enum BridgeEvent {
    TaskChanged {
        task: TaskSummary,
        from: TaskStatus,
    },
    TaskRemoved {
        task_id: String,
        project_id: String,
    },
    RunStarted(RunSummary),
    RunLog {
        run_id: String,
        batch: Vec<AgentEvent>,
    },
    RunFinished(RunSummary),
}

impl BridgeEvent {
    fn emit(self, app: &AppHandle) -> tauri::Result<()> {
        match self {
            Self::TaskChanged { task, from } => app.emit(
                "task:changed",
                TaskChangedPayload {
                    to: task.status,
                    task,
                    from,
                },
            ),
            Self::TaskRemoved {
                task_id,
                project_id,
            } => app.emit(
                "task:removed",
                TaskRemovedPayload {
                    task_id,
                    project_id,
                },
            ),
            Self::RunStarted(run) => app.emit("run:started", run),
            Self::RunLog { run_id, batch } => app.emit("run:log", RunLogPayload { run_id, batch }),
            Self::RunFinished(run) => app.emit("run:finished", run),
        }
    }
}

struct BridgeState {
    tasks: HashMap<String, TaskSummary>,
    runs: HashMap<String, RunSummary>,
    log_cursors: HashMap<String, usize>,
}

impl BridgeState {
    async fn initialize(orchestrator: &Orchestrator) -> Result<Self, OrchestratorError> {
        let tasks = load_tasks(orchestrator).await?;
        let runs = load_runs(orchestrator).await?;
        let mut log_cursors = HashMap::new();
        for run in runs.values().filter(|run| run.status == RunStatus::Running) {
            // The UI loads historical lines itself. Start the live cursor at EOF so reopening the
            // desktop never duplicates history, while future output still arrives immediately.
            log_cursors.insert(run.id.clone(), log_line_count(orchestrator, &run.id).await?);
        }
        Ok(Self {
            tasks,
            runs,
            log_cursors,
        })
    }

    async fn poll(
        &mut self,
        orchestrator: &Orchestrator,
    ) -> Result<Vec<BridgeEvent>, OrchestratorError> {
        let mut events = Vec::new();
        let current_tasks = load_tasks(orchestrator).await?;
        for (id, task) in &current_tasks {
            let previous = self.tasks.get(id);
            if previous.is_none_or(|old| task_changed(old, task)) {
                events.push(BridgeEvent::TaskChanged {
                    task: task.clone(),
                    from: previous.map_or(task.status, |old| old.status),
                });
            }
        }
        for (id, old) in &self.tasks {
            if !current_tasks.contains_key(id) {
                events.push(BridgeEvent::TaskRemoved {
                    task_id: id.clone(),
                    project_id: old.project_id.clone(),
                });
            }
        }
        self.tasks = current_tasks;

        let current_runs = load_runs(orchestrator).await?;
        for (id, run) in &current_runs {
            let previous_status = self.runs.get(id).map(|old| old.status);
            let is_new = previous_status.is_none();
            let was_running = previous_status == Some(RunStatus::Running);
            if is_new {
                events.push(BridgeEvent::RunStarted(run.clone()));
                self.log_cursors.insert(id.clone(), 0);
            }

            if run.status == RunStatus::Running || was_running || is_new {
                self.collect_log_events(orchestrator, id, &mut events)
                    .await?;
            }

            if run.status != RunStatus::Running
                && (is_new || previous_status.is_some_and(|status| status != run.status))
            {
                events.push(BridgeEvent::RunFinished(run.clone()));
                self.log_cursors.remove(id);
            }
        }
        self.runs = current_runs;
        Ok(events)
    }

    async fn collect_log_events(
        &mut self,
        orchestrator: &Orchestrator,
        run_id: &str,
        events: &mut Vec<BridgeEvent>,
    ) -> Result<(), OrchestratorError> {
        let cursor = *self.log_cursors.get(run_id).unwrap_or(&0);
        let (batch, next, _) = orchestrator
            .run_log_tail(run_id, cursor, LOG_BATCH_SIZE)
            .await?;
        self.log_cursors.insert(run_id.to_owned(), next);
        if !batch.is_empty() {
            events.push(BridgeEvent::RunLog {
                run_id: run_id.to_owned(),
                batch,
            });
        }
        Ok(())
    }
}

fn task_changed(old: &TaskSummary, new: &TaskSummary) -> bool {
    old.status != new.status
        || old.blocked_reason != new.blocked_reason
        || old.current_revision != new.current_revision
        || old.updated_at != new.updated_at
}

fn parse<T: FromStr<Err = String>>(value: String) -> Result<T, OrchestratorError> {
    value.parse().map_err(OrchestratorError::InvalidState)
}

fn parse_opt<T: FromStr<Err = String>>(
    value: Option<String>,
) -> Result<Option<T>, OrchestratorError> {
    value.map(parse).transpose()
}

async fn load_tasks(
    orchestrator: &Orchestrator,
) -> Result<HashMap<String, TaskSummary>, OrchestratorError> {
    let rows = sqlx::query(
        "SELECT id,project_id,seq,title,status,blocked_reason,current_revision,developer_agent,\
         reviewer_agent,updated_at FROM tasks WHERE deleted_at IS NULL",
    )
    .fetch_all(orchestrator.store.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            let task = TaskSummary {
                id: row.get("id"),
                project_id: row.get("project_id"),
                seq: row.get("seq"),
                title: row.get("title"),
                status: parse::<TaskStatus>(row.get("status"))?,
                blocked_reason: parse_opt::<BlockedReason>(row.get("blocked_reason"))?,
                current_revision: row.get("current_revision"),
                developer_agent: parse::<AgentKind>(row.get("developer_agent"))?,
                reviewer_agent: parse::<AgentKind>(row.get("reviewer_agent"))?,
                updated_at: row.get("updated_at"),
            };
            Ok((task.id.clone(), task))
        })
        .collect()
}

async fn load_runs(
    orchestrator: &Orchestrator,
) -> Result<HashMap<String, RunSummary>, OrchestratorError> {
    let rows = sqlx::query(
        "SELECT id,task_id,revision,role,agent,status,exit_code,cost_usd,tokens_in,tokens_out,started_at,finished_at \
         FROM agent_runs",
    )
    .fetch_all(orchestrator.store.pool())
    .await?;
    rows.into_iter()
        .map(|row| {
            let run = RunSummary {
                id: row.get("id"),
                task_id: row.get("task_id"),
                revision: row.get("revision"),
                role: parse::<RunRole>(row.get("role"))?,
                agent: parse_opt::<AgentKind>(row.get("agent"))?,
                status: parse::<RunStatus>(row.get("status"))?,
                exit_code: row.get("exit_code"),
                cost_usd: row.get("cost_usd"),
                tokens_in: row.get("tokens_in"),
                tokens_out: row.get("tokens_out"),
                started_at: row.get("started_at"),
                finished_at: row.get("finished_at"),
            };
            Ok((run.id.clone(), run))
        })
        .collect()
}

async fn log_line_count(
    orchestrator: &Orchestrator,
    run_id: &str,
) -> Result<usize, OrchestratorError> {
    let run_dir: String = sqlx::query_scalar("SELECT run_dir FROM agent_runs WHERE id=?")
        .bind(run_id)
        .fetch_one(orchestrator.store.pool())
        .await?;
    let path = Path::new(&run_dir).join("agent-events.jsonl");
    let Ok(file) = tokio::fs::File::open(path).await else {
        return Ok(0);
    };
    let mut reader = BufReader::new(file);
    let mut buffer = String::new();
    let mut lines = 0;
    while reader.read_line(&mut buffer).await? != 0 {
        lines += 1;
        buffer.clear();
    }
    Ok(lines)
}

pub fn spawn(app: AppHandle, orchestrator: Arc<Orchestrator>) {
    tauri::async_runtime::spawn(async move {
        let mut state = match BridgeState::initialize(&orchestrator).await {
            Ok(state) => state,
            Err(error) => {
                let _ = app.emit(
                    "app:error",
                    AppErrorPayload {
                        scope: "event_bridge",
                        code: ErrorCode::DbError,
                        message: format!("实时状态初始化失败：{error}"),
                    },
                );
                return;
            }
        };
        let mut last_error: Option<String> = None;
        loop {
            tokio::time::sleep(POLL_INTERVAL).await;
            match state.poll(&orchestrator).await {
                Ok(events) => {
                    last_error = None;
                    for event in events {
                        let _ = event.emit(&app);
                    }
                }
                Err(error) => {
                    let message = error.to_string();
                    if last_error.as_deref() != Some(message.as_str()) {
                        let _ = app.emit(
                            "app:error",
                            AppErrorPayload {
                                scope: "event_bridge",
                                code: ErrorCode::DbError,
                                message: format!("实时状态暂时中断：{message}"),
                            },
                        );
                        last_error = Some(message);
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use serde_json::json;

    #[tokio::test]
    async fn bridge_emits_task_run_log_and_finish_changes() -> Result<(), Box<dyn std::error::Error>>
    {
        let root = tempfile::tempdir()?;
        let orchestrator = Orchestrator::open(root.path()).await?;
        let mut state = BridgeState::initialize(&orchestrator).await?;
        let project = orchestrator
            .store
            .import_project("test", "/tmp/test", "main", "/tmp/wt")
            .await?;
        let task = orchestrator
            .store
            .create_task(
                &project.id,
                "live",
                "live events",
                AgentKind::Codex,
                AgentKind::ClaudeCode,
                "main",
                3,
            )
            .await?;
        assert!(matches!(
            state.poll(&orchestrator).await?.as_slice(),
            [BridgeEvent::TaskChanged { .. }]
        ));

        orchestrator
            .store
            .transition(
                &task.id,
                &[TaskStatus::Draft],
                TaskStatus::Developing,
                None,
                agentflow_contracts::Actor::Orchestrator,
                "test:start",
                &json!({}),
            )
            .await?;
        let run_dir = root.path().join("run-live");
        tokio::fs::create_dir_all(&run_dir).await?;
        let log = AgentEvent {
            ts: Utc::now().to_rfc3339(),
            stream: agentflow_contracts::EventStream::Stdout,
            kind: agentflow_contracts::AgentEventKind::AssistantText,
            summary: "正在修改核心逻辑".into(),
            text: None,
        };
        tokio::fs::write(
            run_dir.join("agent-events.jsonl"),
            format!("{}\n", serde_json::to_string(&log)?),
        )
        .await?;
        let now = Utc::now().to_rfc3339();
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,started_at,created_at) VALUES('run-live',?,1,'developer','codex','RUNNING',?,30,30,?,?)")
            .bind(&task.id)
            .bind(run_dir.to_string_lossy().as_ref())
            .bind(&now)
            .bind(&now)
            .execute(orchestrator.store.pool())
            .await?;
        let events = state.poll(&orchestrator).await?;
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BridgeEvent::TaskChanged { .. }))
        );
        assert!(
            events
                .iter()
                .any(|event| matches!(event, BridgeEvent::RunStarted(_)))
        );
        assert!(
            events.iter().any(
                |event| matches!(event, BridgeEvent::RunLog { batch, .. } if batch.len() == 1)
            )
        );

        sqlx::query("UPDATE agent_runs SET status='SUCCEEDED',finished_at=? WHERE id='run-live'")
            .bind(Utc::now().to_rfc3339())
            .execute(orchestrator.store.pool())
            .await?;
        assert!(
            state
                .poll(&orchestrator)
                .await?
                .iter()
                .any(|event| matches!(event, BridgeEvent::RunFinished(_)))
        );
        Ok(())
    }
}
