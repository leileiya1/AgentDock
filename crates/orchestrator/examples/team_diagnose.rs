//! Print sanitized team execution facts, without dumping private CLI logs or credentials.
use agentflow_orchestrator::Orchestrator;
use sqlx::Row;
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let data = std::env::args()
        .nth(1)
        .ok_or_else(|| anyhow::anyhow!("data directory required"))?;
    let owner = Orchestrator::open_client(&data).await?;
    let rows=sqlx::query("SELECT r.id,r.run_dir,r.status,r.agent FROM agent_runs r WHERE r.team_work_item_id IS NOT NULL ORDER BY r.created_at").fetch_all(owner.store.pool()).await?;
    for row in rows {
        let path =
            std::path::PathBuf::from(row.get::<String, _>("run_dir")).join("last-message.json");
        let bytes = match owner.store.read_protected_file(&path).await {
            Ok(v) => v,
            Err(_) => continue,
        };
        let value: serde_json::Value = match serde_json::from_slice(&bytes) {
            Ok(v) => v,
            Err(_) => continue,
        };
        println!(
            "{}",
            serde_json::json!({"attempt":row.get::<String,_>("id"),"provider":row.get::<Option<String>,_>("agent"),"status":row.get::<String,_>("status"),"resultTaskId":value.get("task_id"),"resultRevision":value.get("revision"),"resultStatus":value.get("status"),"summary":value.get("summary"),"question":value.get("question")})
        );
    }
    Ok(())
}
