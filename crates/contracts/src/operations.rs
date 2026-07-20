string_enum!(RepairAction {
    RebuildWorktree => "rebuild_worktree",
    ResumeResidual => "resume_residual",
    ResetToCheckpoint => "reset_to_checkpoint"
});

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
