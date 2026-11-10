//! Team transaction boundary. No CLI, Git or network work is performed in a transaction.
use super::*;
use agentflow_contracts::*;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamClaim {
    pub identity: TeamIdentity,
    pub spec: TeamWorkSpec,
    pub input: Value,
    pub provider: Option<AgentKind>,
    pub run_dir: PathBuf,
}

pub struct TeamClaimRequest<'a> {
    pub task_id: &'a str,
    pub work_item_id: &'a str,
    pub expected_spec_hash: &'a str,
    pub member_id: &'a str,
    pub provider: Option<AgentKind>,
    pub input: Value,
    pub input_hash: &'a str,
    pub expected_dependencies: Vec<(String, Option<String>)>,
    pub run_dir: PathBuf,
    pub global_limit: u32,
    pub provider_account: String,
    pub provider_limit: u32,
    pub provider_rpm: u32,
}
impl Store {
    pub async fn is_team_task(&self, task_id: &str) -> Result<bool, PersistenceError> {
        Ok(
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM team_tasks WHERE task_id=?")
                .bind(task_id)
                .fetch_one(&self.pool)
                .await?
                > 0,
        )
    }
    /// A write first serializes competing claimers before reading budget/slot state.
    /// Unique partial indexes remain the final guard against a second writer.
    pub async fn team_claim(
        &self,
        req: TeamClaimRequest<'_>,
    ) -> Result<Option<TeamClaim>, PersistenceError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE team_tasks SET updated_at=updated_at WHERE task_id=?")
            .bind(req.task_id)
            .execute(&mut *tx)
            .await?;
        let task =
            sqlx::query("SELECT phase,config_json,attempts_used FROM team_tasks WHERE task_id=?")
                .bind(req.task_id)
                .fetch_one(&mut *tx)
                .await?;
        let phase: String = task.get("phase");
        if !matches!(phase.as_str(), "planning" | "running") {
            return Ok(None);
        }
        let config: TeamConfig = decode(task.get("config_json"))?;
        let row = sqlx::query(
            "SELECT spec_json,spec_hash,state,epoch FROM team_work_items WHERE task_id=? AND id=?",
        )
        .bind(req.task_id)
        .bind(req.work_item_id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else { return Ok(None) };
        let spec: TeamWorkSpec = decode(row.get("spec_json"))?;
        if row.get::<String, _>("state") != "pending"
            || row.get::<String, _>("spec_hash") != req.expected_spec_hash
        {
            return Ok(None);
        }
        if config.require_plan_approval && spec.kind != TeamWorkKind::Plan {
            let approved:i64=sqlx::query_scalar("SELECT COUNT(*) FROM team_plans p JOIN team_tasks t ON t.task_id=p.task_id AND t.plan_revision=p.revision WHERE t.task_id=? AND p.approved_at IS NOT NULL").bind(req.task_id).fetch_one(&mut *tx).await?;
            if approved == 0 {
                return Ok(None);
            }
        }
        let is_program = spec.kind == TeamWorkKind::Integrate;
        if is_program != req.provider.is_none() {
            return Ok(None);
        }
        if let Some(provider) = &req.provider {
            let eligible = config
                .members
                .iter()
                .any(|m| m.id == req.member_id && m.providers.contains(provider));
            if !eligible {
                return Ok(None);
            }
        }
        for (id, artifact) in &req.expected_dependencies {
            let actual = sqlx::query(
                "SELECT state,artifact_id FROM team_work_items WHERE task_id=? AND id=?",
            )
            .bind(req.task_id)
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?;
            if actual.as_ref().is_none_or(|r| {
                r.get::<String, _>("state") != "output_accepted"
                    || r.get::<Option<String>, _>("artifact_id") != *artifact
            }) {
                return Ok(None);
            }
        }
        let running: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE status='RUNNING'")
                .fetch_one(&mut *tx)
                .await?;
        let own:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING' AND team_member_id IS NOT NULL")
            .bind(req.task_id).fetch_one(&mut *tx).await?;
        let busy:i64=sqlx::query_scalar("SELECT COUNT(*) FROM agent_runs WHERE task_id=? AND status='RUNNING' AND (team_work_item_id=? OR team_member_id=?)")
            .bind(req.task_id).bind(req.work_item_id).bind(req.member_id).fetch_one(&mut *tx).await?;
        if running >= i64::from(req.global_limit)
            || (!is_program && own >= i64::from(config.max_concurrent_attempts))
            || busy > 0
        {
            return Ok(None);
        }
        let used: i64 = task.get("attempts_used");
        let verification = matches!(
            spec.kind,
            TeamWorkKind::Coverage | TeamWorkKind::Verify | TeamWorkKind::Review
        );
        let ceiling = if verification {
            config.max_attempts
        } else {
            config.max_attempts - config.verification_reserve
        };
        if !is_program && used >= i64::from(ceiling) {
            return Ok(None);
        }
        let now = Utc::now().to_rfc3339();
        let run_id = Uuid::now_v7().to_string();
        if let Some(provider) = &req.provider {
            let recent:i64=sqlx::query_scalar("SELECT COUNT(*) FROM provider_dispatch_history WHERE provider=? AND account=? AND dispatched_at>=?")
                .bind(provider.to_string()).bind(&req.provider_account).bind((Utc::now()-chrono::Duration::seconds(60)).to_rfc3339()).fetch_one(&mut *tx).await?;
            if recent >= i64::from(req.provider_rpm) {
                return Ok(None);
            }
            let mut reserved = false;
            for slot in 0..req.provider_limit {
                let result=sqlx::query("INSERT OR IGNORE INTO provider_slots(provider,account,slot,run_id,acquired_at) VALUES(?,?,?,?,?)")
                    .bind(provider.to_string()).bind(&req.provider_account).bind(slot).bind(&run_id).bind(&now).execute(&mut *tx).await?;
                if result.rows_affected() == 1 {
                    reserved = true;
                    break;
                }
            }
            if !reserved {
                return Ok(None);
            }
            sqlx::query("INSERT INTO provider_dispatch_history(provider,account,dispatched_at) VALUES(?,?,?)")
                .bind(provider.to_string()).bind(&req.provider_account).bind(&now).execute(&mut *tx).await?;
        }
        let epoch = row.get::<i64, _>("epoch") + 1;
        let identity = TeamIdentity {
            task_id: req.task_id.into(),
            work_item_id: req.work_item_id.into(),
            attempt_id: run_id.clone(),
            member_id: req.member_id.into(),
            spec_hash: req.expected_spec_hash.into(),
            input_manifest_hash: req.input_hash.into(),
            lease_epoch: epoch as u32,
        };
        sqlx::query("INSERT INTO agent_runs(id,task_id,revision,role,agent,status,run_dir,timeout_secs,idle_timeout_secs,started_at,created_at,team_work_item_id,team_member_id,token_budget_mode,cost_budget_mode) VALUES(?,?,0,?,?,'RUNNING',?,?,?,?,?,?,?,'soft','unavailable')")
            .bind(&run_id).bind(req.task_id).bind(if is_program {"validator"}else{"developer"}).bind(req.provider.as_ref().map(ToString::to_string))
            .bind(req.run_dir.to_string_lossy().as_ref()).bind(config.attempt_timeout_secs).bind(config.attempt_timeout_secs).bind(&now).bind(&now).bind(req.work_item_id).bind((!is_program).then_some(req.member_id)).execute(&mut *tx).await?;
        sqlx::query("INSERT INTO team_attempt_contexts(run_id,identity_json,spec_json,input_json,lease_until) VALUES(?,?,?,?,?)")
            .bind(&run_id).bind(encode(&identity)?).bind(encode(&spec)?).bind(req.input.to_string()).bind((Utc::now()+chrono::Duration::seconds(60)).to_rfc3339()).execute(&mut *tx).await?;
        sqlx::query("UPDATE team_work_items SET state='active',epoch=?,detail=NULL WHERE task_id=? AND id=?")
            .bind(epoch).bind(req.task_id).bind(req.work_item_id).execute(&mut *tx).await?;
        if !is_program {
            sqlx::query(
                "UPDATE team_tasks SET attempts_used=attempts_used+1,updated_at=? WHERE task_id=?",
            )
            .bind(&now)
            .bind(req.task_id)
            .execute(&mut *tx)
            .await?;
        }
        team_event_tx(
            &mut tx,
            req.task_id,
            &run_id,
            "claimed",
            &json_value(&identity)?,
            &now,
        )
        .await?;
        tx.commit().await?;
        Ok(Some(TeamClaim {
            identity,
            spec,
            input: req.input,
            provider: req.provider,
            run_dir: req.run_dir,
        }))
    }
    pub async fn team_identity_valid(
        &self,
        identity: &TeamIdentity,
    ) -> Result<bool, PersistenceError> {
        let row=sqlx::query("SELECT w.state,w.epoch,w.spec_hash,t.phase,c.identity_json,c.cancel_requested,r.status FROM team_work_items w JOIN team_tasks t ON t.task_id=w.task_id JOIN agent_runs r ON r.id=? JOIN team_attempt_contexts c ON c.run_id=r.id WHERE w.task_id=? AND w.id=?")
            .bind(&identity.attempt_id).bind(&identity.task_id).bind(&identity.work_item_id).fetch_optional(&self.pool).await?;
        Ok(row.is_some_and(|r| {
            r.get::<String, _>("status") == "RUNNING"
                && r.get::<String, _>("state") == "active"
                && r.get::<i64, _>("epoch") == i64::from(identity.lease_epoch)
                && r.get::<String, _>("spec_hash") == identity.spec_hash
                && r.get::<i64, _>("cancel_requested") == 0
                && !matches!(
                    r.get::<String, _>("phase").as_str(),
                    "cancelled" | "delivering" | "delivered"
                )
                && decode::<TeamIdentity>(r.get("identity_json")).is_ok_and(|i| i == *identity)
        }))
    }
    /// Finishes only this attempt. An obsolete result cannot accept output or touch a new slot.
    pub async fn team_finish(
        &self,
        claim: &TeamClaim,
        success: bool,
        artifact_id: Option<&str>,
        detail: &str,
        failure_class: Option<&str>,
    ) -> Result<bool, PersistenceError> {
        let mut tx = self.pool.begin().await?;
        sqlx::query("UPDATE team_tasks SET updated_at=updated_at WHERE task_id=?")
            .bind(&claim.identity.task_id)
            .execute(&mut *tx)
            .await?;
        let row=sqlx::query("SELECT w.state,w.epoch,w.spec_hash,c.identity_json,c.cancel_requested,t.phase,r.status FROM team_work_items w JOIN team_tasks t ON t.task_id=w.task_id JOIN agent_runs r ON r.id=? JOIN team_attempt_contexts c ON c.run_id=r.id WHERE w.task_id=? AND w.id=?")
            .bind(&claim.identity.attempt_id).bind(&claim.identity.task_id).bind(&claim.identity.work_item_id).fetch_one(&mut *tx).await?;
        let valid = row.get::<String, _>("status") == "RUNNING"
            && row.get::<String, _>("state") == "active"
            && row.get::<i64, _>("epoch") == i64::from(claim.identity.lease_epoch)
            && row.get::<String, _>("spec_hash") == claim.identity.spec_hash
            && row.get::<i64, _>("cancel_requested") == 0
            && row.get::<String, _>("phase") != "cancelled"
            && decode::<TeamIdentity>(row.get("identity_json"))? == claim.identity;
        let now = Utc::now().to_rfc3339();
        let status = if !valid {
            "CANCELLED"
        } else if success {
            "SUCCEEDED"
        } else {
            "FAILED"
        };
        // Repeated completion cannot change a committed outcome.
        sqlx::query("UPDATE agent_runs SET status=?,finished_at=? WHERE id=? AND status='RUNNING'")
            .bind(status)
            .bind(&now)
            .bind(&claim.identity.attempt_id)
            .execute(&mut *tx)
            .await?;
        if valid {
            sqlx::query("UPDATE team_work_items SET state=?,artifact_id=?,detail=? WHERE task_id=? AND id=? AND epoch=?")
                .bind(if success {"output_accepted"}else{"blocked"}).bind(artifact_id).bind(detail).bind(&claim.identity.task_id).bind(&claim.identity.work_item_id).bind(claim.identity.lease_epoch).execute(&mut *tx).await?;
            sqlx::query("UPDATE team_attempt_contexts SET failure_class=?,start_intent='finished' WHERE run_id=?")
                .bind(failure_class).bind(&claim.identity.attempt_id).execute(&mut *tx).await?;
        }
        sqlx::query("DELETE FROM provider_slots WHERE run_id=?")
            .bind(&claim.identity.attempt_id)
            .execute(&mut *tx)
            .await?;
        team_event_tx(&mut tx,&claim.identity.task_id,&claim.identity.attempt_id,"finished",&serde_json::json!({"identity":claim.identity,"accepted":valid&&success,"detail":detail,"failureClass":failure_class}),&now).await?;
        tx.commit().await?;
        Ok(valid)
    }
}
pub fn decode<T: serde::de::DeserializeOwned>(text: String) -> Result<T, PersistenceError> {
    serde_json::from_str(&text).map_err(|e| PersistenceError::InvalidValue(e.to_string()))
}
pub fn encode<T: Serialize>(value: &T) -> Result<String, PersistenceError> {
    serde_json::to_string(value).map_err(|e| PersistenceError::InvalidValue(e.to_string()))
}
fn json_value<T: Serialize>(value: &T) -> Result<Value, PersistenceError> {
    serde_json::to_value(value).map_err(|e| PersistenceError::InvalidValue(e.to_string()))
}
async fn team_event_tx(
    tx: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    task: &str,
    run: &str,
    kind: &str,
    payload: &Value,
    now: &str,
) -> Result<(), PersistenceError> {
    sqlx::query("INSERT OR IGNORE INTO events(task_id,run_id,actor,event_type,payload_json,created_at,team_event_key) VALUES(?,?,'orchestrator',?,?,?,?)")
        .bind(task).bind(run).bind(format!("team:{kind}")).bind(payload.to_string()).bind(now).bind(format!("{run}:{kind}")).execute(&mut **tx).await?;
    Ok(())
}
