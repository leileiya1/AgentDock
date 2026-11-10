use agentflow_agent_adapters::{
    probe_cli_runtime, probe_cli_runtime_at, probe_cli_runtime_managed, tool_status,
};
use std::{path::PathBuf, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    let Some(name) = std::env::args().nth(1) else {
        eprintln!(
            "usage: probe-provider-runtime <claude|codex|qodercli|grok> [executable] [authorized-cwd] [fresh-managed-output]"
        );
        return ExitCode::from(2);
    };
    if !matches!(name.as_str(), "claude" | "codex" | "qodercli" | "grok") {
        eprintln!("provider does not have a real runtime probe");
        return ExitCode::from(2);
    }
    let explicit_path = std::env::args().nth(2).map(PathBuf::from);
    let status = tool_status(&name, explicit_path, &[]).await;
    println!(
        "provider={name} version={} support={} authenticated={}",
        status.version.as_deref().unwrap_or("unknown"),
        status.support_level,
        status
            .authenticated
            .map_or("unknown".into(), |value| value.to_string())
    );
    if !status.found || !status.compatible || status.authenticated == Some(false) {
        eprintln!(
            "{}",
            status
                .problem
                .or(status.auth_problem)
                .unwrap_or_else(|| "provider is not ready".into())
        );
        return ExitCode::FAILURE;
    }
    let Some(path) = status.path.map(PathBuf::from) else {
        eprintln!("provider path is unavailable");
        return ExitCode::FAILURE;
    };
    let authorized_cwd = std::env::args().nth(3).map(PathBuf::from);
    let managed_dir = std::env::args().nth(4).map(PathBuf::from);
    let probe = match (authorized_cwd, managed_dir) {
        (Some(cwd), Some(dir)) => {
            if dir.exists() {
                eprintln!("managed probe output must be fresh");
                return ExitCode::FAILURE;
            }
            probe_cli_runtime_managed(
                &name,
                &path,
                &cwd,
                &dir,
                tokio_util::sync::CancellationToken::new(),
            )
            .await
        }
        (None, Some(_)) => {
            eprintln!("managed probe requires an authorized working directory");
            return ExitCode::from(2);
        }
        (Some(cwd), None) => probe_cli_runtime_at(&name, &path, &cwd).await,
        (None, None) => probe_cli_runtime(&name, &path).await,
    };
    if probe.passed {
        println!("runtime_probe=passed");
        ExitCode::SUCCESS
    } else {
        eprintln!(
            "runtime_probe=failed: {}",
            probe.problem.as_deref().unwrap_or("unknown failure")
        );
        ExitCode::FAILURE
    }
}
