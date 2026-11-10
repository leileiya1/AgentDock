//! Export current backend facts and verified, redacted raw evidence; no model/Agent calls.
//! Usage: ... -- <data-dir> <task-id> <new-output-directory>
use agentflow_orchestrator::Orchestrator;
use std::path::PathBuf;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let first = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("data directory required"))?;
    let package = first == "--package";
    let data = if package {
        args.next()
            .ok_or_else(|| anyhow::anyhow!("data directory required"))?
    } else {
        first
    };
    let task = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("task ID required"))?;
    let output = PathBuf::from(
        args.next()
            .ok_or_else(|| anyhow::anyhow!("fresh output directory required"))?,
    );
    if package {
        let owner = Orchestrator::open_client(data).await?;
        let result = owner
            .team_report_export(agentflow_contracts::TeamReportExportInput {
                task_id: task,
                parent_directory: output.to_string_lossy().into_owned(),
            })
            .await?;
        println!("{}", serde_json::to_string(&result)?);
        return Ok(());
    }
    anyhow::ensure!(
        !output.exists(),
        "output directory already exists; choose a fresh path"
    );
    let owner = Orchestrator::open_client(data).await?;
    let view = owner
        .team_get(&task)
        .await?
        .ok_or_else(|| anyhow::anyhow!("not a team task"))?;
    tokio::fs::create_dir_all(output.join("evidence")).await?;
    tokio::fs::write(
        output.join("team-report.json"),
        serde_json::to_vec_pretty(&view)?,
    )
    .await?;
    let mut index = Vec::new();
    for criterion in &view.criteria {
        for evidence in &criterion.evidence {
            let raw = owner.team_evidence_read(&task, &evidence.id).await?;
            let name = format!("{}.json", evidence.id);
            tokio::fs::write(output.join("evidence").join(&name), raw).await?;
            index.push(serde_json::json!({"file":format!("evidence/{name}"),"criterionId":criterion.criterion.id,"candidateId":evidence.candidate_id,"checkId":evidence.check_id,"status":evidence.status,"originalSha256":evidence.raw_sha256,"display":"verified then redacted/bounded; original digest remains in report"}));
        }
    }
    tokio::fs::write(
        output.join("evidence-index.json"),
        serde_json::to_vec_pretty(&index)?,
    )
    .await?;
    println!(
        "{}",
        serde_json::json!({"taskId":task,"phase":view.phase,"evidenceRecords":index.len(),"output":output})
    );
    Ok(())
}
