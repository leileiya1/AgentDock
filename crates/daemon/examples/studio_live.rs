//! Live smoke test for the town customisation backend.
//!
//! Talks to a *running* daemon exactly like the desktop app does: writes go through the
//! daemon socket (`agentflow_daemon::request`), reads go straight to the database through
//! `Orchestrator::open_client`. Point it at a throwaway data dir:
//!
//!   agentflowd --data-dir "$T" serve &
//!   cargo run -q -p agentflow-daemon --example studio_live -- "$T" /path/to/git/repo

use agentflow_contracts::{AgentKind, Project};
use agentflow_daemon::{DaemonRequest, DaemonResponse, request};
use agentflow_orchestrator::Orchestrator;
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

type Res<T> = Result<T, Box<dyn std::error::Error>>;

async fn ok(dir: &Path, req: DaemonRequest) -> Res<Value> {
    match request(dir, &req).await? {
        DaemonResponse::Ok { payload } => Ok(payload),
        DaemonResponse::Error { message } => Err(format!("daemon refused: {message}").into()),
    }
}

async fn refused(dir: &Path, req: DaemonRequest) -> Res<String> {
    match request(dir, &req).await? {
        DaemonResponse::Ok { payload } => Err(format!("expected a refusal, got {payload}").into()),
        DaemonResponse::Error { message } => Ok(message),
    }
}

fn check(label: &str, cond: bool) -> Res<()> {
    println!("{} {label}", if cond { "PASS" } else { "FAIL" });
    if cond {
        Ok(())
    } else {
        Err(format!("check failed: {label}").into())
    }
}

fn de<T: serde::de::DeserializeOwned>(v: Value) -> Res<T> {
    Ok(serde_json::from_value(v)?)
}

