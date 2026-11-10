use super::*;
type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
struct Fixture {
    _root: tempfile::TempDir,
    owner: Orchestrator,
    task: String,
    repo: PathBuf,
}
async fn git(repo: &Path, args: &[&str]) -> TestResult<String> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .await?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).into_owned().into());
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_string())
}
fn config() -> TeamConfig {
    TeamConfig {
        team_capacity: 4,
        max_concurrent_attempts: 2,
        planner_member_id: "lead".into(),
        members: vec![
            TeamMember {
                id: "lead".into(),
                name: "Planner / Reviewer".into(),
                roles: vec![TeamRole::Planner, TeamRole::Reviewer],
                providers: vec![AgentKind::ClaudeCode],
            },
            TeamMember {
                id: "dev-a".into(),
                name: "Developer A".into(),
                roles: vec![TeamRole::Developer],
                providers: vec![AgentKind::Codex],
            },
            TeamMember {
                id: "dev-b".into(),
                name: "Developer B".into(),
                roles: vec![TeamRole::Developer],
                providers: vec![AgentKind::Codex],
            },
            TeamMember {
                id: "tester".into(),
                name: "Tester".into(),
                roles: vec![TeamRole::Tester],
                providers: vec![AgentKind::Codex],
            },
        ],
        max_attempts: 40,
        verification_reserve: 4,
        max_reworks: 3,
        max_plan_revisions: 5,
        attempt_timeout_secs: 30,
        write_globs: vec!["*.cjs".into()],
        allowed_commands: vec![vec!["node".into()]],
        require_plan_approval: false,
        cross_vendor_review: true,
    }
}
fn work(id: &str, member: &str) -> TeamWorkSpec {
    TeamWorkSpec {
        id: id.into(),
        title: id.into(),
        instructions: format!("Implement {id}"),
        kind: TeamWorkKind::Implement,
        depends_on: vec![],
        member_ids: vec![member.into()],
        requirement_ids: vec!["goal".into(), if id == "a" { "r1" } else { "r2" }.into()],
        write_globs: vec![format!("{id}.cjs")],
        local_checks: vec![],
    }
}
fn plan() -> TeamPlan {
    TeamPlan {
        schema_version: 1,
        rationale: "two independent modules".into(),
        items: vec![work("a", "dev-a"), work("b", "dev-b")],
        criteria: vec![
            criterion(
                "positive",
                "r1",
                "a",
                "const assert=require('node:assert');assert.equal(require('./a.cjs')(2),4);console.log('positive ok')",
            ),
            criterion(
                "negative",
                "r2",
                "b",
                "const assert=require('node:assert');assert.equal(require('./b.cjs')(-2),-4);console.log('negative ok')",
            ),
        ],
    }
}
fn criterion(id: &str, req: &str, owner: &str, script: &str) -> TeamCriterion {
    TeamCriterion {
        id: id.into(),
        text: id.into(),
        requirement_ids: vec!["goal".into(), req.into()],
        required: true,
        expected: "assertions pass".into(),
        checks: vec![TeamCheck {
            id: format!("check-{id}"),
            argv: vec!["node".into(), "-e".into(), script.into()],
            expected_exit: 0,
            stdout_contains: Some(format!("{id} ok")),
            timeout_secs: 10,
        }],
        owner_work_item_id: owner.into(),
    }
}
async fn fixture() -> TestResult<Fixture> {
    let root = tempfile::tempdir()?;
    let repo = root.path().join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    git(&repo, &["init", "-q", "-b", "main"]).await?;
    git(&repo, &["config", "user.name", "AgentFlow Test"]).await?;
    git(&repo, &["config", "user.email", "test@agentflow.local"]).await?;
    tokio::fs::write(repo.join("a.cjs"), "module.exports=n=>0;\n").await?;
    tokio::fs::write(repo.join("b.cjs"), "module.exports=n=>0;\n").await?;
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-qm", "baseline"]).await?;
    let owner = Orchestrator::open(root.path().join("data")).await?;
    owner
        .settings_update(&GlobalSettings {
            default_provider_max_concurrent: 2,
            max_concurrent_runs: Some(4),
            ..GlobalSettings::default()
        })
        .await?;
    let project = owner.project_import(&repo).await?;
    let view = owner
        .team_create(TeamCreateInput {
            project_id: project.id,
            title: "team fixture".into(),
            goal: "positive and negative doubling".into(),
            requirements: vec![
                "double positive values".into(),
                "double negative values".into(),
            ],
            target_branch: None,
            config: config(),
        })
        .await?;
    Ok(Fixture {
        _root: root,
        owner,
        task: view.task_id,
        repo,
    })
}
fn submission(claim: &TeamClaim) -> TeamSubmission {
    TeamSubmission {
        schema_version: 1,
        identity: claim.identity.clone(),
        summary: "deterministic executor".into(),
        plan: None,
        approved: true,
        changed_files: vec![],
        check_ids: vec![],
        issues: vec![],
        environment_notes: vec![],
    }
}
async fn claim(f: &Fixture) -> TestResult<TeamClaim> {
    match f.owner.team_claim_next(8).await? {
        Some(c) => Ok(c),
        None => Err(format!(
            "expected ready work: {:?}",
            f.owner.team_get(&f.task).await?
        )
        .into()),
    }
}
async fn complete(f: &Fixture, c: &TeamClaim, mut s: TeamSubmission) -> TestResult {
    let (wt, base, included) = f.owner.team_prepare_workspace(c).await?;
    if c.spec.kind == TeamWorkKind::Implement {
        let path = format!("{}.cjs", c.spec.id);
        tokio::fs::write(wt.join(&path), "module.exports=n=>n*2;\n").await?;
        s.changed_files = if f.owner.git.has_changes(&wt).await? {
            vec![path]
        } else {
            vec![]
        };
    }
    f.owner
        .team_complete_submission(c, s, &wt, &base, included)
        .await?;
    f.owner.team_progress(&f.task).await?;
    Ok(())
}
async fn planned(f: &Fixture, p: TeamPlan) -> TestResult {
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(f).await?;
    assert_eq!(c.spec.kind, TeamWorkKind::Plan);
    let mut s = submission(&c);
    s.plan = Some(p);
    complete(f, &c, s).await?;
    let coverage = claim(f).await?;
    assert_eq!(coverage.spec.kind, TeamWorkKind::Coverage);
    complete(f, &coverage, submission(&coverage)).await?;
    Ok(())
}
async fn developed(f: &Fixture) -> TestResult {
    planned(f, plan()).await?;
    let a = claim(f).await?;
    let b = claim(f).await?;
    assert_eq!(a.provider, b.provider);
    assert_ne!(a.identity.member_id, b.identity.member_id);
    assert_ne!(a.identity.attempt_id, b.identity.attempt_id);
    let (ra, rb) = tokio::join!(
        complete(f, &a, submission(&a)),
        complete(f, &b, submission(&b))
    );
    ra?;
    rb?;
    let integrate = claim(f).await?;
    assert_eq!(integrate.spec.kind, TeamWorkKind::Integrate);
    f.owner.team_execute_claim(integrate).await?;
    Ok(())
}
async fn verified(f: &Fixture) -> TestResult {
    let c = claim(f).await?;
    assert_eq!(c.spec.kind, TeamWorkKind::Verify);
    let mut s = submission(&c);
    s.check_ids = vec!["check-positive".into(), "check-negative".into()];
    complete(f, &c, s).await?;
    Ok(())
}

