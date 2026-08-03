use agentflow_daemon::{DaemonRequest, default_data_dir, request, serve};
#[cfg(target_os = "macos")]
use anyhow::Context;
use anyhow::bail;
use clap::{Parser, Subcommand};
use serde::Deserialize;
use serde_json::{Value, json};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use tokio_util::sync::CancellationToken;

#[derive(Parser)]
#[command(
    name = "agentflowd",
    version,
    about = "AgentFlow long-running Rust daemon"
)]
struct Cli {
    #[arg(long, env = "AGENTFLOW_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    Serve,
    Status,
    Stop,
    InstallService,
    RollbackService,
    UninstallService,
    /// Internal structured hook used by Claude Code headless runs.
    #[command(hide = true)]
    ClaudePermissionHook {
        #[arg(long)]
        output: PathBuf,
        #[arg(long = "allow")]
        allowed: Vec<String>,
    },
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let cli = Cli::parse();
    let data_dir = cli.data_dir.unwrap_or(default_data_dir()?);
    match cli.command.unwrap_or(Command::Serve) {
        Command::Serve => {
            let shutdown = CancellationToken::new();
            let signal = shutdown.clone();
            tokio::spawn(async move {
                let _ = tokio::signal::ctrl_c().await;
                signal.cancel();
            });
            serve(data_dir, shutdown).await?;
        }
        Command::Status => {
            println!(
                "{}",
                serde_json::to_string_pretty(&request(&data_dir, &DaemonRequest::Ping).await?)?
            );
        }
        Command::Stop => {
            println!(
                "{}",
                serde_json::to_string_pretty(&request(&data_dir, &DaemonRequest::Shutdown).await?)?
            );
        }
        Command::InstallService => install_service(&data_dir).await?,
        Command::RollbackService => rollback_service(&data_dir).await?,
        Command::UninstallService => uninstall_service().await?,
        Command::ClaudePermissionHook { output, allowed } => {
            claude_permission_hook(&output, &allowed)?;
        }
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct ClaudeHookInput {
    session_id: Option<String>,
    cwd: String,
    tool_name: String,
    #[serde(default)]
    tool_input: Value,
}

fn claude_permission_hook(output: &Path, allowed: &[String]) -> anyhow::Result<()> {
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let hook: ClaudeHookInput = serde_json::from_str(&input)?;
    if hook.tool_name != "Bash" {
        println!("{{}}");
        return Ok(());
    }
    let command = hook
        .tool_input
        .get("command")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .trim();
    if allowed
        .iter()
        .any(|prefix| command_matches_allow(prefix, command))
    {
        println!("{{}}");
        return Ok(());
    }

    let sensitive = command_is_sensitive(command);
    let action_type = classify_claude_command(command, sensitive);
    let captured_command = if sensitive {
        "[REDACTED_SECRET_ARGUMENT]"
    } else {
        command
    };
    let request = json!({
        "actionType": action_type,
        "reason": hook.tool_input.get("description").and_then(Value::as_str)
            .unwrap_or("Claude Code 请求执行未预授权的 Bash 命令"),
        "operation": {
            "argv": ["Bash", "-lc", captured_command],
            "cwd": hook.cwd,
            "paths": [],
            "networkDomains": [],
            "environmentNames": [],
            "attributes": {"source": "claude_pre_tool_use"}
        },
        "resumeToken": hook.session_id
    });
    write_secure_atomic(output, &serde_json::to_vec(&request)?)?;
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": "defer"
            }
        })
    );
    Ok(())
}

fn command_matches_allow(prefix: &str, command: &str) -> bool {
    let Some(rest) = command.strip_prefix(prefix.trim()) else {
        return false;
    };
    if !rest.is_empty() && !rest.starts_with(char::is_whitespace) {
        return false;
    }
    !["&&", "||", ";", "\n", "`", "$("]
        .iter()
        .any(|operator| rest.contains(operator))
}

