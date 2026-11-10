//! Explicit real-CLI acceptance harness. Never runs in cargo test or modifies the source repo.
//! Usage: cargo run -p agentflow-orchestrator --example team_smoke -- /tmp/agentflow-real-team-<unique>
use agentflow_contracts::*;
use agentflow_orchestrator::Orchestrator;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{process::Command, task::JoinSet};
async fn git(repo: &Path, args: &[&str]) -> anyhow::Result<()> {
    let out = Command::new("git")
        .current_dir(repo)
        .args(args)
        .output()
        .await?;
    anyhow::ensure!(
        out.status.success(),
        "git failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    Ok(())
}
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--recovery") {
        let root = PathBuf::from(
            std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("fresh recovery fixture required"))?,
        );
        return real_recovery(root).await;
    }
    if matches!(
        std::env::args().nth(1).as_deref(),
        Some("--resume" | "--retry")
    ) {
        let root = PathBuf::from(
            std::env::args()
                .nth(2)
                .ok_or_else(|| anyhow::anyhow!("resume root required"))?,
        );
        let owner = Orchestrator::open(root.join("data")).await?;
        let task: String =
            sqlx::query_scalar("SELECT task_id FROM team_tasks ORDER BY created_at DESC LIMIT 1")
                .fetch_one(owner.store.pool())
                .await?;
        if std::env::args().nth(1).as_deref() == Some("--retry") {
            let work = std::env::args()
                .nth(3)
                .ok_or_else(|| anyhow::anyhow!("work item ID required"))?;
            owner
                .team_action(&task, TeamAction::RetryWork { work_item_id: work })
                .await?;
        }
        let state = owner
            .team_get(&task)
            .await?
            .ok_or_else(|| anyhow::anyhow!("team missing"))?;
        if matches!(state.phase, TeamPhase::Paused | TeamPhase::Blocked) {
            owner.team_action(&task, TeamAction::Resume).await?;
        }
        return drive(owner, root, task).await;
    }
    let conflict = std::env::args().nth(1).as_deref() == Some("--conflict");
    let crash = std::env::args().nth(1).as_deref() == Some("--crash-on-implement");
    let plan_only = std::env::args().nth(1).as_deref() == Some("--plan-only");
    let root = PathBuf::from(
        std::env::args()
            .nth(if conflict || crash || plan_only { 2 } else { 1 })
            .ok_or_else(|| anyhow::anyhow!("provide a fresh disposable output directory"))?,
    );
    anyhow::ensure!(
        !root.exists(),
        "output path already exists; choose a fresh path rather than overwriting it"
    );
    let repo = root.join("repo");
    tokio::fs::create_dir_all(&repo).await?;
    git(&repo, &["init", "-q", "-b", "main"]).await?;
    git(&repo, &["config", "user.name", "AgentFlow Acceptance"]).await?;
    git(
        &repo,
        &["config", "user.email", "acceptance@agentflow.local"],
    )
    .await?;
    if conflict {
        // Two real CLI members change different exports on the same baseline line. Git must
        // reconcile their actual deltas; the harness never injects an accepted artifact/result.
        tokio::fs::write(repo.join("core.cjs"), "exports.a=n=>0;exports.b=n=>0;\n").await?;
        tokio::fs::write(repo.join("server.cjs"), "const http=require('node:http');exports.create=()=>http.createServer((q,r)=>{const u=new URL(q.url,'http://localhost');const f=u.pathname==='/positive'?require('./core.cjs').a:require('./core.cjs').b;r.end(String(f(Number(u.searchParams.get('n')))));});\n").await?;
    } else {
        tokio::fs::write(repo.join("a.cjs"), "module.exports=n=>0;\n").await?;
        tokio::fs::write(repo.join("b.cjs"), "module.exports=n=>0;\n").await?;
        tokio::fs::write(repo.join("server.cjs"),"const http=require('node:http');exports.create=()=>http.createServer((q,r)=>{const u=new URL(q.url,'http://localhost');const f=u.pathname==='/positive'?require('./a.cjs'):require('./b.cjs');r.end(String(f(Number(u.searchParams.get('n')))));});\n").await?;
    }
    git(&repo, &["add", "."]).await?;
    git(&repo, &["commit", "-qm", "immutable baseline HTTP harness"]).await?;
    let owner = Orchestrator::open(root.join("data")).await?;
    owner
        .settings_update(&GlobalSettings {
            max_concurrent_runs: Some(4),
            default_provider_max_concurrent: 2,
            ..GlobalSettings::default()
        })
        .await?;
    let project = owner.project_import(&repo).await?;
    let config = TeamConfig {
        team_capacity: 4,
        max_concurrent_attempts: 2,
        planner_member_id: "lead".into(),
        members: vec![
            TeamMember {
                id: "lead".into(),
                name: "Claude planner/reviewer".into(),
                roles: vec![TeamRole::Planner, TeamRole::Reviewer],
                providers: vec![AgentKind::ClaudeCode, AgentKind::Codex],
            },
            TeamMember {
                id: "dev-a".into(),
                name: "Codex A".into(),
                roles: vec![TeamRole::Developer],
                providers: vec![AgentKind::Codex],
            },
            TeamMember {
                id: "dev-b".into(),
                name: "Codex B".into(),
                roles: vec![TeamRole::Developer],
                providers: vec![AgentKind::Codex],
            },
            TeamMember {
                id: "tester".into(),
                name: "Independent Codex tester".into(),
                roles: vec![TeamRole::Tester],
                providers: vec![AgentKind::Codex],
            },
        ],
        max_attempts: if conflict { 24 } else { 20 },
        verification_reserve: if conflict { 6 } else { 4 },
        max_reworks: if conflict { 3 } else { 2 },
        max_plan_revisions: 3,
        attempt_timeout_secs: 180,
        write_globs: if conflict {
            vec!["core.cjs".into()]
        } else {
            vec!["a.cjs".into(), "b.cjs".into()]
        },
        allowed_commands: vec![vec!["node".into()]],
        require_plan_approval: plan_only,
        cross_vendor_review: true,
    };
    let goal = if conflict {
        "Implement the two independent doubling exports in core.cjs while preserving the existing immutable server.cjs. Plan TWO independent work items, with NO dependency on each other, assigned to dev-a and dev-b. dev-a changes ONLY exports.a to n=>n*2, preserving exports.b EXACTLY as its input; dev-b changes ONLY exports.b to n=>n*2, preserving exports.a EXACTLY as its input. Both work items authorize core.cjs. Preserve the single-line layout to exercise real Git integration conflict handling. Each item's local checks assert only its owned export. On integration conflict the responsible member must reconcile on the supplied partial candidate, preserving the other member's completed export. Final acceptance must assert BOTH exports double positive, negative and zero inputs, preserve server.cjs bytes (use SHA256 of the actual baseline file), AND actually start require('./server.cjs').create(), listen on 127.0.0.1 port 0, fetch /positive?n=2 (body 4) and /negative?n=-2 (body -4), then close in finally. Use node -e argv scripts; no added test files. Ensure syntactically valid scripts with no unescaped newlines in literals. Keep all check IDs in tester.checkIds. Submit source results for backend local checks even if CLI sandbox cannot listen; never claim unexecuted tests passed."
    } else {
        "Complete the two independent CommonJS doubling modules in this existing local project: a.cjs handles positive values and b.cjs handles negative values. Preserve the existing server.cjs harness. Plan TWO independent work items with no dependency on each other, assigned to dev-a and dev-b respectively, so both Codex members can work concurrently. Each exports one function n=>n*2. Acceptance must actually start the existing server via require('./server.cjs').create(), listen on port 0 at 127.0.0.1 and fetch each corresponding route, assert the response, then close the server in finally. Use node -e argv assertion scripts in the acceptance contract; no product test file changes. Keep all required check IDs in tester.checkIds. Do not invent evidence or edit the immutable server harness."
    };
    let view=owner.team_create(TeamCreateInput{project_id:project.id,title:if conflict {"Real CLI shared-file integration conflict"} else {"Real CLI team HTTP doubling"}.into(),goal:goal.into(),requirements:vec!["GET /positive?n=2 responds with body 4, verified through a real local HTTP request".into(),"GET /negative?n=-2 responds with body -4, verified through a real local HTTP request".into()],target_branch:None,config}).await?;
    let task = view.task_id;
    if crash {
        tokio::fs::write(root.join("resilience-fixture.json"),serde_json::to_vec_pretty(&serde_json::json!({"taskId":task,"scenario":"real owner crash during Codex execution, then target branch advance before delivery","limits":"original fixed task budget is preserved"}))?).await?;
    }
    owner.team_action(&task, TeamAction::Start).await?;
    drive(owner, root, task).await
}
async fn real_recovery(root: PathBuf) -> anyhow::Result<()> {
    anyhow::ensure!(
        !root.exists(),
        "recovery fixture already exists; resume it rather than resetting its budget"
    );
    let log = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.with_extension("crash.log"))?;
    let status = Command::new(std::env::current_exe()?)
        .args([
            "--crash-on-implement",
            root.to_str()
                .ok_or_else(|| anyhow::anyhow!("UTF-8 path required"))?,
        ])
        .stdout(std::process::Stdio::from(log.try_clone()?))
        .stderr(std::process::Stdio::from(log))
        .status()
        .await?;
    anyhow::ensure!(
        status.code() == Some(73),
        "crash checkpoint was not reached (exit {status}); inspect preserved fixture/log, do not reset it"
    );
    // Owner startup must inspect/reap real old CLI groups before resuming the same task.
    let owner = Orchestrator::open(root.join("data")).await?;
    let task: String =
        sqlx::query_scalar("SELECT task_id FROM team_tasks ORDER BY created_at DESC LIMIT 1")
            .fetch_one(owner.store.pool())
            .await?;
    let resources = owner.team_recovery_get(&task).await?;
    tokio::fs::write(
        root.join("post-crash-resource-proof.json"),
        serde_json::to_vec_pretty(&resources)?,
    )
    .await?;
    anyhow::ensure!(
        resources.resources.is_empty(),
        "old CLI groups remain unproved; resources retained, inspect recovery facts"
    );
    let view = owner
        .team_get(&task)
        .await?
        .ok_or_else(|| anyhow::anyhow!("team missing"))?;
    anyhow::ensure!(
        view.attempts_used > 0 && view.config.max_attempts == 20,
        "recovery must preserve original counters and budget"
    );
    if view.phase == TeamPhase::Blocked {
        owner.team_action(&task, TeamAction::Resume).await?;
    }
    drive(owner, root, task).await
}
async fn drive(owner: Orchestrator, root: PathBuf, task: String) -> anyhow::Result<()> {
    println!(
        "{}",
        serde_json::json!({"taskId":task,"output":root,"verification":"REAL CLI, not deterministic executor"})
    );
    let mut jobs = JoinSet::new();
    let mut last = String::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(1200);
    loop {
        while jobs.len() < 4 {
            match owner.team_claim_next(4).await? {
                Some(c) => {
                    println!(
                        "{}",
                        serde_json::json!({"event":"claim","work":c.spec.id,"kind":c.spec.kind,"member":c.identity.member_id,"provider":c.provider,"attempt":c.identity.attempt_id})
                    );
                    let w = owner.clone();
                    jobs.spawn(async move { w.team_execute_claim(c).await });
                }
                None => break,
            }
        }
        let view = owner
            .team_get(&task)
            .await?
            .ok_or_else(|| anyhow::anyhow!("team vanished"))?;
        if std::env::args().nth(1).as_deref() == Some("--crash-on-implement") {
            let rows=sqlx::query_as::<_,(String,String)>("SELECT r.id,r.run_dir FROM agent_runs r JOIN team_work_items w ON w.task_id=r.task_id AND w.id=r.team_work_item_id WHERE r.task_id=? AND r.status='RUNNING' AND json_extract(w.spec_json,'$.kind')='implement'").bind(&task).fetch_all(owner.store.pool()).await?;
            for (attempt, dir) in rows {
                let path = PathBuf::from(dir).join("process-lease.json");
                if let Ok(lease) = agentflow_process_supervisor::read_process_lease(&path).await
                    && agentflow_process_supervisor::inspect_process_group_lease(&lease).await?
                        == Some(true)
                {
                    tokio::fs::write(root.join("crash-trigger.json"),serde_json::to_vec_pretty(&serde_json::json!({"taskId":task,"attemptId":attempt,"lease":lease,"attemptsUsed":view.attempts_used,"maxAttempts":view.config.max_attempts,"exitCode":73,"acceptedOutputInvented":false}))?).await?;
                    // Intentional source-executor crash: no cancellation/drop cleanup runs here.
                    // The parent must recover the actual CLI group, not simulated status rows.
                    std::process::exit(73);
                }
            }
        }
        let snapshot = serde_json::to_string(
            &serde_json::json!({"phase":view.phase,"work":view.items.iter().map(|w|(&w.spec.id,w.state,&w.detail)).collect::<Vec<_>>(),"candidate":view.candidate.as_ref().map(|c|&c.id),"criteria":view.criteria.iter().map(|c|(&c.criterion.id,c.status,c.stale)).collect::<Vec<_>>(),"detail":view.detail}),
        )?;
        if snapshot != last {
            println!("{snapshot}");
            last = snapshot;
        }
        if jobs.is_empty() && view.phase == TeamPhase::Delivered {
            break;
        }
        if jobs.is_empty() && view.phase == TeamPhase::ReadyForDelivery {
            if root.join("resilience-fixture.json").exists()
                && !root.join("target-change-proof.json").exists()
            {
                git(
                    &root.join("repo"),
                    &[
                        "commit",
                        "--allow-empty",
                        "-qm",
                        "external target advance before team delivery",
                    ],
                )
                .await?;
                let rejection = owner.team_action(&task, TeamAction::Deliver).await;
                anyhow::ensure!(
                    rejection.is_err(),
                    "target branch changed but old candidate was delivered"
                );
                tokio::fs::write(root.join("target-change-proof.json"),serde_json::to_vec_pretty(&serde_json::json!({"priorCandidate":view.candidate,"priorEvidence":view.criteria,"artifactIds":view.artifacts.iter().map(|a|&a.id).collect::<Vec<_>>(),"deliveryRejected":true,"reason":rejection.err().map(|e|e.to_string())}))?).await?;
                let fresh = owner.team_action(&task, TeamAction::RefreshTarget).await?;
                anyhow::ensure!(
                    fresh.candidate.is_none()
                        && fresh
                            .criteria
                            .iter()
                            .all(|c| c.status != CriterionStatus::Passed),
                    "refresh reused prior candidate/evidence"
                );
                continue;
            }
            let delivered = owner.team_action(&task, TeamAction::Deliver).await?;
            println!(
                "{}",
                serde_json::json!({"result":delivered.phase,"report":delivered.report_path,"candidate":delivered.candidate})
            );
            break;
        }
        if jobs.is_empty()
            && matches!(
                view.phase,
                TeamPhase::Blocked | TeamPhase::Cancelled | TeamPhase::PlanApproval
            )
        {
            println!(
                "{}",
                serde_json::json!({"result":"not_accepted","phase":view.phase,"detail":view.detail,"blockers":view.gate_blockers,"report":view.report_path})
            );
            break;
        }
        if tokio::time::Instant::now() >= deadline {
            owner.team_action(&task, TeamAction::Cancel).await?;
            while let Some(job) = jobs.join_next().await {
                job??;
            }
            anyhow::bail!(
                "real CLI acceptance deadline reached; task cancelled and evidence retained"
            );
        }
        tokio::select! {r=jobs.join_next(),if !jobs.is_empty()=>{if let Some(r)=r {r??;}},_=tokio::time::sleep(Duration::from_millis(250))=>{}}
    }
    tokio::fs::write(
        root.join("final-team-view.json"),
        serde_json::to_vec_pretty(&owner.team_get(&task).await?)?,
    )
    .await?;
    Ok(())
}
