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
fn council_merge_keeps_fullest_evidence_and_every_suggestion() {
    // §23: merging duplicate issues keeps the most severe rank, the most complete evidence, and
    // every distinct suggested fix — never collapsing to one arbitrary member's version.
    let low = ReviewIssueResult {
        severity: Severity::Low,
        file: Some("src/auth.rs".into()),
        line_start: Some(5),
        line_end: Some(5),
        title: "缺少输入校验".into(),
        description: Some("这里必须校验，否则会空指针并连锁失败——这是更完整的证据说明".into()),
        suggested_action: Some("加一个判空分支".into()),
    };
    let high = ReviewIssueResult {
        severity: Severity::High,
        file: Some("src/auth.rs".into()),
        line_start: Some(5),
        line_end: Some(5),
        title: "缺少输入校验".into(),
        description: Some("简短证据".into()),
        suggested_action: Some("改用类型系统保证非空".into()),
    };
    let aggregate = aggregate_council(
        &[
            member(
                AgentKind::ClaudeCode,
                ReviewDecision::RequestChanges,
                vec![low],
            ),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![high],
            ),
        ],
        false,
    );
    assert_eq!(aggregate.issues.len(), 1);
    let merged = &aggregate.issues[0];
    assert_eq!(merged.issue.severity, Severity::High); // most severe rank drives gating
    assert!(
        merged
            .issue
            .description
            .as_deref()
            .unwrap_or_default()
            .contains("更完整的证据")
    ); // fullest evidence
    let suggestion = merged.issue.suggested_action.as_deref().unwrap_or_default();
    assert!(suggestion.contains("加一个判空分支")); // both distinct fixes preserved
    assert!(suggestion.contains("改用类型系统保证非空"));
    assert_eq!(merged.reported_by.len(), 2);
}

#[test]
fn council_flags_severity_disagreement_without_changing_the_decision() {
    // §24: when members split across the serious↔minor boundary on the same issue, flag it — but
    // the decision must be unchanged (still RequestChanges on the highest severity). Safety first:
    // this only surfaces disagreement to the human, it never relaxes the gate.
    let aggregate = aggregate_council(
        &[
            member(
                AgentKind::ClaudeCode,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Critical, "并发写入未加锁")],
            ),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Low, "并发写入未加锁")],
            ),
        ],
        false,
    );
    assert_eq!(aggregate.issues.len(), 1);
    assert!(
        aggregate.issues[0].severity_disagreement,
        "critical vs low is a real disagreement"
    );
    assert_eq!(aggregate.issues[0].issue.severity, Severity::Critical); // gate still uses the max
    assert_eq!(aggregate.decision, ReviewDecision::RequestChanges); // decision unchanged

    // Agreement within the serious tier (critical vs high) is not flagged as a disagreement.
    let agree = aggregate_council(
        &[
            member(
                AgentKind::ClaudeCode,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Critical, "同一处")],
            ),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::High, "同一处")],
            ),
        ],
        false,
    );
    assert!(!agree.issues[0].severity_disagreement);
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

#[test]
fn council_split_without_serious_issue_leans_conservative() {
    // 2 人平票（1 通过 / 1 请改，且无高危问题）→ 保守取「请改」，不放行到人工批准。
    let split = aggregate_council(
        &[
            member(AgentKind::ClaudeCode, ReviewDecision::Pass, Vec::new()),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::Low, "命名可读性")],
            ),
        ],
        false,
    );
    assert_eq!(split.decision, ReviewDecision::RequestChanges);
}

#[test]
fn council_serious_issue_from_a_single_member_vetoes_pass() {
    // 多数通过，但任一成员发现高危问题即否决放行——异构评审的核心价值：去相关性盲区。
    let vetoed = aggregate_council(
        &[
            member(AgentKind::ClaudeCode, ReviewDecision::Pass, Vec::new()),
            member(AgentKind::Codex, ReviewDecision::Pass, Vec::new()),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                vec![issue(Severity::High, "未校验的外部输入")],
            ),
        ],
        false,
    );
    assert_eq!(vetoed.decision, ReviewDecision::RequestChanges);
    assert!(
        vetoed
            .issues
            .iter()
            .any(|i| i.issue.severity == Severity::High)
    );
}

