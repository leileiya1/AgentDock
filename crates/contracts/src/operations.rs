string_enum!(RepairAction {
    RebuildWorktree => "rebuild_worktree",
    ResumeResidual => "resume_residual",
    ResetToCheckpoint => "reset_to_checkpoint"
});

string_enum!(QueueState {
    Queued => "QUEUED",
    Running => "RUNNING",
    Completed => "COMPLETED",
    Failed => "FAILED"
});

string_enum!(QueueWaitingReason {
    Paused => "paused",
    SchedulerPaused => "scheduler_paused",
    OutsideRunWindow => "outside_run_window",
    DailyBudgetExhausted => "daily_budget_exhausted",
    RetryDelay => "retry_delay",
    ConcurrencyLimit => "concurrency_limit",
    TasksAhead => "tasks_ahead"
});

/// Authoritative scheduler state persisted in `daemon_queue`.
///
/// The desktop must query this record instead of inferring queue state from a
/// task status or retaining the last mutation response in memory. That keeps
/// pause, priority and position accurate after either process restarts.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueTaskState {
    pub task_id: String,
    pub state: QueueState,
    pub paused: bool,
    #[specta(type = i32)]
    pub priority: i16,
    #[specta(type = Option<u32>)]
    pub position: Option<u32>,
    pub waiting_reason: Option<QueueWaitingReason>,
    pub not_before: Option<String>,
    pub last_error: Option<String>,
    #[specta(type = u32)]
    pub attempts: u32,
    pub enqueued_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct TaskCheckpoint {
    pub id: String,
    #[specta(type = i32)]
    pub revision: i64,
    pub phase: String,
    pub commit_sha: String,
    pub patch_sha256: Option<String>,
    #[specta(type = i32)]
    pub untracked_files: i64,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct RepairReport {
    pub task_id: String,
    pub status: TaskStatus,
    pub blocked_reason: Option<BlockedReason>,
    pub worktree_exists: bool,
    pub residual_changes: bool,
    pub latest_checkpoint: Option<TaskCheckpoint>,
    pub actions: Vec<RepairAction>,
}

string_enum!(AuditExportScope {
    Project => "project",
    Task => "task"
});

/// Result of a local, redacted audit export. `redacted` means the export policy
/// was applied; callers must never interpret it as proof that the source held a secret.
#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema, Type)]
#[serde(rename_all = "camelCase")]
pub struct AuditExportResult {
    pub path: String,
    pub scope: AuditExportScope,
    pub project_id: String,
    pub task_id: Option<String>,
    pub event_count: u32,
    pub task_count: u32,
    #[specta(type = f64)]
    pub bytes: u64,
    pub redacted: bool,
    pub created_at: String,
}
