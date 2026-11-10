//! Send one daemon request, given as JSON, the same way the desktop app does.
//!
//!   cargo run -q -p agentflow-daemon --example daemon_call -- '{"command":"ping"}'
//!
//! Uses AGENTFLOW_DATA_DIR when set, otherwise the app's default data dir.
use agentflow_daemon::{DaemonRequest, DaemonResponse, default_data_dir, request};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let json = std::env::args()
        .nth(1)
        .ok_or("usage: daemon_call '<request json>'")?;
    let req: DaemonRequest = serde_json::from_str(&json)?;
    let dir = match std::env::var_os("AGENTFLOW_DATA_DIR") {
        Some(dir) => dir.into(),
        None => default_data_dir()?,
    };
    match request(&dir, &req).await? {
        DaemonResponse::Ok { payload } => println!("{}", serde_json::to_string_pretty(&payload)?),
        DaemonResponse::Error { message } => {
            eprintln!("REFUSED: {message}");
            std::process::exit(1);
        }
    }
    Ok(())
}
