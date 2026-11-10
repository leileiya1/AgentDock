//! Town customisation: validation rules and real-SQLite round trips.

use agentflow_contracts::{
    AgentKind, AgentProfile, AgentProfilePatch, AgentsSource, MemberRole, ProjectAppearance,
    ProjectAppearancePatch, ProjectMemberInput,
};

use crate::{Orchestrator, OrchestratorError, apply_appearance_patch, apply_profile_patch};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn member(agent: AgentKind, role: MemberRole) -> ProjectMemberInput {
    ProjectMemberInput { agent, role }
}

fn blank_profile(agent: AgentKind) -> AgentProfile {
    AgentProfile {
        agent,
        nickname: None,
        color: None,
        accessory: None,
        catchphrase: None,
        updated_at: None,
    }
}

async fn setup() -> Result<(tempfile::TempDir, Orchestrator, String), Box<dyn std::error::Error>> {
    let root = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(root.path().join("data")).await?;
    let project = orchestrator
        .store
        .import_project("town", "/tmp/studio-town", "main", "/tmp/w")
        .await?;
    Ok((root, orchestrator, project.id))
}

fn is_invalid<T: std::fmt::Debug>(result: Result<T, OrchestratorError>) -> bool {
    matches!(result, Err(OrchestratorError::InvalidInput(_)))
}

