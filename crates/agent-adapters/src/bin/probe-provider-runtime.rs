use agentflow_agent_adapters::{probe_cli_runtime, tool_status};
use std::{path::PathBuf, process::ExitCode};

#[tokio::main]
async fn main() -> ExitCode {
    let Some(name) = std::env::args().nth(1) else {
        eprintln!("usage: probe-provider-runtime <claude|codex> [executable]");
        return ExitCode::from(2);
    };
    if !matches!(name.as_str(), "claude" | "codex") {
        eprintln!("only claude and codex currently have real runtime probes");
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
    let probe = probe_cli_runtime(&name, &path).await;
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