#[test]
fn council_unanimous_mode_blocks_pass_on_any_dissent() {
    // 要求全票通过时，任何一票「请改」都不放行，即使多数通过。
    let dissent = aggregate_council(
        &[
            member(AgentKind::ClaudeCode, ReviewDecision::Pass, Vec::new()),
            member(AgentKind::Codex, ReviewDecision::Pass, Vec::new()),
            member(
                AgentKind::GeminiCli,
                ReviewDecision::RequestChanges,
                Vec::new(),
            ),
        ],
        true,
    );
    assert_eq!(dissent.decision, ReviewDecision::RequestChanges);
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

async fn seed_review(
    orchestrator: &Orchestrator,
    task_id: &str,
    revision: i64,
    critical_issues: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let titles = (0..critical_issues)
        .map(|i| format!("blocker {i}"))
        .collect::<Vec<_>>();
    seed_review_titled(
        orchestrator,
        task_id,
        revision,
        &titles.iter().map(String::as_str).collect::<Vec<_>>(),
    )
    .await
}

async fn seed_review_titled(
    orchestrator: &Orchestrator,
    task_id: &str,
    revision: i64,
    critical_titles: &[&str],
) -> Result<(), Box<dyn std::error::Error>> {
    let now = chrono::Utc::now().to_rfc3339();
    let run_id = format!("run-{task_id}-{revision}");
    sqlx::query(
        "INSERT INTO agent_runs(id,task_id,revision,role,status,run_dir,timeout_secs,idle_timeout_secs,created_at) \
         VALUES(?,?,?,'reviewer','SUCCEEDED','/tmp',60,60,?)",
    )
    .bind(&run_id).bind(task_id).bind(revision).bind(&now)
    .execute(orchestrator.store.pool()).await?;
    let review_id = format!("review-{task_id}-{revision}");
    sqlx::query(
        "INSERT INTO reviews(id,task_id,revision,run_id,commit_sha,decision,raw_path,created_at) \
         VALUES(?,?,?,?,'abc1234','request_changes','/tmp/raw',?)",
    )
    .bind(&review_id)
    .bind(task_id)
    .bind(revision)
    .bind(&run_id)
    .bind(&now)
    .execute(orchestrator.store.pool())
    .await?;
    for (i, title) in critical_titles.iter().enumerate() {
        sqlx::query(
            "INSERT INTO review_issues(id,review_id,severity,file,title) VALUES(?,?,'critical','src/core.rs',?)",
        )
        .bind(format!("issue-{review_id}-{i}"))
        .bind(&review_id)
        .bind(*title)
        .execute(orchestrator.store.pool())
        .await?;
    }
    Ok(())
}

#[tokio::test]
async fn convergence_stall_fires_only_when_blockers_stop_decreasing()
-> Result<(), Box<dyn std::error::Error>> {
    // §32: two consecutive review rounds without a drop in blocking-issue count means the loop is
    // not converging and should pause for a human decision.
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("conv", "/tmp/conv", "main", "/tmp/conv-wt")
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "converge",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(6),
        )
        .await?;
    // r1: 3 blockers, r2: 3 (no decrease), r3: 1 (decrease), r4: 2 (increase).
    seed_review(&orchestrator, &task.id, 1, 3).await?;
    seed_review(&orchestrator, &task.id, 2, 3).await?;
    seed_review(&orchestrator, &task.id, 3, 1).await?;
    seed_review(&orchestrator, &task.id, 4, 2).await?;
    let mut task_row = orchestrator.task(&task.id).await?;

    task_row.revision = 2; // 3 vs 3 → no progress
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Stalled
    );

    task_row.revision = 3; // 1 vs 3 → progress
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Ok
    );

    task_row.revision = 4; // 2 vs 1 → worse than before
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Regressed
    );
    Ok(())
}

#[tokio::test]
async fn convergence_stall_ignores_a_previous_round_with_no_blockers()
-> Result<(), Box<dyn std::error::Error>> {
    // The first appearance of blockers is not a stall: there was nothing to reduce yet.
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("conv2", "/tmp/conv2", "main", "/tmp/conv2-wt")
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "converge",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(6),
        )
        .await?;
    seed_review(&orchestrator, &task.id, 1, 0).await?; // no blockers last round
    seed_review(&orchestrator, &task.id, 2, 2).await?; // blockers appear now
    let mut task_row = orchestrator.task(&task.id).await?;
    task_row.revision = 2;
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Ok
    );
    Ok(())
}