#[test]
fn appearance_patch_distinguishes_absent_from_null() -> TestResult {
    let start = ProjectAppearance {
        project_id: "p".into(),
        kind: Some("team".into()),
        roof: Some("#aabbcc".into()),
        ..Default::default()
    };
    // absent fields stay; colour is normalised to lower case
    let patch: ProjectAppearancePatch = serde_json::from_str(r##"{"roof":"#C8553D"}"##)?;
    let next = apply_appearance_patch(start.clone(), &patch)?;
    assert_eq!(next.kind.as_deref(), Some("team"));
    assert_eq!(next.roof.as_deref(), Some("#c8553d"));
    // explicit null clears
    let patch: ProjectAppearancePatch = serde_json::from_str(r#"{"kind":null}"#)?;
    assert_eq!(apply_appearance_patch(start, &patch)?.kind, None);
    Ok(())
}

#[test]
fn appearance_patch_round_trips_through_daemon_serialization() -> TestResult {
    // the desktop serialises the patch again to send it to the daemon: null must survive
    let patch: ProjectAppearancePatch = serde_json::from_str(r#"{"sign":null}"#)?;
    let again: ProjectAppearancePatch = serde_json::from_str(&serde_json::to_string(&patch)?)?;
    assert_eq!(again.sign, Some(None));
    assert_eq!(again.kind, None);
    Ok(())
}

#[test]
fn appearance_rejects_bad_values_and_keeps_unknown_kinds() {
    let base = ProjectAppearance::default();
    let with = |json: &str| {
        apply_appearance_patch(
            base.clone(),
            &serde_json::from_str(json).unwrap_or_default(),
        )
    };
    assert!(is_invalid(with(r#"{"roof":"red"}"#)));
    assert!(is_invalid(with(r#"{"sign":"gold"}"#)));
    assert!(is_invalid(with(r#"{"kind":"Office Tower"}"#)));
    assert!(is_invalid(with(r#"{"yard":["fountain","rocket"]}"#)));
    // a building type this build does not know is stored as-is
    assert_eq!(
        with(r#"{"kind":"space-station"}"#)
            .ok()
            .and_then(|a| a.kind),
        Some("space-station".into())
    );
    // duplicates collapse
    assert_eq!(
        with(r#"{"yard":["pet","pet","flag"]}"#)
            .ok()
            .map(|a| a.yard),
        Some(vec!["pet".to_string(), "flag".to_string()])
    );
}

#[test]
fn profile_text_limits_count_characters_not_bytes() -> TestResult {
    let base = blank_profile(AgentKind::ClaudeCode);
    let patch = AgentProfilePatch {
        nickname: Some(Some("  小克小克小克小克小克小克小克小克  ".into())),
        ..Default::default()
    };
    assert_eq!(
        apply_profile_patch(base.clone(), &patch)?
            .nickname
            .as_deref(),
        Some("小克小克小克小克小克小克小克小克")
    );
    let too_long = AgentProfilePatch {
        nickname: Some(Some("一二三四五六七八九十一二三四五六七".into())),
        ..Default::default()
    };
    assert!(is_invalid(apply_profile_patch(base.clone(), &too_long)));
    let bad = AgentProfilePatch {
        accessory: Some(Some("crown".into())),
        ..Default::default()
    };
    assert!(is_invalid(apply_profile_patch(base, &bad)));
    Ok(())
}

#[tokio::test]
async fn appearance_defaults_updates_and_cascades() -> TestResult {
    let (_root, orchestrator, project) = setup().await?;
    let list = orchestrator.project_appearance_list().await?;
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].project_id, project);
    assert_eq!(
        (list[0].kind.clone(), list[0].updated_at.clone()),
        (None, None)
    );

    let patch: ProjectAppearancePatch =
        serde_json::from_str(r##"{"kind":"team","roof":"#336699","yard":["flag"]}"##)?;
    orchestrator
        .project_appearance_update(&project, &patch)
        .await?;
    let patch: ProjectAppearancePatch = serde_json::from_str(r#"{"roof":null,"sign":"neon"}"#)?;
    let saved = orchestrator
        .project_appearance_update(&project, &patch)
        .await?;
    assert_eq!(saved.kind.as_deref(), Some("team"));
    assert_eq!(saved.roof, None);
    assert_eq!(saved.sign.as_deref(), Some("neon"));
    assert_eq!(saved.yard, vec!["flag".to_string()]);
    assert_eq!(orchestrator.project_appearance_list().await?[0], saved);

    let missing = orchestrator
        .project_appearance_update("nope", &ProjectAppearancePatch::default())
        .await;
    assert!(is_invalid(missing));

    sqlx::query("DELETE FROM projects WHERE id=?")
        .bind(&project)
        .execute(orchestrator.store.pool())
        .await?;
    let left: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM project_appearance")
        .fetch_one(orchestrator.store.pool())
        .await?;
    assert_eq!(left, 0);
    Ok(())
}

#[tokio::test]
async fn members_replace_reorder_and_drive_the_summary() -> TestResult {
    let (_root, orchestrator, project) = setup().await?;
    let summary = &orchestrator.project_summaries().await?[0];
    assert_eq!(summary.agents_source, AgentsSource::Derived);

    let sidecar = AgentKind::External("my-sidecar".into());
    let first = orchestrator
        .project_members_set(
            &project,
            &[
                member(AgentKind::ClaudeCode, MemberRole::Developer),
                member(sidecar.clone(), MemberRole::Reviewer),
            ],
        )
        .await?;
    assert_eq!(first[1].agent, sidecar);
    let claude_added = first[0].added_at.clone();

    let second = orchestrator
        .project_members_set(
            &project,
            &[
                member(sidecar.clone(), MemberRole::Both),
                member(AgentKind::Codex, MemberRole::Reviewer),
                member(AgentKind::ClaudeCode, MemberRole::Developer),
            ],
        )
        .await?;
    let order: Vec<_> = second
        .iter()
        .map(|m| (m.agent.clone(), m.position))
        .collect();
    assert_eq!(
        order,
        vec![
            (sidecar.clone(), 0),
            (AgentKind::Codex, 1),
            (AgentKind::ClaudeCode, 2)
        ]
    );
    assert_eq!(second[0].role, MemberRole::Both);
    assert_eq!(second[2].added_at, claude_added);

    let summary = &orchestrator.project_summaries().await?[0];
    assert_eq!(summary.agents_source, AgentsSource::Assigned);
    assert_eq!(
        summary.assigned_agents,
        vec![sidecar.clone(), AgentKind::Codex, AgentKind::ClaudeCode]
    );

    let dup = orchestrator
        .project_members_set(
            &project,
            &[
                member(AgentKind::Codex, MemberRole::Developer),
                member(AgentKind::Codex, MemberRole::Reviewer),
            ],
        )
        .await;
    assert!(is_invalid(dup));
    let too_many: Vec<_> = (0..13)
        .map(|i| {
            member(
                AgentKind::External(format!("bot-{i}")),
                MemberRole::Developer,
            )
        })
        .collect();
    assert!(is_invalid(
        orchestrator.project_members_set(&project, &too_many).await
    ));
    // a rejected set leaves the previous members in place
    assert_eq!(orchestrator.project_members_get(&project).await?.len(), 3);

    orchestrator.project_members_set(&project, &[]).await?;
    let summary = &orchestrator.project_summaries().await?[0];
    assert_eq!(summary.agents_source, AgentsSource::Derived);
    Ok(())
}

#[tokio::test]
async fn agent_profiles_update_clear_and_reset() -> TestResult {
    let (_root, orchestrator, _project) = setup().await?;
    assert!(orchestrator.agent_profiles_list().await?.is_empty());
    let sidecar = AgentKind::External("my-sidecar".into());
    let patch: AgentProfilePatch = serde_json::from_str(
        r##"{"nickname":"小侧","color":"#2A9D8F","accessory":"scarf","catchphrase":"交给我"}"##,
    )?;
    orchestrator.agent_profile_update(&sidecar, &patch).await?;
    let patch: AgentProfilePatch = serde_json::from_str(r#"{"catchphrase":null}"#)?;
    let saved = orchestrator.agent_profile_update(&sidecar, &patch).await?;
    assert_eq!(saved.nickname.as_deref(), Some("小侧"));
    assert_eq!(saved.color.as_deref(), Some("#2a9d8f"));
    assert_eq!(saved.catchphrase, None);
    assert_eq!(orchestrator.agent_profiles_list().await?, vec![saved]);

    assert!(orchestrator.agent_profile_reset(&sidecar).await?);
    assert!(!orchestrator.agent_profile_reset(&sidecar).await?);
    assert!(orchestrator.agent_profiles_list().await?.is_empty());
    Ok(())
}