#[tokio::main]
async fn main() -> Res<()> {
    let mut args = std::env::args().skip(1);
    let dir = PathBuf::from(
        args.next()
            .ok_or("usage: studio_live <data-dir> <git-repo>")?,
    );
    let repo = args
        .next()
        .ok_or("usage: studio_live <data-dir> <git-repo>")?;

    let ping = ok(&dir, DaemonRequest::Ping).await?;
    check(
        "daemon answers ping with ipcVersion 2",
        ping["ipcVersion"] == 2,
    )?;

    let project: Project = de(ok(&dir, DaemonRequest::ProjectImport { path: repo }).await?)?;
    let pid = project.id.clone();
    println!("     imported project {} ({})", project.name, pid);
    let read = Orchestrator::open_client(&dir).await?;

    // ---- appearance ----
    let a = ok(&dir, DaemonRequest::ProjectAppearanceUpdate {
        project_id: pid.clone(),
        patch: de(json!({ "kind": "team", "roof": "#3366CC", "sign": "neon", "yard": ["fountain", "flag"] }))?,
    }).await?;
    check(
        "appearance write returns the saved look",
        a["kind"] == "team" && a["sign"] == "neon",
    )?;
    check(
        "roof colour is normalised to lowercase",
        a["roof"] == "#3366cc",
    )?;
    let looks = read.project_appearance_list().await?;
    let look = looks
        .iter()
        .find(|l| l.project_id == pid)
        .ok_or("appearance not readable")?;
    check(
        "read path sees the appearance",
        look.yard == vec!["fountain".to_string(), "flag".to_string()],
    )?;

    let cleared = ok(
        &dir,
        DaemonRequest::ProjectAppearanceUpdate {
            project_id: pid.clone(),
            patch: de(json!({ "roof": null }))?,
        },
    )
    .await?;
    check(
        "null clears only the roof (patch keeps the rest)",
        cleared["roof"].is_null() && cleared["sign"] == "neon",
    )?;

    let msg = refused(
        &dir,
        DaemonRequest::ProjectAppearanceUpdate {
            project_id: pid.clone(),
            patch: de(json!({ "yard": ["volcano"] }))?,
        },
    )
    .await?;
    check(
        &format!("unknown yard item is refused ({msg})"),
        !msg.is_empty(),
    )?;
    let msg = refused(
        &dir,
        DaemonRequest::ProjectAppearanceUpdate {
            project_id: "no-such-project".into(),
            patch: de(json!({ "sign": "wood" }))?,
        },
    )
    .await?;
    check(
        &format!("unknown project is refused ({msg})"),
        !msg.is_empty(),
    )?;

    // ---- members ----
    let before = read.project_summaries().await?;
    let s0 = before
        .iter()
        .find(|s| s.project_id == pid)
        .ok_or("no summary")?;
    check(
        "summary starts derived with no assigned agents",
        s0.assigned_agents.is_empty(),
    )?;

    let members = ok(
        &dir,
        DaemonRequest::ProjectMembersSet {
            project_id: pid.clone(),
            members: de(json!([
                { "agent": "claude_code", "role": "developer" },
                { "agent": "codex", "role": "reviewer" },
                { "agent": "qoder_cli", "role": "both" }
            ]))?,
        },
    )
    .await?;
    check(
        "members write returns three members",
        members.as_array().map(Vec::len) == Some(3),
    )?;
    let got = read.project_members_get(&pid).await?;
    check(
        "read path keeps member order",
        got.iter().map(|m| m.agent.clone()).collect::<Vec<_>>()
            == vec![AgentKind::ClaudeCode, AgentKind::Codex, AgentKind::QoderCli],
    )?;
    let after = read.project_summaries().await?;
    let s1 = after
        .iter()
        .find(|s| s.project_id == pid)
        .ok_or("no summary")?;
    check(
        "project summary now carries the assigned crew",
        s1.assigned_agents.len() == 3 && serde_json::to_value(s1.agents_source)? == "assigned",
    )?;

    let msg = refused(&dir, DaemonRequest::ProjectMembersSet {
        project_id: pid.clone(),
        members: de(json!([{ "agent": "codex", "role": "developer" }, { "agent": "codex", "role": "reviewer" }]))?,
    }).await?;
    check(
        &format!("duplicate member is refused ({msg})"),
        !msg.is_empty(),
    )?;

    ok(
        &dir,
        DaemonRequest::ProjectMembersSet {
            project_id: pid.clone(),
            members: vec![],
        },
    )
    .await?;
    let s2 = read
        .project_summaries()
        .await?
        .into_iter()
        .find(|s| s.project_id == pid)
        .ok_or("no summary")?;
    check(
        "clearing members falls back to derived",
        s2.assigned_agents.is_empty() && serde_json::to_value(s2.agents_source)? == "derived",
    )?;

    // ---- agent profiles ----
    let p = ok(&dir, DaemonRequest::AgentProfileUpdate {
        agent: AgentKind::ClaudeCode,
        patch: de(json!({ "nickname": "小克", "color": "#E9875A", "accessory": "headphones", "catchphrase": "这个我来" }))?,
    }).await?;
    check(
        "profile write returns nickname and lowercase colour",
        p["nickname"] == "小克" && p["color"] == "#e9875a",
    )?;
    let profiles = read.agent_profiles_list().await?;
    check(
        "read path sees the profile",
        profiles.iter().any(|x| {
            x.agent == AgentKind::ClaudeCode && x.accessory.as_deref() == Some("headphones")
        }),
    )?;

    let msg = refused(
        &dir,
        DaemonRequest::AgentProfileUpdate {
            agent: AgentKind::Codex,
            patch: de(json!({ "color": "orange" }))?,
        },
    )
    .await?;
    check(&format!("bad colour is refused ({msg})"), !msg.is_empty())?;
    let msg = refused(
        &dir,
        DaemonRequest::AgentProfileUpdate {
            agent: AgentKind::Codex,
            patch: de(json!({ "nickname": "一个非常非常非常非常非常长的名字超过十六" }))?,
        },
    )
    .await?;
    check(
        &format!("over-long nickname is refused ({msg})"),
        !msg.is_empty(),
    )?;

    let reset = ok(
        &dir,
        DaemonRequest::AgentProfileReset {
            agent: AgentKind::ClaudeCode,
        },
    )
    .await?;
    check("reset reports it removed a profile", reset == json!(true))?;
    let again = ok(
        &dir,
        DaemonRequest::AgentProfileReset {
            agent: AgentKind::ClaudeCode,
        },
    )
    .await?;
    check("second reset is a harmless no-op", again == json!(false))?;
    check(
        "profile is gone from the read path",
        read.agent_profiles_list()
            .await?
            .iter()
            .all(|x| x.agent != AgentKind::ClaudeCode),
    )?;

    println!("ALL LIVE CHECKS PASSED");
    Ok(())
}
