//! Print a run's decrypted agent events (read-only), as the app's log view gets them.
//!   cargo run -q -p agentflow-daemon --example run_log -- <run_id> [max_lines]
use agentflow_daemon::default_data_dir;
use agentflow_orchestrator::Orchestrator;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let run_id = std::env::args()
        .nth(1)
        .ok_or("usage: run_log <run_id> [max]")?;
    let max: usize = std::env::args()
        .nth(2)
        .and_then(|v| v.parse().ok())
        .unwrap_or(400);
    let dir = match std::env::var_os("AGENTFLOW_DATA_DIR") {
        Some(dir) => dir.into(),
        None => default_data_dir()?,
    };
    let read = Orchestrator::open_client(&dir).await?;
    let (lines, _, _) = read.run_log_tail(&run_id, 0, max).await?;
    for e in lines {
        let text = e.text.unwrap_or_default().replace('\n', " ");
        let short: String = text.chars().take(260).collect();
        println!(
            "{:?} | {} | {}",
            e.kind,
            e.summary.chars().take(160).collect::<String>(),
            short
        );
    }
    Ok(())
}
