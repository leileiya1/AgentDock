//! Opt-in local team execution. Shared adapters, permission broker, Git, process leases and
//! legacy approval/delivery remain the boundaries; no model is called by the dispatcher.
use super::*;
use agentflow_persistence::team::{TeamClaim, TeamClaimRequest, decode, encode};
use agentflow_workflow::team as rules;
mod control;
mod dispatch;
mod execution;
mod integration;
mod maintenance;
mod plan_preview;
mod queries;
mod recovery;
mod report_export;
#[cfg(test)]
mod tests;
mod verification;

fn err(message: impl Into<String>) -> OrchestratorError {
    OrchestratorError::InvalidState(message.into())
}
fn hash<T: Serialize>(v: &T) -> Result<String, OrchestratorError> {
    // serde_json Value uses sorted object keys (preserve_order is not enabled). Arrays retain
    // semantic order. Schema version is part of each contract/input manifest.
    let value = serde_json::to_value(v).map_err(|e| err(e.to_string()))?;
    Ok(hex_digest(
        &serde_json::to_vec(&value).map_err(|e| err(e.to_string()))?,
    ))
}
fn hex_digest(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn system_spec(
    id: &str,
    kind: TeamWorkKind,
    deps: Vec<String>,
    members: Vec<String>,
    instructions: &str,
) -> TeamWorkSpec {
    TeamWorkSpec {
        id: id.into(),
        title: id.into(),
        instructions: instructions.into(),
        kind,
        depends_on: deps,
        member_ids: members,
        requirement_ids: Vec::new(),
        write_globs: Vec::new(),
        local_checks: Vec::new(),
    }
}
async fn event(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &str,
    kind: &str,
    payload: Value,
) -> Result<(), OrchestratorError> {
    sqlx::query("INSERT INTO events(task_id,actor,event_type,payload_json,created_at) VALUES(?,'orchestrator',?,?,?)")
        .bind(task).bind(format!("team:{kind}")).bind(payload.to_string()).bind(Utc::now().to_rfc3339()).execute(&mut **tx).await?;
    Ok(())
}
async fn put_work(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &str,
    spec: &TeamWorkSpec,
) -> Result<(), OrchestratorError> {
    sqlx::query("INSERT INTO team_work_items(task_id,id,spec_json,spec_hash,state) VALUES(?,?,?,?,'pending')")
        .bind(task).bind(&spec.id).bind(encode(spec)?).bind(hash(spec)?).execute(&mut **tx).await?;
    Ok(())
}

async fn team_config_file_hash(repo: &Path) -> Result<String, OrchestratorError> {
    match tokio::fs::read(repo.join(".agentflow/project.toml")).await {
        Ok(bytes) => Ok(hex_digest(&bytes)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok("absent".into()),
        Err(error) => Err(error.into()),
    }
}
