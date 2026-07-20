use super::*;

fn member(
    agent: AgentKind,
    decision: ReviewDecision,
    issues: Vec<ReviewIssueResult>,
) -> CouncilMemberReview {
    CouncilMemberReview {
        agent,
        review: ReviewResult {
            schema_version: 1,
            task_id: "task".into(),
            revision: 1,
            commit_sha: "abcdef1".into(),
            decision,
            summary: "independent review".into(),
            issues,
        },
        run_dir: PathBuf::from("/tmp/run"),
    }
}

fn issue(severity: Severity, title: &str) -> ReviewIssueResult {
    ReviewIssueResult {
        severity,
        file: Some("src/main.rs".into()),
        line_start: Some(10),
        line_end: Some(10),
        title: title.into(),
        description: Some("details".into()),
        suggested_action: None,
    }
}

#[test]
fn council_deduplicates_issues_and_keeps_agreement_sources() {
    let aggregate = aggregate_council(
        &[
            member(
                AgentKind::ClaudeCode,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Medium, "Missing guard")],
            ),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::High, "missing guard")],
            ),
        ],
        false,
    );
    assert_eq!(aggregate.decision, ReviewDecision::RequestChanges);
    assert_eq!(aggregate.issues.len(), 1);
    assert_eq!(aggregate.issues[0].issue.severity, Severity::High);
    assert_eq!(aggregate.issues[0].reported_by.len(), 2);
}

#[test]
fn council_uses_bounded_deterministic_adjudication() {
    let majority_pass = aggregate_council(
        &[
            member(AgentKind::ClaudeCode, ReviewDecision::Pass, Vec::new()),
            member(AgentKind::GeminiCli, ReviewDecision::Pass, Vec::new()),
            member(
                AgentKind::QwenCode,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Low, "Minor naming")],
            ),
        ],
        false,
    );
    assert_eq!(majority_pass.decision, ReviewDecision::Pass);
    let blocked = aggregate_council(
        &[
            member(AgentKind::ClaudeCode, ReviewDecision::Pass, Vec::new()),
            member(AgentKind::GeminiCli, ReviewDecision::Block, Vec::new()),
        ],
        false,
    );
    assert_eq!(blocked.decision, ReviewDecision::Block);
}

#[tokio::test]
async fn council_excludes_the_developer_family_and_duplicate_vendors()
-> Result<(), Box<dyn std::error::Error>> {
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let mut settings = ProjectSettings::default();
    settings.review_council.enabled = true;
    settings.review_council.reviewers = vec![
        AgentKind::OpenAiApi,
        AgentKind::ClaudeCode,
        AgentKind::AnthropicApi,
        AgentKind::GeminiCli,
    ];
    let task = TaskRow {
        id: "task".into(),
        project_id: "project".into(),
        seq: 1,
        title: "test".into(),
        description: "test".into(),
        status: TaskStatus::ReadyForReview,
        blocked_detail: None,
        developer: AgentKind::Codex,
        reviewer: AgentKind::OpenAiApi,
        target_branch: "main".into(),
        base_commit: None,
        branch: None,
        worktree_path: None,
        revision: 1,
        max_revisions: 3,
        api_egress_approved: true,
        policy: TaskPolicy {
            require_plan_approval: false,
            ..TaskPolicy::default()
        },
    };
    let targets = orchestrator.council_targets(&task, &settings, &AgentKind::Codex);
    assert_eq!(targets, vec![AgentKind::ClaudeCode, AgentKind::GeminiCli]);
    Ok(())
}