#[tokio::test]
async fn i01_i06_i07_i11_i13_full_controlled_pipeline_and_local_delivery() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    let pre = f.owner.team_get(&f.task).await?.ok_or("missing")?;
    assert!(pre.candidate.is_some());
    assert!(
        pre.criteria
            .iter()
            .all(|c| c.status == CriterionStatus::NotRun)
    );
    assert!(!pre.gate_blockers.is_empty());
    verified(&f).await?;
    let review = claim(&f).await?;
    assert_eq!(review.spec.kind, TeamWorkKind::Review);
    complete(&f, &review, submission(&review)).await?;
    let ready = f.owner.team_get(&f.task).await?.ok_or("missing")?;
    assert_eq!(ready.phase, TeamPhase::ReadyForDelivery);
    assert!(ready.gate_blockers.is_empty(), "{:?}", ready.gate_blockers);
    assert!(
        ready
            .criteria
            .iter()
            .all(|c| c.status == CriterionStatus::Passed)
    );
    f.owner.team_action(&f.task, TeamAction::Deliver).await?;
    let delivered = f.owner.team_get(&f.task).await?.ok_or("missing")?;
    if delivered.phase != TeamPhase::Delivered {
        let events = sqlx::query_scalar::<_, String>(
            "SELECT payload_json FROM events WHERE task_id=? AND event_type LIKE 'merge:%'",
        )
        .bind(&f.task)
        .fetch_all(f.owner.store.pool())
        .await?;
        eprintln!("delivery events {events:?}");
    }
    assert_eq!(delivered.phase, TeamPhase::Delivered);
    assert_eq!(
        git(&f.repo, &["rev-parse", "HEAD^{tree}"]).await?,
        ready.candidate.ok_or("candidate")?.tree_sha
    );
    // Repeated requests reconcile the original receipt, without another merge.
    let first = git(&f.repo, &["rev-parse", "HEAD"]).await?;
    f.owner.team_action(&f.task, TeamAction::Deliver).await?;
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]).await?, first);
    Ok(())
}
#[tokio::test]
async fn i02_i03_atomic_claim_same_seat_late_event_and_dedup() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let (a, b) = tokio::join!(f.owner.team_claim_next(8), f.owner.team_claim_next(8));
    let a = a?;
    let b = b?;
    assert_eq!(usize::from(a.is_some()) + usize::from(b.is_some()), 1);
    let old = a.or(b).ok_or("claim")?;
    f.owner
        .team_action(
            &f.task,
            TeamAction::CancelWork {
                work_item_id: "__plan".into(),
            },
        )
        .await?;
    assert!(
        f.owner
            .team_action(
                &f.task,
                TeamAction::RetryWork {
                    work_item_id: "__plan".into()
                }
            )
            .await
            .is_err()
    );
    assert!(
        !f.owner
            .store
            .team_finish(&old, true, None, "late", None)
            .await?
    );
    f.owner
        .team_action(
            &f.task,
            TeamAction::RetryWork {
                work_item_id: "__plan".into(),
            },
        )
        .await?;
    let next = claim(&f).await?;
    assert!(next.identity.lease_epoch > old.identity.lease_epoch);
    assert!(
        !f.owner
            .store
            .team_finish(&old, true, None, "duplicate", None)
            .await?
    );
    assert!(f.owner.store.team_identity_valid(&next.identity).await?);
    let events: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE team_event_key=?")
        .bind(format!("{}:finished", old.identity.attempt_id))
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(events, 1);
    Ok(())
}
#[tokio::test]
async fn i04_i05_local_plan_change_preserves_unrelated_inflight_and_invalidates_successors()
-> TestResult {
    let f = fixture().await?;
    planned(&f, plan()).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    let old = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let mut new = plan();
    new.items[0].instructions = "different implementation a".into();
    f.owner
        .team_action(
            &f.task,
            TeamAction::RevisePlan {
                expected_revision: old.plan_revision,
                plan: new,
            },
        )
        .await?;
    assert!(f.owner.store.team_identity_valid(&b.identity).await?);
    assert!(!f.owner.store.team_identity_valid(&a.identity).await?);
    complete(&f, &b, submission(&b)).await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(
        after
            .items
            .iter()
            .find(|i| i.spec.id == "b")
            .ok_or("b")?
            .state,
        TeamWorkState::Accepted
    );
    assert_eq!(
        after
            .items
            .iter()
            .find(|i| i.spec.id == "a")
            .ok_or("a")?
            .state,
        TeamWorkState::Pending
    );
    assert_eq!(after.attempts_used, old.attempts_used);
    Ok(())
}
#[tokio::test]
async fn dependencies_wait_for_backend_output_acceptance() -> TestResult {
    let f = fixture().await?;
    let mut p = plan();
    p.items[1].depends_on.push("a".into());
    planned(&f, p).await?;
    let a = claim(&f).await?;
    assert_eq!(a.spec.id, "a");
    assert!(f.owner.team_claim_next(8).await?.is_none());
    complete(&f, &a, submission(&a)).await?;
    let b = claim(&f).await?;
    let (wt, base, included) = f.owner.team_prepare_workspace(&b).await?;
    assert!(!included.is_empty());
    assert_eq!(
        tokio::fs::read_to_string(wt.join("a.cjs")).await?,
        "module.exports=n=>n*2;\n"
    );
    assert_ne!(
        base,
        f.owner.task(&f.task).await?.base_commit.ok_or("base")?
    );
    Ok(())
}
#[tokio::test]
async fn i07_unrun_entry_and_new_candidate_cannot_reuse_green_evidence() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    let c = claim(&f).await?;
    let mut s = submission(&c);
    s.check_ids = vec!["check-positive".into()];
    complete(&f, &c, s).await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(view.criteria[0].status, CriterionStatus::Passed);
    assert_eq!(view.criteria[1].status, CriterionStatus::NotRun);
    assert_ne!(view.phase, TeamPhase::ReadyForDelivery);
    f.owner
        .team_request_rework(&f.task, &["a".into()], "force new candidate", None)
        .await?;
    let a = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    let next = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert!(
        next.criteria
            .iter()
            .all(|c| c.status == CriterionStatus::NotRun)
    );
    assert!(next.criteria[0].stale);
    Ok(())
}
#[tokio::test]
async fn i09_i14_shared_budget_contention_keeps_verification_reserve_and_unknown_usage()
-> TestResult {
    let f = fixture().await?;
    planned(&f, plan()).await?;
    sqlx::query("UPDATE team_tasks SET attempts_used=35 WHERE task_id=?")
        .bind(&f.task)
        .execute(f.owner.store.pool())
        .await?;
    let (a, b) = tokio::join!(f.owner.team_claim_next(8), f.owner.team_claim_next(8));
    let a = a?;
    let b = b?;
    assert_eq!(usize::from(a.is_some()) + usize::from(b.is_some()), 1);
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(view.attempts_used, 36);
    assert!(
        view.attempts
            .iter()
            .all(|a| a.cost_usd.is_none() && a.tokens_in.is_none())
    );
    let mut changed = plan();
    changed.items.push(work("c", "dev-a"));
    changed.items[2].requirement_ids = vec!["goal".into()];
    let count = view.attempts_used;
    assert_eq!(count, 36);
    assert_eq!(view.reworks_used, 0);
    Ok(())
}
#[tokio::test]
async fn i12_cancel_race_and_crash_recovery_keep_legacy_readable() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let (wt, _, _) = f.owner.team_prepare_workspace(&c).await?;
    f.owner.team_recover().await?;
    assert!(!f.owner.store.team_identity_valid(&c.identity).await?);
    let next = claim(&f).await?;
    assert_ne!(next.identity.attempt_id, c.identity.attempt_id);
    assert!(wt.exists());
    f.owner.team_action(&f.task, TeamAction::Cancel).await?;
    assert!(
        !f.owner
            .store
            .team_finish(&next, true, None, "raced success", None)
            .await?
    );
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Cancelled
    );
    assert!(f.owner.team_claim_next(8).await?.is_none());
    let project = f.owner.project_import(&f.repo).await?;
    let legacy = f
        .owner
        .task_create(
            &project.id,
            "legacy",
            "unchanged",
            AgentKind::Codex,
            AgentKind::ClaudeCode,
            None,
            None,
        )
        .await?;
    assert!(f.owner.team_get(&legacy.id).await?.is_none());
    assert!(
        f.owner
            .task_get(&legacy.id)
            .await?
            .acceptance_criteria
            .is_empty()
    );
    Ok(())
}
#[tokio::test]
async fn i11_target_changed_after_acceptance_blocks_delivery() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    tokio::fs::write(f.repo.join("unrelated.txt"), "branch changed").await?;
    git(&f.repo, &["add", "."]).await?;
    git(&f.repo, &["commit", "-qm", "external change"]).await?;
    assert!(
        f.owner
            .team_action(&f.task, TeamAction::Deliver)
            .await
            .is_err()
    );
    assert_ne!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Delivered
    );
    Ok(())
}