#[tokio::test]
async fn resolved_issue_reappearing_is_a_regression() -> Result<(), Box<dyn std::error::Error>> {
    // §34「旧问题重现」: an issue fixed in round 2 that comes back in round 3 is a regression, even
    // when the blocker count itself did not rise.
    let dir = tempfile::tempdir()?;
    let orchestrator = Orchestrator::open(dir.path()).await?;
    let project = orchestrator
        .store
        .import_project("reappear", "/tmp/reappear", "main", "/tmp/reappear-wt")
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "reappear",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(6),
        )
        .await?;
    seed_review_titled(&orchestrator, &task.id, 1, &["null deref in auth"]).await?;
    seed_review_titled(&orchestrator, &task.id, 2, &["missing timeout"]).await?; // the round-1 issue was fixed
    seed_review_titled(&orchestrator, &task.id, 3, &["null deref in auth"]).await?; // …and it came back
    let mut task_row = orchestrator.task(&task.id).await?;
    task_row.revision = 3;
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Regressed
    );
    Ok(())
}

async fn run_git(repo: &std::path::Path, args: &[&str]) -> Result<(), Box<dyn std::error::Error>> {
    let status = tokio::process::Command::new("git")
        .args(args)
        .current_dir(repo)
        .output()
        .await?;
    if !status.status.success() {
        return Err(String::from_utf8_lossy(&status.stderr).into_owned().into());
    }
    Ok(())
}

async fn head_sha(repo: &std::path::Path) -> Result<String, Box<dyn std::error::Error>> {
    let out = tokio::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo)
        .output()
        .await?;
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}

#[tokio::test]
async fn churning_the_same_file_three_rounds_is_a_stall() -> Result<(), Box<dyn std::error::Error>>
{
    // §33: even while blockers drift down (3→2→1), rewriting the same file in three consecutive
    // rounds signals the approach is not converging.
    let dir = tempfile::tempdir()?;
    let repo = dir.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    run_git(&repo, &["init", "-q", "-b", "main"]).await?;
    run_git(&repo, &["config", "user.email", "t@t.local"]).await?;
    run_git(&repo, &["config", "user.name", "t"]).await?;
    tokio::fs::write(repo.join("core.rs"), "v0").await?;
    run_git(&repo, &["add", "."]).await?;
    run_git(&repo, &["commit", "-q", "-m", "base"]).await?;
    let base = head_sha(&repo).await?;
    let mut shas = Vec::new();
    for i in 1..=3 {
        tokio::fs::write(repo.join("core.rs"), format!("v{i}")).await?;
        run_git(&repo, &["commit", "-qam", &format!("r{i}")]).await?;
        shas.push(head_sha(&repo).await?);
    }

    let orchestrator = Orchestrator::open(dir.path().join("data")).await?;
    let project = orchestrator
        .store
        .import_project(
            "churn",
            repo.to_str().unwrap_or_default(),
            "main",
            "/tmp/churn-wt",
        )
        .await?;
    let task = orchestrator
        .task_create(
            &project.id,
            "churn",
            "test",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            Some(6),
        )
        .await?;
    sqlx::query("UPDATE tasks SET base_commit=? WHERE id=?")
        .bind(&base)
        .bind(&task.id)
        .execute(orchestrator.store.pool())
        .await?;
    for (index, sha) in shas.iter().enumerate() {
        sqlx::query("INSERT INTO task_revisions(id,task_id,revision,commit_sha,created_at) VALUES(?,?,?,?,?)")
            .bind(uuid::Uuid::now_v7().to_string())
            .bind(&task.id)
            .bind((index + 1) as i64)
            .bind(sha)
            .bind(chrono::Utc::now().to_rfc3339())
            .execute(orchestrator.store.pool())
            .await?;
    }
    // Blockers strictly decreasing, so §32/§34 do not fire — only the same-file churn should.
    seed_review(&orchestrator, &task.id, 1, 3).await?;
    seed_review(&orchestrator, &task.id, 2, 2).await?;
    seed_review(&orchestrator, &task.id, 3, 1).await?;
    let mut task_row = orchestrator.task(&task.id).await?;
    task_row.revision = 3;
    assert_eq!(
        orchestrator.assess_convergence(&task_row).await?,
        ConvergenceHealth::Stalled
    );
    Ok(())
}