fn command_is_sensitive(command: &str) -> bool {
    let lower = command.to_ascii_lowercase();
    [
        "--token=",
        "--password=",
        "--secret=",
        "--api-key=",
        "authorization:",
        "private_key",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn classify_claude_command(command: &str, sensitive: bool) -> &'static str {
    if sensitive {
        return "secret_access";
    }
    let words = command.split_whitespace().collect::<Vec<_>>();
    if matches!(
        words.first().copied(),
        Some("sudo" | "security" | "launchctl" | "systemctl")
    ) {
        return "system_change";
    }
    if matches!(words.as_slice(), ["git", subcommand, ..] if matches!(*subcommand, "add" | "commit" | "push" | "pull" | "merge" | "rebase" | "reset" | "checkout" | "switch" | "clean" | "tag"))
    {
        return "git_mutation";
    }
    if matches!(words.as_slice(), [tool, subcommand, ..]
        if matches!((*tool, *subcommand),
            ("npm" | "pnpm" | "yarn" | "bun", "install" | "add")
            | ("cargo", "add" | "install")
            | ("pip" | "pip3", "install")
            | ("brew", "install")))
    {
        return "dependency_install";
    }
    "command_execute"
}

fn write_secure_atomic(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    let parent = path
        .parent()
        .ok_or_else(|| anyhow::anyhow!("permission capture path has no parent"))?;
    std::fs::create_dir_all(parent)?;
    let temporary = parent.join(format!(".claude-permission-{}.tmp", std::process::id()));
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temporary)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    std::fs::rename(temporary, path)?;
    Ok(())
}

#[cfg(target_os = "macos")]
fn user_home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .context("HOME is unavailable")
}

