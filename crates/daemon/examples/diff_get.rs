//! Print a task revision's diff identity (commit + sha256), as the approval bar reads it.
//!   cargo run -q -p agentflow-daemon --example diff_get -- <task_id> <revision>
use agentflow_daemon::default_data_dir;
use agentflow_orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let task = std::env::args()
        .nth(1)
        .ok_or("usage: diff_get <task_id> <revision>")?;
    let rev: i64 = std::env::args().nth(2).ok_or("revision")?.parse()?;
    let dir = match std::env::var_os("AGENTFLOW_DATA_DIR") {
        Some(dir) => dir.into(),
        None => default_data_dir()?,
    };
    let read = Orchestrator::open_client(&dir).await?;
    let diff = read.diff_get(&task, rev).await?;
    let v = serde_json::to_value(&diff)?;
    println!(
        "{} {} files={}",
        v["commitSha"].as_str().unwrap_or("?"),
        v["diffSha256"].as_str().unwrap_or("?"),
        v["files"].as_array().map(Vec::len).unwrap_or(0)
    );
    Ok(())
}