async fn files(f: &Fixture, c: &TeamClaim, changes: &[(&str, &str)]) -> TestResult {
    let (wt, base, included) = f.owner.team_prepare_workspace(c).await?;
    let mut s = submission(c);
    for (path, content) in changes {
        tokio::fs::write(wt.join(path), content).await?;
        s.changed_files.push(path.to_string());
    }
    f.owner
        .team_complete_submission(c, s, &wt, &base, included)
        .await?;
    f.owner.team_progress(&f.task).await?;
    Ok(())
}
#[tokio::test]
async fn integration_conflict_routes_to_owner_and_preserves_other_delta() -> TestResult {
    let f = fixture().await?;
    let mut p = plan();
    p.items[1].write_globs = vec!["a.cjs".into(), "b.cjs".into()];
    planned(&f, p).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    files(&f, &a, &[("a.cjs", "module.exports=n=>n*2;\n")]).await?;
    files(&f, &b, &[("a.cjs", "module.exports=n=>n*3;\n")]).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(after.reworks_used, 1);
    assert_eq!(
        after
            .items
            .iter()
            .find(|w| w.spec.id == "a")
            .ok_or("a")?
            .state,
        TeamWorkState::Accepted
    );
    assert_eq!(
        after
            .items
            .iter()
            .find(|w| w.spec.id == "b")
            .ok_or("b")?
            .state,
        TeamWorkState::Pending
    );
    let fix = claim(&f).await?;
    assert_eq!(fix.spec.id, "b");
    files(&f, &fix, &[("b.cjs", "module.exports=n=>n*2;\n")]).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::ReadyForDelivery
    );
    Ok(())
}
#[tokio::test]
async fn failed_assertion_reworks_only_responsible_member_and_retests_new_candidate() -> TestResult
{
    let f = fixture().await?;
    planned(&f, plan()).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    files(&f, &b, &[("b.cjs", "module.exports=n=>n*3;\n")]).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    let old = f
        .owner
        .team_get(&f.task)
        .await?
        .ok_or("view")?
        .candidate
        .ok_or("candidate")?;
    verified(&f).await?;
    let failed = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(failed.reworks_used, 1);
    assert_eq!(failed.criteria[1].status, CriterionStatus::Failed);
    assert_eq!(
        failed
            .items
            .iter()
            .find(|i| i.spec.id == "a")
            .ok_or("a")?
            .artifact_id,
        Some(
            failed
                .artifacts
                .iter()
                .find(|a| a.work_item_id == "a")
                .ok_or("artifact")?
                .id
                .clone()
        )
    );
    let fix = claim(&f).await?;
    assert_eq!(fix.spec.id, "b");
    complete(&f, &fix, submission(&fix)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    let before = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_ne!(before.candidate.ok_or("candidate")?.id, old.id);
    assert!(
        before
            .criteria
            .iter()
            .all(|c| c.stale && c.status == CriterionStatus::NotRun)
    );
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::ReadyForDelivery
    );
    Ok(())
}
#[tokio::test]
async fn i08_fallback_contributor_can_make_cross_vendor_review_unsatisfiable() -> TestResult {
    let f = fixture().await?;
    f.owner
        .team_action(
            &f.task,
            TeamAction::ReplaceMember {
                member: TeamMember {
                    id: "dev-b".into(),
                    name: "fallback developer".into(),
                    roles: vec![TeamRole::Developer],
                    providers: vec![AgentKind::ClaudeCode],
                },
            },
        )
        .await?;
    planned(&f, plan()).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    complete(&f, &b, submission(&b)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    verified(&f).await?;
    assert!(f.owner.team_claim_next(8).await?.is_none());
    let blocked = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(blocked.phase, TeamPhase::Blocked);
    assert!(
        blocked
            .detail
            .as_deref()
            .unwrap_or_default()
            .contains("no independent")
    );
    assert!(f.owner.team_assert_gate(&f.task).await.is_err());
    Ok(())
}
#[tokio::test]
async fn real_local_http_runner_executes_positive_and_negative_requests() -> TestResult {
    let f = fixture().await?;
    let mut p = plan();
    for (c, file, n, label) in [(0, "a", 2, "positive"), (1, "b", -2, "negative")] {
        p.criteria[c].checks[0].argv[2] = format!(
            "const http=require('node:http'),assert=require('node:assert'); const s=http.createServer((q,r)=>r.end(String(require('./{file}.cjs')(Number(new URL(q.url,'http://localhost').searchParams.get('n'))))));s.listen(0,'127.0.0.1',async()=>{{try{{const r=await fetch('http://127.0.0.1:'+s.address().port+'/?n={n}');assert.equal(await r.text(),'{expected}');console.log('{label} ok')}}catch(e){{console.error(e);process.exitCode=1}}finally{{s.close()}}}})",
            expected = n * 2
        );
    }
    planned(&f, p).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    complete(&f, &b, submission(&b)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    verified(&f).await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert!(
        view.criteria
            .iter()
            .all(|c| c.status == CriterionStatus::Passed)
    );
    for c in view.criteria {
        let e = c.evidence.last().ok_or("evidence")?;
        let raw = f
            .owner
            .store
            .read_protected_file(Path::new(&e.raw_path))
            .await?;
        assert!(String::from_utf8(raw)?.contains("http://127.0.0.1"));
    }
    Ok(())
}
#[tokio::test]
async fn lease_expiry_keeps_resources_and_startup_reaps_real_runner_group() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let (wt, _, _) = f.owner.team_prepare_workspace(&c).await?;
    sqlx::query(
        "UPDATE team_attempt_contexts SET lease_until='2000-01-01T00:00:00Z' WHERE run_id=?",
    )
    .bind(&c.identity.attempt_id)
    .execute(f.owner.store.pool())
    .await?;
    assert!(f.owner.team_claim_next(8).await?.is_none());
    let worker = f.owner.clone();
    let cc = c.clone();
    let check = TeamCheck {
        id: "long".into(),
        argv: vec![
            "node".into(),
            "-e".into(),
            "console.log('running');setInterval(()=>{},1000)".into(),
        ],
        expected_exit: 0,
        stdout_contains: None,
        timeout_secs: 20,
    };
    let running =
        tokio::spawn(async move { worker.team_run_check(&cc, &check, &wt, "long").await });
    let lease_path = c.run_dir.join("checks/long/process-lease.json");
    for _ in 0..100 {
        if lease_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let lease = agentflow_process_supervisor::read_process_lease(&lease_path).await?;
    running.abort();
    let _ = running.await;
    f.owner.team_recover().await?;
    assert_ne!(
        agentflow_process_supervisor::inspect_process_lease(&lease),
        agentflow_process_supervisor::LeaseState::Alive
    );
    assert!(!f.owner.store.team_identity_valid(&c.identity).await?);
    Ok(())
}
#[tokio::test]
async fn i09_rework_plan_and_member_replacement_do_not_reset_global_limits() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    for index in 0..4 {
        f.owner
            .team_request_rework(&f.task, &["a".into()], &format!("same issue {index}"), None)
            .await?;
    }
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(view.reworks_used, 3);
    assert_eq!(view.phase, TeamPhase::Blocked);
    let attempts = view.attempts_used;
    f.owner
        .team_action(
            &f.task,
            TeamAction::ReplaceMember {
                member: view.config.members[1].clone(),
            },
        )
        .await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(after.attempts_used, attempts);
    assert_eq!(after.reworks_used, 3);
    Ok(())
}

#[tokio::test]
async fn plan_approval_cannot_be_bypassed_by_pause_resume() -> TestResult {
    let f = fixture().await?;
    let mut view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    view.config.require_plan_approval = true;
    sqlx::query("UPDATE team_tasks SET config_json=? WHERE task_id=?")
        .bind(encode(&view.config)?)
        .bind(&f.task)
        .execute(f.owner.store.pool())
        .await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let mut s = submission(&c);
    s.plan = Some(plan());
    complete(&f, &c, s).await?;
    assert!(f.owner.team_claim_next(8).await?.is_none());
    f.owner.team_action(&f.task, TeamAction::Pause).await?;
    f.owner.team_action(&f.task, TeamAction::Resume).await?;
    let resumed = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let report_path = resumed.report_path.as_ref().ok_or("report")?;
    let report: TeamView = serde_json::from_slice(
        &f.owner
            .store
            .read_protected_file(Path::new(report_path))
            .await?,
    )?;
    assert_eq!(report.phase, TeamPhase::PlanApproval);
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::PlanApproval
    );
    assert!(f.owner.team_claim_next(8).await?.is_none());
    f.owner
        .team_action(
            &f.task,
            TeamAction::ApprovePlan {
                expected_revision: 1,
            },
        )
        .await?;
    assert_eq!(claim(&f).await?.spec.kind, TeamWorkKind::Coverage);
    Ok(())
}

