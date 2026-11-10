//! Explicit operator-approved acceptance-contract revision. No silent model repair.
//! Usage: ... -- <data-dir> <task-id> <criteria-json-file> <reason>
use agentflow_contracts::{TeamAction, TeamCriterion};
use agentflow_orchestrator::Orchestrator;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    anyhow::ensure!(
        args.len() == 4,
        "data-dir task-id criteria-file reason required"
    );
    let criteria: Vec<TeamCriterion> = serde_json::from_slice(&tokio::fs::read(&args[2]).await?)?;
    let owner = Orchestrator::open(&args[0]).await?;
    let old = owner
        .team_get(&args[1])
        .await?
        .ok_or_else(|| anyhow::anyhow!("team missing"))?;
    let view = owner
        .team_action(
            &args[1],
            TeamAction::ReviseAcceptance {
                expected_revision: old.plan_revision,
                criteria,
                reason: args[3].clone(),
            },
        )
        .await?;
    println!(
        "{}",
        serde_json::json!({"taskId":view.task_id,"phase":view.phase,"planRevision":view.plan_revision,"attemptsUsed":view.attempts_used,"reworksUsed":view.reworks_used,"authorization":"explicit operator action; recorded with original and revised criteria"})
    );
    Ok(())
}
