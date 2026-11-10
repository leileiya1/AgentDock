//! Town customisation: project appearance, assigned members and agent profiles.
//! Reads go straight to the database; writes are daemon-owned like every other mutation.

use super::*;
use agentflow_contracts::{
    AgentProfile, AgentProfilePatch, ProjectAppearance, ProjectAppearancePatch, ProjectMember,
    ProjectMemberInput,
};
use serde::Deserialize;

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectAppearanceArgs {
    project_id: String,
    patch: ProjectAppearancePatch,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct ProjectMembersArgs {
    project_id: String,
    members: Vec<ProjectMemberInput>,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentProfileArgs {
    agent: AgentKind,
    patch: AgentProfilePatch,
}

#[derive(Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub(super) struct AgentArgs {
    agent: AgentKind,
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_appearance_list(
    state: State<'_, Backend>,
) -> Result<Vec<ProjectAppearance>, AppError> {
    state.0.project_appearance_list().await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_appearance_update(
    state: State<'_, Backend>,
    args: ProjectAppearanceArgs,
) -> Result<ProjectAppearance, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectAppearanceUpdate {
            project_id: args.project_id,
            patch: args.patch,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_members_get(
    state: State<'_, Backend>,
    args: ProjectIdArgs,
) -> Result<Vec<ProjectMember>, AppError> {
    state
        .0
        .project_members_get(&args.project_id)
        .await
        .map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn project_members_set(
    state: State<'_, Backend>,
    args: ProjectMembersArgs,
) -> Result<Vec<ProjectMember>, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::ProjectMembersSet {
            project_id: args.project_id,
            members: args.members,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn agent_profiles_list(
    state: State<'_, Backend>,
) -> Result<Vec<AgentProfile>, AppError> {
    state.0.agent_profiles_list().await.map_err(app_error)
}

#[tauri::command]
#[specta::specta]
pub(super) async fn agent_profile_update(
    state: State<'_, Backend>,
    args: AgentProfileArgs,
) -> Result<AgentProfile, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::AgentProfileUpdate {
            agent: args.agent,
            patch: args.patch,
        },
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub(super) async fn agent_profile_reset(
    state: State<'_, Backend>,
    args: AgentArgs,
) -> Result<bool, AppError> {
    daemon_mutate(
        &state,
        DaemonRequest::AgentProfileReset { agent: args.agent },
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    // These payloads are exactly what the generated TypeScript bindings send over IPC.

    #[test]
    fn appearance_args_keep_absent_and_null_apart() -> TestResult {
        let args: ProjectAppearanceArgs = serde_json::from_value(json!({
            "projectId": "p1",
            "patch": { "roof": null, "sign": "neon", "yard": ["fountain"] }
        }))?;
        assert_eq!(args.project_id, "p1");
        let patch = serde_json::to_value(&args.patch)?;
        assert_eq!(
            patch["roof"],
            serde_json::Value::Null,
            "explicit null must clear the roof"
        );
        assert!(patch.get("kind").is_none(), "absent kind must stay absent");
        assert_eq!(patch["sign"], "neon");
        assert_eq!(patch["yard"], json!(["fountain"]));
        Ok(())
    }

    #[test]
    fn members_and_profile_args_match_the_bindings() -> TestResult {
        let members: ProjectMembersArgs = serde_json::from_value(json!({
            "projectId": "p1",
            "members": [{ "agent": "claude_code", "role": "developer" }, { "agent": "codex", "role": "both" }]
        }))?;
        assert_eq!(members.members.len(), 2);

        let profile: AgentProfileArgs = serde_json::from_value(json!({
            "agent": "kimi_cli",
            "patch": { "nickname": "小咪", "accessory": null }
        }))?;
        let patch = serde_json::to_value(&profile.patch)?;
        assert_eq!(patch["nickname"], "小咪");
        assert_eq!(patch["accessory"], serde_json::Value::Null);
        assert!(patch.get("color").is_none());

        let reset: AgentArgs = serde_json::from_value(json!({ "agent": "codex" }))?;
        assert_eq!(serde_json::to_value(&reset.agent)?, "codex");
        Ok(())
    }

    #[test]
    fn unknown_member_role_is_rejected_at_the_boundary() {
        let bad = serde_json::from_value::<ProjectMembersArgs>(json!({
            "projectId": "p1",
            "members": [{ "agent": "codex", "role": "boss" }]
        }));
        assert!(bad.is_err());
    }
}