#[tokio::test]
async fn config_changes_invalidate_each_current_criterion_and_raw_evidence_is_inspectable()
-> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let evidence = view.criteria[0].evidence.last().ok_or("evidence")?;
    assert!(
        f.owner
            .team_evidence_read(&f.task, &evidence.id)
            .await?
            .contains("positive ok")
    );
    assert!(
        f.owner
            .team_evidence_read("another-task", &evidence.id)
            .await
            .is_err()
    );
    tokio::fs::create_dir_all(f.repo.join(".agentflow")).await?;
    tokio::fs::write(
        f.repo.join(".agentflow/project.toml"),
        "schema_version = 1\n",
    )
    .await?;
    let changed = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert!(
        changed
            .criteria
            .iter()
            .all(|c| c.stale && c.status == CriterionStatus::NotRun)
    );
    assert!(
        changed
            .gate_blockers
            .iter()
            .any(|b| b.contains("configuration"))
    );
    assert!(f.owner.team_assert_gate(&f.task).await.is_err());
    Ok(())
}
#[tokio::test]
async fn i10_lost_delivery_response_reconciles_tree_without_second_side_effect() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    f.owner.team_action(&f.task, TeamAction::Deliver).await?;
    let head = git(&f.repo, &["rev-parse", "HEAD"]).await?;
    let op: String = sqlx::query_scalar(
        "SELECT id FROM task_operations WHERE task_id=? AND kind='team_deliver'",
    )
    .bind(&f.task)
    .fetch_one(f.owner.store.pool())
    .await?;
    sqlx::query("UPDATE task_operations SET status='RUNNING',phase='result_unknown' WHERE id=?")
        .bind(&op)
        .execute(f.owner.store.pool())
        .await?;
    sqlx::query("UPDATE team_tasks SET phase='delivering' WHERE task_id=?")
        .bind(&f.task)
        .execute(f.owner.store.pool())
        .await?;
    tokio::fs::write(
        f.repo.join(".git/HEAD.lock"),
        format!("agentflow-team-delivery:{op}\n"),
    )
    .await?;
    f.owner.team_recover().await?;
    assert!(!f.repo.join(".git/HEAD.lock").exists());
    assert_eq!(git(&f.repo, &["rev-parse", "HEAD"]).await?, head);
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Delivered
    );
    Ok(())
}
#[tokio::test]
async fn refreshed_target_keeps_development_but_requires_fresh_candidate_and_checks() -> TestResult
{
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    tokio::fs::write(f.repo.join("external.txt"), "keep external change\n").await?;
    git(&f.repo, &["add", "."]).await?;
    git(&f.repo, &["commit", "-qm", "external target change"]).await?;
    f.owner
        .team_action(&f.task, TeamAction::RefreshTarget)
        .await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert!(after.candidate.is_none());
    assert!(
        after
            .items
            .iter()
            .filter(|i| i.spec.kind == TeamWorkKind::Implement)
            .all(|i| i.state == TeamWorkState::Accepted)
    );
    assert!(
        after
            .criteria
            .iter()
            .all(|c| c.stale && c.status == CriterionStatus::NotRun)
    );
    let coverage = claim(&f).await?;
    complete(&f, &coverage, submission(&coverage)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    verified(&f).await?;
    let r = claim(&f).await?;
    complete(&f, &r, submission(&r)).await?;
    f.owner.team_action(&f.task, TeamAction::Deliver).await?;
    assert_eq!(
        tokio::fs::read_to_string(f.repo.join("external.txt")).await?,
        "keep external change\n"
    );
    Ok(())
}

#[tokio::test]
async fn unknown_process_identity_is_not_treated_as_a_free_resource() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    tokio::fs::create_dir_all(&c.run_dir).await?;
    let lease = agentflow_process_supervisor::ProcessLease {
        pid: std::process::id(),
        process_group: std::process::id(),
        os_started_at_secs: 0,
        started_at: Utc::now().to_rfc3339(),
        owner_pid: 0,
        program: "unknown".into(),
    };
    tokio::fs::write(
        c.run_dir.join("process-lease.json"),
        serde_json::to_vec(&lease)?,
    )
    .await?;
    assert!(f.owner.team_unreaped_resources(&c).await?);
    assert!(f.owner.team_claim_next(8).await?.is_none());
    assert!(f.owner.store.team_identity_valid(&c.identity).await?);
    f.owner.team_recover().await?;
    assert!(!f.owner.store.team_identity_valid(&c.identity).await?);
    // Even a delayed execution callback cannot release an unknown live group's reservation.
    f.owner.team_execute_claim(c.clone()).await?;
    let status: String = sqlx::query_scalar("SELECT status FROM agent_runs WHERE id=?")
        .bind(&c.identity.attempt_id)
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(status, "RUNNING");
    let slots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_slots WHERE run_id=?")
        .bind(&c.identity.attempt_id)
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(slots, 1);
    Ok(())
}

