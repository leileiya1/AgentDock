use agentflow_contracts::{
    AgentKind, AppError, StorageCleanupScope, TaskDetail, TaskEvent, TaskSummary,
};
use agentflow_daemon::{DaemonRequest, DaemonResponse, request as daemon_request};
use agentflow_orchestrator::Orchestrator;
use serde::Deserialize;
use specta::Type;
use specta_typescript::Typescript;
use std::{path::PathBuf, sync::Arc};
use tauri::{Manager, State};
use tauri_specta::{Builder, collect_commands};

#[macro_use]
mod provider_setup;
mod daemon_client;
mod error;
mod event_bridge;
mod governance_commands;
mod management_commands;
mod view_types;
#[macro_use]
mod repair_commands;
use daemon_client::{ensure_daemon, mutate as daemon_mutate};
use error::app_error;
use governance_commands::*;
use management_commands::*;
use view_types::ExportPath;
use provider_setup::{
    api_credential_delete, api_credential_set, cli_credential_delete, cli_credential_set,
    cli_install,
};
use repair_commands::{task_repair_apply, task_repair_inspect};

struct Backend(Arc<Orchestrator>, PathBuf, tokio::sync::Mutex<()>);

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct ProjectImportArgs {
    path: String,
}
#[derive(Deserialize, Type)]
struct CliPathArgs {
    tool: String,
    path: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct ProjectIdArgs {
    project_id: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct TaskIdArgs {
    task_id: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct TaskCreateArgs {
    project_id: String,
    title: String,
    description: String,
    developer_agent: AgentKind,
    reviewer_agent: AgentKind,
    target_branch: Option<String>,
    max_revisions: Option<i32>,
    allow_api_egress: bool,
    policy: agentflow_contracts::TaskPolicy,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct GuidanceArgs {
    task_id: String,
    guidance: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct RejectArgs {
    task_id: String,
    revision: i32,
    reason: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct ApproveArgs {
    task_id: String,
    revision: i32,
    commit_sha: String,
    diff_sha256: String,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct DiffArgs {
    task_id: String,
    revision: i32,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct EventsArgs {
    task_id: String,
    after_id: Option<i32>,
    limit: Option<i32>,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct ProjectSettingsArgs {
    project_id: String,
    patch: agentflow_contracts::ProjectSettings,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct GlobalSettingsArgs {
    patch: agentflow_contracts::GlobalSettings,
}
#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
struct TaskCleanupArgs {
    task_id: String,
    scope: StorageCleanupScope,
}
#[tauri::command]
#[specta::specta]
async fn env_check(state: State<'_, Backend>) -> Result<agentflow_contracts::EnvReport, AppError> {
    Ok(state.0.env_check().await)
}

#[tauri::command]
#[specta::specta]
async fn provider_list(
    state: State<'_, Backend>,
) -> Result<Vec<agentflow_contracts::ProviderDescriptor>, AppError> {
    Ok(state.0.provider_list().await)
}
#[tauri::command]
#[specta::specta]
async fn onboarding_check(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::OnboardingReport, AppError> {
    let daemon_running = matches!(
        daemon_request(&state.1, &DaemonRequest::Ping).await,
        Ok(DaemonResponse::Ok { .. })
    );
    state
        .0
        .onboarding_check(daemon_running)
        .await
        .map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn onboarding_complete(state: State<'_, Backend>) -> Result<(), AppError> {
    daemon_mutate(&state, DaemonRequest::OnboardingComplete).await
}
#[tauri::command]
#[specta::specta]
async fn storage_report(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::StorageReport, AppError> {
    state.0.storage_report().await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn storage_cleanup(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::CleanupResult, AppError> {
    daemon_mutate(&state, DaemonRequest::StorageCleanup).await
}
#[tauri::command]
#[specta::specta]
async fn task_cleanup(
    state: State<'_, Backend>,
    args: TaskCleanupArgs,
) -> Result<agentflow_contracts::CleanupResult, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskCleanup {
            task_id: args.task_id,
            scope: args.scope,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn trash_list(
    state: State<'_, Backend>,
) -> Result<Vec<agentflow_contracts::TrashEntry>, AppError> {
    state.0.trash_list().await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn task_restore(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<TaskSummary, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskRestore {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn trash_empty(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::CleanupResult, AppError> {
    daemon_mutate(&state, DaemonRequest::TrashEmpty).await
}
#[tauri::command]
#[specta::specta]
async fn env_set_cli_path(
    state: State<'_, Backend>,
    args: CliPathArgs,
) -> Result<agentflow_contracts::EnvReport, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::EnvSetCliPath {
            tool: args.tool,
            path: args.path,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn project_import(
    state: State<'_, Backend>,
    args: ProjectImportArgs,
) -> Result<agentflow_contracts::Project, AppError> {
    daemon_mutate(&state, DaemonRequest::ProjectImport { path: args.path }).await
}
#[tauri::command]
#[specta::specta]
async fn project_list(
    state: State<'_, Backend>,
) -> Result<Vec<agentflow_contracts::Project>, AppError> {
    state.0.project_list().await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn task_create(
    state: State<'_, Backend>,
    args: TaskCreateArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskCreate {
            project_id: args.project_id,
            title: args.title,
            description: args.description,
            developer_agent: args.developer_agent,
            reviewer_agent: args.reviewer_agent,
            target_branch: args.target_branch,
            max_revisions: args.max_revisions.map(i64::from),
            allow_api_egress: args.allow_api_egress,
            policy: args.policy,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_list(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<Vec<TaskSummary>, AppError> {
    state.0.task_list(&args.project_id).await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn task_get(state: State<'_, Backend>, args: TaskIdArgs) -> Result<TaskDetail, AppError> {
    state.0.task_get(&args.task_id).await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn task_start(state: State<'_, Backend>, args: TaskIdArgs) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskStart {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_cancel(state: State<'_, Backend>, args: TaskIdArgs) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskCancel {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_resume_with_guidance(
    state: State<'_, Backend>,
    args: GuidanceArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskResumeWithGuidance {
            task_id: args.task_id,
            guidance: args.guidance,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_force_approve(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskForceApprove {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_approve(
    state: State<'_, Backend>,
    args: ApproveArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskApprove {
            task_id: args.task_id,
            revision: i64::from(args.revision),
            commit_sha: args.commit_sha,
            diff_sha256: args.diff_sha256,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_reject(state: State<'_, Backend>, args: RejectArgs) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskReject {
            task_id: args.task_id,
            revision: i64::from(args.revision),
            reason: args.reason,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_merge(state: State<'_, Backend>, args: TaskIdArgs) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskMerge {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn task_mark_merged_external(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<TaskDetail, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::TaskMarkMergedExternal {
            task_id: args.task_id,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn diff_get(
    state: State<'_, Backend>,
    args: DiffArgs,
) -> Result<agentflow_contracts::DiffPayload, AppError> {
    state
        .0
        .diff_get(&args.task_id, i64::from(args.revision))
        .await
        .map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn run_list(
    state: State<'_, Backend>,
    args: TaskIdArgs,
) -> Result<Vec<agentflow_contracts::RunSummary>, AppError> {
    state.0.run_list(&args.task_id).await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn events_list(
    state: State<'_, Backend>,
    args: EventsArgs,
) -> Result<Vec<TaskEvent>, AppError> {
    state
        .0
        .events_list(
            &args.task_id,
            args.after_id.map(i64::from),
            args.limit.map(i64::from),
        )
        .await
        .map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn events_export(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<ExportPath, AppError> {
    state
        .0
        .events_export(&args.project_id)
        .await
        .map(|p| ExportPath {
            path: p.to_string_lossy().into_owned(),
        })
        .map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn project_settings_get(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<agentflow_contracts::ProjectSettings, AppError> {
    state
        .0
        .project_settings_get(&args.project_id)
        .await
        .map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn project_settings_update(
    state: State<'_, Backend>,
    args: ProjectSettingsArgs,
) -> Result<agentflow_contracts::ProjectSettings, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectSettingsUpdate {
            project_id: args.project_id,
            settings: Box::new(args.patch),
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn settings_get(
    state: State<'_, Backend>,
) -> Result<agentflow_contracts::GlobalSettings, AppError> {
    state.0.settings_get().await.map_err(app_error)
}
#[tauri::command]
#[specta::specta]
async fn settings_update(
    state: State<'_, Backend>,
    args: GlobalSettingsArgs,
) -> Result<agentflow_contracts::GlobalSettings, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::SettingsUpdate {
            settings: args.patch,
        },
    )
    .await
}
#[tauri::command]
#[specta::specta]
async fn review_get(
    state: State<'_, Backend>,
    args: DiffArgs,
) -> Result<Option<agentflow_contracts::Review>, AppError> {
    state
        .0
        .review_get(&args.task_id, i64::from(args.revision))
        .await
        .map_err(app_error)
}
fn command_builder() -> Builder<tauri::Wry> {
    Builder::<tauri::Wry>::new().commands(collect_commands![
        env_check,
        provider_list,
        onboarding_check,
        onboarding_complete,
        storage_report,
        storage_cleanup,
        database_backup_list,
        database_backup_create,
        database_backup_restore,
        task_cleanup,
        trash_list,
        task_restore,
        trash_empty,
        env_set_cli_path,
        cli_install,
        cli_credential_set,
        cli_credential_delete,
        api_credential_set,
        api_credential_delete,
        project_import,
        project_list,
        project_git_compatibility,
        task_create,
        task_list,
        task_get,
        task_start,
        task_cancel,
        queue_task_pause,
        queue_task_resume,
        queue_task_priority,
        task_resume_with_guidance,
        task_repair_inspect,
        task_repair_apply,
        task_force_approve,
        task_approve,
        task_reject,
        task_merge,
        task_mark_merged_external,
        task_plan_approve,
        task_plan_reject,
        task_budget_update,
        task_governance_get,
        task_quality_replay,
        task_delivery_start,
        task_delivery_refresh,
        task_rollback,
        execution_node_list,
        execution_node_upsert,
        execution_node_check,
        execution_node_delete,
        diff_get,
        run_list,
        events_list,
        events_export,
        project_settings_get,
        project_settings_update,
        project_config_trust_get,
        project_config_trust_approve,
        project_config_trust_revoke,
        settings_get,
        settings_update,
        review_get,
        run_log_tail,
    ])
}

fn main() {
    let builder = command_builder();
    if std::env::args().any(|arg| arg == "--export-bindings") {
        // Only the explicit xtask may update checked-in bindings. Otherwise launching an older
        // debug app can silently overwrite newer generated types in the shared source tree.
        builder
            .export(
                Typescript::default(),
                concat!(env!("CARGO_MANIFEST_DIR"), "/../src/generated/bindings.ts"),
            )
            .expect("failed to export Tauri TypeScript bindings");
        return;
    }
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(builder.invoke_handler())
        .setup(move |app| {
            builder.mount_events(app);
            let path = app.path().app_data_dir()?;
            tauri::async_runtime::block_on(ensure_daemon(&path))?;
            let orchestrator = tauri::async_runtime::block_on(Orchestrator::open_client(&path))
                .map_err(Box::<dyn std::error::Error>::from)?;
            let orchestrator = Arc::new(orchestrator);
            event_bridge::spawn(app.handle().clone(), Arc::clone(&orchestrator));
            app.manage(Backend(orchestrator, path, tokio::sync::Mutex::new(())));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("AgentFlow desktop failed");
}