#[cfg(target_os = "macos")]
async fn install_service(data_dir: &Path) -> anyhow::Result<()> {
    let home = user_home()?;
    let install_dir = data_dir.join("bin");
    tokio::fs::create_dir_all(&install_dir).await?;
    let installed = install_dir.join("agentflowd");
    let staged = install_dir.join("agentflowd.new");
    let previous = install_dir.join("agentflowd.previous");
    let current = std::env::current_exe()?;
    tokio::fs::copy(&current, &staged).await?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        tokio::fs::set_permissions(&staged, std::fs::Permissions::from_mode(0o755)).await?;
    }
    // Keep exactly one known-good slot. Rename preserves the inode of a still-running daemon and
    // gives desktop startup a deterministic rollback target if the new health probe fails.
    if installed.exists() {
        if previous.exists() {
            tokio::fs::remove_file(&previous).await?;
        }
        tokio::fs::rename(&installed, &previous).await?;
    }
    tokio::fs::rename(&staged, &installed).await?;
    let logs = data_dir.join("logs");
    tokio::fs::create_dir_all(&logs).await?;
    let agents = home.join("Library/LaunchAgents");
    tokio::fs::create_dir_all(&agents).await?;
    let plist = agents.join("com.agentflow.daemon.plist");
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>com.agentflow.daemon</string>
<key>ProgramArguments</key><array><string>{}</string><string>--data-dir</string><string>{}</string><string>serve</string></array>
<key>RunAtLoad</key><true/><key>KeepAlive</key><true/>
<key>EnvironmentVariables</key><dict>
<key>PATH</key><string>{}</string>
</dict>
<key>StandardOutPath</key><string>{}</string>
<key>StandardErrorPath</key><string>{}</string>
</dict></plist>
"#,
        xml(&installed),
        xml(data_dir),
        service_path(&home),
        xml(&logs.join("agentflowd.log")),
        xml(&logs.join("agentflowd-error.log"))
    );
    tokio::fs::write(&plist, content).await?;
    let domain = format!("gui/{}", unsafe_user_id());
    let _ = tokio::process::Command::new("launchctl")
        .args(["bootout", &domain, plist.to_string_lossy().as_ref()])
        .output()
        .await;
    let output = tokio::process::Command::new("launchctl")
        .args(["bootstrap", &domain, plist.to_string_lossy().as_ref()])
        .output()
        .await?;
    if !output.status.success() {
        let _ = rollback_service(data_dir).await;
        bail!(
            "launchctl bootstrap failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!("installed {}", plist.display());
    Ok(())
}

#[cfg(target_os = "macos")]
async fn rollback_service(data_dir: &Path) -> anyhow::Result<()> {
    let install_dir = data_dir.join("bin");
    let installed = install_dir.join("agentflowd");
    let previous = install_dir.join("agentflowd.previous");
    if !previous.is_file() {
        bail!("no previous daemon slot is available");
    }
    let home = user_home()?;
    let plist = home.join("Library/LaunchAgents/com.agentflow.daemon.plist");
    let domain = format!("gui/{}", unsafe_user_id());
    let _ = tokio::process::Command::new("launchctl")
        .args(["bootout", &domain, plist.to_string_lossy().as_ref()])
        .output()
        .await;
    let failed = install_dir.join("agentflowd.failed");
    if failed.exists() {
        tokio::fs::remove_file(&failed).await?;
    }
    if installed.exists() {
        tokio::fs::rename(&installed, &failed).await?;
    }
    tokio::fs::rename(&previous, &installed).await?;
    let output = tokio::process::Command::new("launchctl")
        .args(["bootstrap", &domain, plist.to_string_lossy().as_ref()])
        .output()
        .await?;
    if !output.status.success() {
        bail!(
            "rollback launchctl bootstrap failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    println!("rolled back {}", installed.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
async fn install_service(_data_dir: &Path) -> anyhow::Result<()> {
    bail!("service installation is currently implemented for macOS launchd")
}

#[cfg(not(target_os = "macos"))]
async fn rollback_service(_data_dir: &Path) -> anyhow::Result<()> {
    bail!("service rollback is currently implemented for macOS launchd")
}

#[cfg(target_os = "macos")]
async fn uninstall_service() -> anyhow::Result<()> {
    let plist = user_home()?.join("Library/LaunchAgents/com.agentflow.daemon.plist");
    let domain = format!("gui/{}", unsafe_user_id());
    let _ = tokio::process::Command::new("launchctl")
        .args(["bootout", &domain, plist.to_string_lossy().as_ref()])
        .output()
        .await;
    if plist.exists() {
        tokio::fs::remove_file(&plist).await?;
    }
    println!("uninstalled {}", plist.display());
    Ok(())
}

#[cfg(not(target_os = "macos"))]
async fn uninstall_service() -> anyhow::Result<()> {
    bail!("service installation is currently implemented for macOS launchd")
}

#[cfg(target_os = "macos")]
fn unsafe_user_id() -> u32 {
    // `id -u` avoids introducing unsafe libc calls; failure falls back to the common console domain.
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|value| value.trim().parse().ok())
        .unwrap_or(501)
}

#[cfg(target_os = "macos")]
fn xml(path: &Path) -> String {
    path.to_string_lossy()
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(target_os = "macos")]
fn service_path(home: &Path) -> String {
    [
        home.join(".local/bin"),
        home.join(".bun/bin"),
        home.join(".cargo/bin"),
    ]
    .iter()
    .map(|path| xml(path))
    .chain([
        "/opt/homebrew/bin".into(),
        "/usr/local/bin".into(),
        "/usr/bin".into(),
        "/bin".into(),
        "/usr/sbin".into(),
        "/sbin".into(),
    ])
    .collect::<Vec<_>>()
    .join(":")
}

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::*;

    #[test]
    fn launchd_path_includes_user_cli_locations() {
        let path = service_path(Path::new("/Users/test"));
        assert!(path.starts_with("/Users/test/.local/bin:/Users/test/.bun/bin"));
        assert!(path.contains("/Users/test/.cargo/bin"));
        assert!(path.contains("/opt/homebrew/bin"));
    }
}

#[cfg(test)]
mod permission_hook_tests {
    use super::*;

    #[test]
    fn allow_rules_never_accept_compound_shell_commands() {
        assert!(command_matches_allow("git status", "git status --short"));
        assert!(!command_matches_allow(
            "git status",
            "git status && rm -rf build"
        ));
        assert!(!command_matches_allow("git status", "git statusx"));
    }

    #[test]
    fn command_classification_fails_closed_for_secrets_and_system_changes() {
        assert_eq!(
            classify_claude_command("npm install zod", false),
            "dependency_install"
        );
        assert_eq!(
            classify_claude_command("git commit -m test", false),
            "git_mutation"
        );
        assert_eq!(
            classify_claude_command("sudo launchctl list", false),
            "system_change"
        );
        assert!(command_is_sensitive(
            "curl -H 'Authorization: bearer secret' example.test"
        ));
        assert_eq!(
            classify_claude_command("curl --token=secret", true),
            "secret_access"
        );
    }
}
