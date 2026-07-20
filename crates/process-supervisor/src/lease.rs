use serde::{Deserialize, Serialize};
use std::{io, path::Path, process::Stdio, time::Duration};
use sysinfo::{Pid, System};
use tokio::process::Command;

/// Durable identity for a provider process. The OS start time prevents a recycled PID from
/// causing recovery to terminate an unrelated process.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessLease {
    pub pid: u32,
    pub process_group: u32,
    pub os_started_at_secs: u64,
    pub started_at: String,
    pub owner_pid: u32,
    pub program: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LeaseState {
    Alive,
    Exited,
    PidReused,
}

pub(crate) fn new_lease(pid: u32, started_at: String, program: &Path) -> ProcessLease {
    ProcessLease {
        pid,
        process_group: pid,
        os_started_at_secs: process_start_time(pid).unwrap_or(0),
        started_at,
        owner_pid: std::process::id(),
        program: program.to_string_lossy().into_owned(),
    }
}

pub(crate) async fn write_process_lease(
    path: &Path,
    lease: &ProcessLease,
) -> Result<(), io::Error> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let temporary = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(lease)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    tokio::fs::write(&temporary, bytes).await?;
    tokio::fs::rename(temporary, path).await
}

pub async fn read_process_lease(path: &Path) -> Result<ProcessLease, io::Error> {
    let bytes = tokio::fs::read(path).await?;
    serde_json::from_slice(&bytes)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

pub fn inspect_process_lease(lease: &ProcessLease) -> LeaseState {
    match process_start_time(lease.pid) {
        None => LeaseState::Exited,
        Some(started) if lease.os_started_at_secs == 0 || started == lease.os_started_at_secs => {
            LeaseState::Alive
        }
        Some(_) => LeaseState::PidReused,
    }
}

/// Terminate the recorded process group only if the PID still refers to the original process.
/// SIGKILL is sent after the grace period even when the leader exits, because grandchildren can
/// remain alive in the same process group after their parent has gone.
pub async fn terminate_process_lease(
    lease: &ProcessLease,
    grace: Duration,
) -> Result<bool, io::Error> {
    if inspect_process_lease(lease) != LeaseState::Alive || lease.process_group == 0 {
        return Ok(false);
    }
    #[cfg(unix)]
    {
        signal_group(lease.process_group, "-TERM").await?;
        tokio::time::sleep(grace).await;
        let _ = signal_group(lease.process_group, "-KILL").await;
    }
    Ok(true)
}

fn process_start_time(pid: u32) -> Option<u64> {
    let system = System::new_all();
    system
        .process(Pid::from_u32(pid))
        .map(sysinfo::Process::start_time)
}

#[cfg(unix)]
async fn signal_group(process_group: u32, signal: &str) -> Result<(), io::Error> {
    let status = Command::new("/bin/kill")
        // GNU kill interprets a negative process-group id as another option
        // unless option parsing is terminated. BSD kill accepts the same form,
        // so this keeps Linux and macOS cancellation behavior aligned.
        .args([signal, "--", &format!("-{process_group}")])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        // A process can exit between identity verification and signaling. Treat that race as an
        // already-completed termination, while preserving real command execution failures.
        if process_start_time(process_group).is_none() {
            Ok(())
        } else {
            Err(io::Error::other(format!(
                "kill {signal} process group {process_group} failed"
            )))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn current_process_lease_matches_and_wrong_marker_is_rejected() {
        let mut lease = new_lease(std::process::id(), "now".into(), Path::new("self"));
        assert_eq!(inspect_process_lease(&lease), LeaseState::Alive);
        lease.os_started_at_secs = lease.os_started_at_secs.saturating_add(1);
        assert_eq!(inspect_process_lease(&lease), LeaseState::PidReused);
    }
}
