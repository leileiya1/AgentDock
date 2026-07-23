use super::*;

fn tool_status(
    found: bool,
    compatible: bool,
    authenticated: Option<bool>,
    problem: Option<&str>,
    auth_problem: Option<&str>,
) -> ToolStatus {
    ToolStatus {
        found,
        path: found.then(|| "/usr/local/bin/cli".into()),
        version: found.then(|| "1.0.0".into()),
        compatible,
        problem: problem.map(str::to_string),
        authenticated,
        auth_method: None,
        auth_problem: auth_problem.map(str::to_string),
        support_level: CliSupportLevel::Verified,
        verified_versions: vec!["1.0.0".into()],
    }
}

fn descriptor(kind: AgentKind, name: &str, status: ToolStatus) -> ProviderDescriptor {
    cli_descriptor(kind, name, &status)
}

#[test]
fn assess_chain_runs_only_installed_and_authenticated_providers() {
    // Mirrors the reported P0 environment: Claude is installed and compatible but not logged in,
    // Gemini is not installed, and only Codex can actually run.
    let catalog = vec![
        descriptor(
            AgentKind::ClaudeCode,
            "Claude Code",
            tool_status(
                true,
                true,
                Some(false),
                None,
                Some("Claude Code 已安装，但尚未登录"),
            ),
        ),
        descriptor(
            AgentKind::Codex,
            "Codex",
            tool_status(true, true, Some(true), None, None),
        ),
        descriptor(
            AgentKind::GeminiCli,
            "Gemini CLI",
            tool_status(false, false, None, Some("gemini not found"), None),
        ),
    ];
    let chain = vec![
        AgentKind::ClaudeCode,
        AgentKind::GeminiCli,
        AgentKind::Codex,
    ];
    let assessment = Orchestrator::assess_chain(&chain, &catalog);

    // Only the runnable provider is attempted, and it keeps its position in the chain order.
    assert_eq!(assessment.ready, vec![AgentKind::Codex]);
    // The full chain is preserved, each entry annotated with real availability.
    assert_eq!(assessment.entries.len(), 3);
    assert!(!assessment.entries[0].available);
    assert!(
        assessment.entries[0]
            .problem
            .as_deref()
            .unwrap_or_default()
            .contains("尚未登录")
    );
    assert!(!assessment.entries[1].available);
    assert!(
        assessment.entries[1]
            .problem
            .as_deref()
            .unwrap_or_default()
            .contains("not found")
    );
    assert!(assessment.entries[2].available);

    // The skipped-provider reasons name every filtered-out provider, so the failure card can list
    // all root causes rather than only the last provider that was tried (P0-02).
    let skipped = assessment.skipped_reasons();
    assert_eq!(skipped.len(), 2);
    assert!(
        skipped
            .iter()
            .any(|line| line.contains("Claude Code") && line.contains("尚未登录"))
    );
    assert!(skipped.iter().any(|line| line.contains("Gemini CLI")));
}

#[test]
fn assess_chain_reports_when_nothing_is_runnable() {
    let catalog = vec![descriptor(
        AgentKind::ClaudeCode,
        "Claude Code",
        tool_status(false, false, None, Some("claude not found"), None),
    )];
    let assessment = Orchestrator::assess_chain(&[AgentKind::ClaudeCode], &catalog);
    assert!(assessment.ready.is_empty());
    let detail = planner_failure_detail(&assessment.skipped_reasons());
    assert!(detail.contains("没有 Provider 能完成规划"));
    assert!(detail.contains("Claude Code"));
}

#[test]
fn assess_chain_flags_providers_missing_from_catalog() {
    // A configured fallback the catalog does not surface cannot run and must be reported, not tried.
    let assessment = Orchestrator::assess_chain(&[AgentKind::GrokCli], &[]);
    assert!(assessment.ready.is_empty());
    assert!(
        assessment.entries[0]
            .problem
            .as_deref()
            .unwrap_or_default()
            .contains("尚未纳入")
    );
}

#[test]
fn planner_failure_detail_handles_empty_chain() {
    let detail = planner_failure_detail(&[]);
    assert!(detail.contains("没有可用于规划的 Provider"));
}

#[test]
fn generic_probe_targets_cover_every_builtin_cli_without_covering_apis() {
    for kind in [
        AgentKind::ClaudeCode,
        AgentKind::Codex,
        AgentKind::GeminiCli,
        AgentKind::QwenCode,
        AgentKind::QoderCli,
        AgentKind::GrokCli,
        AgentKind::KimiCli,
        AgentKind::MiniMaxCli,
    ] {
        assert!(Orchestrator::cli_probe_target(&kind).is_some(), "{kind}");
    }
    assert!(
        Orchestrator::cli_probe_target(&AgentKind::OpenAiApi).is_none(),
        "remote APIs use their own health checks"
    );
}

#[tokio::test]
async fn preflight_report_maps_roles_to_their_fallback_chains()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project(
            "p",
            "/tmp/preflight-probe",
            "main",
            "/tmp/preflight-probe-wt",
        )
        .await?;
    let settings = orchestrator.project(&project.id).await?.settings;
    // The chosen developer (Claude) is not logged in, but the configured Qoder fallback can run.
    let catalog = vec![
        descriptor(
            AgentKind::ClaudeCode,
            "Claude Code",
            tool_status(true, true, Some(false), None, Some("尚未登录")),
        ),
        descriptor(
            AgentKind::Codex,
            "Codex",
            tool_status(true, true, Some(true), None, None),
        ),
        descriptor(
            AgentKind::QoderCli,
            "Qoder CLI",
            tool_status(true, true, None, None, None),
        ),
    ];
    let report = orchestrator.build_preflight_report(
        &settings,
        AgentKind::ClaudeCode,
        AgentKind::Codex,
        false,
        true,
        &catalog,
    );

    // Both required roles are probed, each keyed to the primary provider it will try first.
    assert_eq!(report.roles.len(), 2);
    assert_eq!(report.roles[0].role, PreflightRole::Developer);
    assert_eq!(report.roles[0].primary, AgentKind::ClaudeCode);
    assert_eq!(
        report.roles[0].chain.first().map(|entry| &entry.provider),
        Some(&AgentKind::ClaudeCode)
    );
    assert_eq!(report.roles[1].role, PreflightRole::Reviewer);
    assert_eq!(report.roles[1].primary, AgentKind::Codex);
    assert_eq!(
        report.roles[1].chain.first().map(|entry| &entry.provider),
        Some(&AgentKind::Codex)
    );
    // The chosen primary is surfaced as unavailable even when the role is satisfied by a fallback,
    // so the desktop can still tell the user their pick needs a re-login.
    let claude_entry = report.roles[0]
        .chain
        .iter()
        .find(|entry| entry.provider == AgentKind::ClaudeCode);
    assert!(claude_entry.is_some_and(|entry| !entry.available));
    assert!(report.roles[0].ready);
    assert!(report.roles[1].ready);
    // The overall verdict is exactly the conjunction of each role having a runnable provider.
    assert!(report.ready);
    assert_eq!(report.ready, report.roles.iter().all(|role| role.ready));

    // With nothing installed, both roles fail and the run is refused up front.
    let empty = orchestrator.build_preflight_report(
        &settings,
        AgentKind::ClaudeCode,
        AgentKind::Codex,
        false,
        true,
        &[],
    );
    assert!(!empty.ready);
    assert!(empty.roles.iter().all(|role| !role.ready));
    Ok(())
}