#[tokio::test]
async fn resume_after_login_rechecks_blocked_work_without_resetting_budget() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let old = claim(&f).await?;
    f.owner
        .store
        .team_finish(&old, false, None, "authentication required", Some("auth"))
        .await?;
    f.owner.team_progress(&f.task).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Blocked
    );
    f.owner.team_action(&f.task, TeamAction::Resume).await?;
    let next = claim(&f).await?;
    assert_ne!(next.identity.attempt_id, old.identity.attempt_id);
    assert_eq!(
        f.owner
            .team_get(&f.task)
            .await?
            .ok_or("view")?
            .attempts_used,
        2
    );
    Ok(())
}

#[tokio::test]
async fn rejected_coverage_replans_provisional_checks_without_losing_original_goal() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let mut s = submission(&c);
    s.plan = Some(plan());
    complete(&f, &c, s).await?;
    let coverage = claim(&f).await?;
    let mut rejected = submission(&coverage);
    rejected.approved = false;
    rejected.issues = vec!["a: provisional check is weaker than its declared expectation".into()];
    complete(&f, &coverage, rejected).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Planning
    );
    let fix = claim(&f).await?;
    assert_eq!(fix.spec.kind, TeamWorkKind::Plan);
    let mut p = plan();
    p.criteria[0].checks[0].argv[2].push_str(";console.log('extra assertion context')");
    let mut s = submission(&fix);
    s.plan = Some(p);
    complete(&f, &fix, s).await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(after.plan_revision, 2);
    assert_eq!(after.attempts_used, 3);
    assert_eq!(after.requirements.len(), 3);
    assert_eq!(claim(&f).await?.spec.kind, TeamWorkKind::Coverage);
    Ok(())
}

#[tokio::test]
async fn explicit_acceptance_revision_keeps_source_and_limits_but_invalidates_evidence()
-> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let old = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let mut criteria = old.plan.as_ref().ok_or("plan")?.criteria.clone();
    criteria[0].checks[0].argv[2].push_str(";console.log('corrected explicit check')");
    let mut automatic = old.plan.clone().ok_or("plan")?;
    automatic.criteria = criteria.clone();
    assert!(
        f.owner
            .team_action(
                &f.task,
                TeamAction::RevisePlan {
                    expected_revision: old.plan_revision,
                    plan: automatic
                }
            )
            .await
            .is_err()
    );
    let view=f.owner.team_action(&f.task,TeamAction::ReviseAcceptance{expected_revision:old.plan_revision,criteria,reason:"operator corrects an invalid test implementation; original expected behavior unchanged".into()}).await?;
    assert_eq!(view.attempts_used, old.attempts_used);
    assert_eq!(view.reworks_used, old.reworks_used);
    assert!(view.candidate.is_none());
    assert!(
        view.criteria
            .iter()
            .all(|c| c.stale && c.status == CriterionStatus::NotRun)
    );
    assert!(
        view.items
            .iter()
            .filter(|w| w.spec.kind == TeamWorkKind::Implement)
            .all(|w| w.state == TeamWorkState::Accepted)
    );
    let actor: String = sqlx::query_scalar(
        "SELECT actor FROM events WHERE task_id=? AND event_type='team:acceptance_authorized'",
    )
    .bind(&f.task)
    .fetch_one(f.owner.store.pool())
    .await?;
    assert_eq!(actor, "human");
    Ok(())
}
#[tokio::test]
async fn blocked_work_does_not_prevent_unrelated_ready_item_from_running() -> TestResult {
    let f = fixture().await?;
    planned(&f, plan()).await?;
    let a = claim(&f).await?;
    f.owner
        .store
        .team_finish(&a, false, None, "member login missing", Some("auth"))
        .await?;
    f.owner.team_progress(&f.task).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Running
    );
    let b = claim(&f).await?;
    assert_eq!(b.spec.id, "b");
    complete(&f, &b, submission(&b)).await?;
    assert_eq!(
        f.owner.team_get(&f.task).await?.ok_or("view")?.phase,
        TeamPhase::Blocked
    );
    Ok(())
}

#[tokio::test]
async fn invalid_test_package_is_blocked_without_product_rework() -> TestResult {
    let f = fixture().await?;
    let mut p = plan();
    p.criteria[1].checks[0].argv[2] = "const broken='actual\nnewline';".into();
    planned(&f, p).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    complete(&f, &b, submission(&b)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    verified(&f).await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(view.criteria[1].status, CriterionStatus::Blocked);
    assert_eq!(view.reworks_used, 0);
    assert_eq!(view.phase, TeamPhase::Blocked);
    assert!(
        view.criteria[1]
            .evidence
            .last()
            .ok_or("evidence")?
            .detail
            .contains("test package")
    );
    Ok(())
}

#[tokio::test]
async fn cli_environment_notes_do_not_override_real_runner_results() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    let c = claim(&f).await?;
    let mut s = submission(&c);
    s.check_ids = vec!["check-positive".into(), "check-negative".into()];
    s.environment_notes =
        vec!["CLI sandbox cannot listen on localhost; invoke the authorized backend Runner".into()];
    complete(&f, &c, s).await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert!(
        view.criteria
            .iter()
            .all(|c| c.status == CriterionStatus::Passed)
    );
    assert_eq!(
        view.items
            .iter()
            .find(|w| w.spec.id == "__verify")
            .ok_or("verify")?
            .state,
        TeamWorkState::Accepted
    );
    let r = claim(&f).await?;
    assert_eq!(r.spec.kind, TeamWorkKind::Review);
    assert!(
        r.input["actualRunnerEvidence"]
            .as_array()
            .is_some_and(|v| v.len() == 2)
    );
    Ok(())
}

#[tokio::test]
async fn occupied_port_is_environment_blockage_not_product_failure() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let (wt, _, _) = f.owner.team_prepare_workspace(&c).await?;
    let listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    let port = listener.local_addr()?.port();
    let check = TeamCheck {
        id: "port".into(),
        argv: vec![
            "node".into(),
            "-e".into(),
            format!("require('node:http').createServer().listen({port},'127.0.0.1')"),
        ],
        expected_exit: 0,
        stdout_contains: None,
        timeout_secs: 5,
    };
    let (status, detail, _, _) = f.owner.team_run_check(&c, &check, &wt, "port").await?;
    assert_eq!(status, CriterionStatus::Blocked);
    assert!(detail.contains("environment port"));
    drop(listener);
    Ok(())
}
#[tokio::test]
async fn sensitive_committee_keeps_cross_vendor_rule_when_general_mode_is_weaker() -> TestResult {
    let f = fixture().await?;
    let mut view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    view.config.cross_vendor_review = false;
    view.config.members[3].roles.push(TeamRole::Reviewer);
    sqlx::query("UPDATE team_tasks SET config_json=? WHERE task_id=?")
        .bind(encode(&view.config)?)
        .bind(&f.task)
        .execute(f.owner.store.pool())
        .await?;
    let mut p = plan();
    p.items[0].write_globs = vec!["auth.cjs".into()];
    planned(&f, p).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    files(&f, &a, &[("auth.cjs", "module.exports=n=>n*2;\n")]).await?;
    complete(&f, &b, submission(&b)).await?;
    let i = claim(&f).await?;
    f.owner.team_execute_claim(i).await?;
    // Controlled executor makes the review dependency ready; absent criterion evidence still
    // blocks final delivery. This case isolates reviewer eligibility, without inventing tests.
    sqlx::query(
        "UPDATE team_work_items SET state='output_accepted' WHERE task_id=? AND id='__verify'",
    )
    .bind(&f.task)
    .execute(f.owner.store.pool())
    .await?;
    let r = claim(&f).await?;
    assert_eq!(r.provider, Some(AgentKind::ClaudeCode));
    complete(&f, &r, submission(&r)).await?;
    assert!(f.owner.team_claim_next(8).await?.is_none());
    let result = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(result.phase, TeamPhase::Blocked);
    assert!(result.gate_blockers.iter().any(|b| b.contains("1/2")));
    Ok(())
}

#[tokio::test]
async fn online_reconciliation_reclaims_only_proven_resources_and_keeps_limits() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let old = claim(&f).await?;
    let (wt, _, _) = f.owner.team_prepare_workspace(&old).await?;
    let check = TeamCheck {
        id: "proof".into(),
        argv: vec![
            "node".into(),
            "-e".into(),
            "console.log('proof');setTimeout(()=>{},500)".into(),
        ],
        expected_exit: 0,
        stdout_contains: Some("proof".into()),
        timeout_secs: 5,
    };
    let worker = f.owner.clone();
    let cc = old.clone();
    let check_workspace = wt.clone();
    let running = tokio::spawn(async move {
        worker
            .team_run_check(&cc, &check, &check_workspace, "proof")
            .await
    });
    let lease_path = old.run_dir.join("checks/proof/process-lease.json");
    for _ in 0..100 {
        if lease_path.exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let bytes = tokio::fs::read(&lease_path).await?;
    running.await??;
    let mut lease: agentflow_process_supervisor::ProcessLease = serde_json::from_slice(&bytes)?;
    lease.os_started_at_secs = 0;
    tokio::fs::write(&lease_path, serde_json::to_vec(&lease)?).await?;
    sqlx::query("UPDATE team_attempt_contexts SET cancel_requested=1,failure_class='result_unknown' WHERE run_id=?").bind(&old.identity.attempt_id).execute(f.owner.store.pool()).await?;
    f.owner
        .team_set_phase(&f.task, TeamPhase::Blocked, Some("unknown"))
        .await?;
    let before = f.owner.team_recovery_get(&f.task).await?;
    assert_eq!(before.resources[0].state, TeamResourceState::Unknown);
    f.owner
        .team_action(&f.task, TeamAction::ReconcileResources)
        .await?;
    assert!(
        f.owner
            .team_action(&f.task, TeamAction::Resume)
            .await
            .is_err()
    );
    let slots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_slots WHERE run_id=?")
        .bind(&old.identity.attempt_id)
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(slots, 1);
    let rejected = f
        .owner
        .team_complete_submission(
            &old,
            submission(&old),
            &wt,
            &f.owner.git.resolve(&wt, "HEAD").await?,
            vec![],
        )
        .await;
    assert!(rejected.is_err());
    let slots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_slots WHERE run_id=?")
        .bind(&old.identity.attempt_id)
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(slots, 1);
    // Restore the original recorded identity of the real, now exited check process.
    tokio::fs::write(&lease_path, bytes).await?;
    f.owner
        .team_action(&f.task, TeamAction::ReconcileResources)
        .await?;
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(after.items[0].state, TeamWorkState::Blocked);
    assert_eq!(after.attempts_used, 1);
    assert_eq!(after.reworks_used, 0);
    assert!(
        f.owner
            .team_recovery_get(&f.task)
            .await?
            .resources
            .is_empty()
    );
    f.owner
        .team_action(&f.task, TeamAction::ReconcileResources)
        .await?;
    let count: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM events WHERE team_event_key=?")
        .bind(format!("{}:resources_reconciled", old.identity.attempt_id))
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(count, 1);
    f.owner.team_action(&f.task, TeamAction::Resume).await?;
    let next = claim(&f).await?;
    assert!(next.identity.lease_epoch > old.identity.lease_epoch);
    assert!(
        !f.owner
            .store
            .team_finish(&old, true, None, "late old output", None)
            .await?
    );
    assert!(f.owner.store.team_identity_valid(&next.identity).await?);
    Ok(())
}
#[tokio::test]
async fn online_reconciliation_never_reaps_a_live_supervisor_or_resurrects_cancellation()
-> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    f.owner.team_prepare_workspace(&c).await?;
    let key = format!("team/{}/{}", f.task, c.identity.attempt_id);
    f.owner
        .register_cancellation(&key, CancellationToken::new());
    f.owner.team_action(&f.task, TeamAction::Cancel).await?;
    f.owner
        .team_action(&f.task, TeamAction::ReconcileResources)
        .await?;
    assert_eq!(f.owner.team_recovery_get(&f.task).await?.resources.len(), 1);
    f.owner.unregister_cancellation(&key);
    f.owner
        .team_action(&f.task, TeamAction::ReconcileResources)
        .await?;
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(view.phase, TeamPhase::Cancelled);
    assert!(
        view.items
            .iter()
            .all(|w| w.state == TeamWorkState::Cancelled)
    );
    assert!(
        f.owner
            .team_recovery_get(&f.task)
            .await?
            .resources
            .is_empty()
    );
    assert!(f.owner.team_claim_next(8).await?.is_none());
    Ok(())
}
#[tokio::test]
async fn plan_preview_is_read_only_and_preserves_unrelated_inflight_work() -> TestResult {
    let f = fixture().await?;
    let draft = f
        .owner
        .team_plan_preview(TeamPlanPreviewInput {
            task_id: f.task.clone(),
            expected_revision: 0,
            plan: plan(),
        })
        .await?;
    assert!(!draft.blockers.is_empty());
    assert!(
        f.owner
            .team_action(
                &f.task,
                TeamAction::RevisePlan {
                    expected_revision: 0,
                    plan: plan()
                }
            )
            .await
            .is_err()
    );
    planned(&f, plan()).await?;
    let a = claim(&f).await?;
    let b = claim(&f).await?;
    complete(&f, &a, submission(&a)).await?;
    let before = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let mut new = plan();
    new.items[0].instructions = "revised a".into();
    let input = TeamPlanPreviewInput {
        task_id: f.task.clone(),
        expected_revision: before.plan_revision,
        plan: new.clone(),
    };
    let preview = f.owner.team_plan_preview(input).await?;
    assert_eq!(preview.affected, vec!["a"]);
    assert_eq!(preview.preserved, vec!["b"]);
    assert!(preview.blockers.is_empty());
    let unchanged = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(unchanged.plan_revision, before.plan_revision);
    assert_eq!(unchanged.artifacts.len(), before.artifacts.len());
    assert!(f.owner.store.team_identity_valid(&b.identity).await?);
    f.owner
        .team_action(
            &f.task,
            TeamAction::RevisePlan {
                expected_revision: before.plan_revision,
                plan: new,
            },
        )
        .await?;
    assert!(f.owner.store.team_identity_valid(&b.identity).await?);
    let mut weaker = plan();
    weaker.criteria[0].checks.clear();
    let preview = f
        .owner
        .team_plan_preview(TeamPlanPreviewInput {
            task_id: f.task.clone(),
            expected_revision: before.plan_revision,
            plan: weaker,
        })
        .await?;
    assert!(!preview.blockers.is_empty());
    Ok(())
}
#[tokio::test]
async fn replan_cannot_delete_active_program_integration_missing_from_member_list() -> TestResult {
    let f = fixture().await?;
    developed(&f).await?;
    // Reset only integration in this controlled fixture, then reserve its canonical run.
    sqlx::query("UPDATE team_work_items SET state='pending' WHERE task_id=? AND id='__integrate'")
        .bind(&f.task)
        .execute(f.owner.store.pool())
        .await?;
    let i = claim(&f).await?;
    assert_eq!(i.spec.kind, TeamWorkKind::Integrate);
    let view = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let p = view.plan.clone().ok_or("plan")?;
    let preview = f
        .owner
        .team_plan_preview(TeamPlanPreviewInput {
            task_id: f.task.clone(),
            expected_revision: view.plan_revision,
            plan: p.clone(),
        })
        .await?;
    assert!(!preview.blockers.is_empty());
    assert!(
        f.owner
            .team_action(
                &f.task,
                TeamAction::RevisePlan {
                    expected_revision: view.plan_revision,
                    plan: p
                }
            )
            .await
            .is_err()
    );
    assert!(f.owner.store.team_identity_valid(&i.identity).await?);
    Ok(())
}

#[tokio::test]
async fn report_export_verifies_content_retains_history_and_removes_failed_packages() -> TestResult
{
    let f = fixture().await?;
    developed(&f).await?;
    verified(&f).await?;
    let before = f.owner.team_get(&f.task).await?.ok_or("view")?;
    let parent = f._root.path().join("exports");
    tokio::fs::create_dir(&parent).await?;
    tokio::fs::write(parent.join("user.txt"), "keep user data").await?;
    let input = TeamReportExportInput {
        task_id: f.task.clone(),
        parent_directory: parent.to_string_lossy().into_owned(),
    };
    let exported = f.owner.team_report_export(input.clone()).await?;
    assert_eq!(exported.evidence_count, 2);
    let dir = PathBuf::from(&exported.directory);
    let manifest = tokio::fs::read(dir.join("manifest.json")).await?;
    assert_eq!(hex_digest(&manifest), exported.manifest_sha256);
    let parsed: Value = serde_json::from_slice(&manifest)?;
    let records = parsed["evidence"].as_array().ok_or("records")?;
    for record in records {
        let display = tokio::fs::read(dir.join(record["path"].as_str().ok_or("path")?)).await?;
        assert_eq!(hex_digest(&display), record["exportedDisplaySha256"]);
        assert!(
            record["originalSha256"]
                .as_str()
                .is_some_and(|s| s.len() == 64)
        );
    }
    assert!(!dir.join("local-data.key").exists());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(std::fs::metadata(&dir)?.permissions().mode() & 0o777, 0o700);
        assert_eq!(
            std::fs::metadata(dir.join("team-report.json"))?
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let after = f.owner.team_get(&f.task).await?.ok_or("view")?;
    assert_eq!(after.phase, before.phase);
    assert_eq!(after.attempts_used, before.attempts_used);
    let evidence = before.criteria[0].evidence.first().ok_or("evidence")?;
    tokio::fs::write(&evidence.raw_path, b"tampered").await?;
    assert!(f.owner.team_report_export(input).await.is_err());
    assert_eq!(std::fs::read_dir(&parent)?.count(), 2); // existing user file + complete first export
    assert_eq!(
        tokio::fs::read_to_string(parent.join("user.txt")).await?,
        "keep user data"
    );
    Ok(())
}

#[tokio::test]
async fn startup_recovery_includes_an_orphaned_readiness_probe_group() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    let (workspace, _, _) = f.owner.team_prepare_workspace(&c).await?;
    let dir = c.run_dir.join("probe");
    tokio::fs::create_dir_all(&dir).await?;
    sqlx::query("UPDATE team_attempt_contexts SET start_intent=? WHERE run_id=?")
        .bind(format!("probe:{}", dir.display()))
        .bind(&c.identity.attempt_id)
        .execute(f.owner.store.pool())
        .await?;
    let spec = agentflow_process_supervisor::ProcessSpec {
        program: "node".into(),
        args: vec!["-e".into(), "setInterval(()=>{},1000)".into()],
        cwd: workspace,
        env: std::env::vars()
            .filter(|(k, _)| matches!(k.as_str(), "PATH" | "HOME"))
            .collect(),
        clear_environment: true,
        env_denylist: vec![],
        timeout: Duration::from_secs(30),
        idle_timeout: Duration::from_secs(30),
        stdout_path: dir.join("stdout.log"),
        stderr_path: dir.join("stderr.log"),
        lease_path: dir.join("process-lease.json"),
    };
    let (tx, mut rx) = mpsc::channel(16);
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let running = tokio::spawn(agentflow_process_supervisor::run(
        spec,
        CancellationToken::new(),
        tx,
    ));
    for _ in 0..100 {
        if dir.join("process-lease.json").exists() {
            break;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    let lease =
        agentflow_process_supervisor::read_process_lease(&dir.join("process-lease.json")).await?;
    running.abort();
    let _ = running.await;
    drain.abort();
    assert!(f.owner.team_unreaped_resources(&c).await?);
    f.owner.team_recover().await?;
    assert_eq!(
        agentflow_process_supervisor::inspect_process_group_lease(&lease).await?,
        Some(false)
    );
    assert!(
        f.owner
            .team_recovery_get(&f.task)
            .await?
            .resources
            .is_empty()
    );
    assert_eq!(
        f.owner
            .team_get(&f.task)
            .await?
            .ok_or("view")?
            .attempts_used,
        1
    );
    Ok(())
}

#[tokio::test]
async fn pre_managed_probe_history_without_a_lease_is_quarantined_not_invented() -> TestResult {
    let f = fixture().await?;
    f.owner.team_action(&f.task, TeamAction::Start).await?;
    let c = claim(&f).await?;
    f.owner.team_prepare_workspace(&c).await?;
    // Historical-format fixture: no resource protocol marker, no probe lease/outcome.
    sqlx::query("UPDATE team_attempt_contexts SET input_json=json_remove(input_json,'$.resourceProtocolVersion') WHERE run_id=?")
        .bind(&c.identity.attempt_id).execute(f.owner.store.pool()).await?;
    f.owner.team_recover().await?;
    let recovery = f.owner.team_recovery_get(&f.task).await?;
    assert_eq!(recovery.resources.len(), 1);
    assert_eq!(recovery.resources[0].state, TeamResourceState::Unknown);
    assert!(!f.owner.store.team_identity_valid(&c.identity).await?);
    let slots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM provider_slots WHERE run_id=?")
        .bind(&c.identity.attempt_id)
        .fetch_one(f.owner.store.pool())
        .await?;
    assert_eq!(slots, 1);
    Ok(())
}
