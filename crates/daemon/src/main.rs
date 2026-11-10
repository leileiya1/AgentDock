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

/// `can-open` exit code for "this database was migrated past what this build knows".
const CANNOT_OPEN_EXIT: i32 = 3;

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
    /// Exit 0 if this build can open the data dir's database, 3 if the database is newer.
    /// Installers ask this before swapping binaries so they never downgrade past a migration.
    #[command(hide = true)]
    CanOpen,
    /// Internal structured hook used by Claude Code headless runs.
    #[command(hide = true)]
    ClaudePermissionHook {
        #[arg(long)]
        output: PathBuf,
        #[arg(long = "allow")]
        allowed: Vec<String>,
        /// Commands the user approved in the permission dialog; matched exactly.
        #[arg(long = "exact")]
        exact: Vec<String>,
        /// Planner / reviewer runs: deny instead of asking, they cannot wait for a human.
        #[arg(long)]
        read_only: bool,
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
        Command::UninstallService => uninstall_service(&data_dir).await?,
        Command::CanOpen => {
            let check = agentflow_persistence::schema_check(&data_dir.join("agentflow.db")).await?;
            println!("{}", serde_json::to_string(&check)?);
            if !check.can_open() {
                std::process::exit(CANNOT_OPEN_EXIT);
            }
        }
        Command::ClaudePermissionHook {
            output,
            allowed,
            exact,
            read_only,
        } => {
            claude_permission_hook(&output, &allowed, &exact, read_only)?;
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

/// What the hook tells Claude Code about one Bash call.
#[derive(Debug, PartialEq, Eq)]
enum HookVerdict {
    /// Leave it to Claude's own `--allowedTools` rules.
    PassThrough,
    /// Explicitly allow (safe read, or a command the user approved).
    Allow(&'static str),
    /// Refuse with a reason Claude can act on (read-only runs).
    Deny(&'static str),
    /// Ask AgentFlow's permission broker (developer runs only).
    Defer,
}

const READ_ONLY_DENIAL: &str = "AgentFlow：规划和审查阶段只读，不能执行这条命令。请改用 Read / Grep / Glob 工具，或只用 cat、ls、grep、git diff 这类只读命令。";

fn hook_verdict(
    command: &str,
    allowed: &[String],
    exact: &[String],
    read_only: bool,
) -> HookVerdict {
    let command = command.trim();
    if allowed
        .iter()
        .any(|prefix| command_matches_allow(prefix, command))
    {
        return HookVerdict::PassThrough;
    }
    let sensitive = command_is_sensitive(command);
    if !sensitive && exact.iter().any(|approved| approved.trim() == command) {
        return HookVerdict::Allow("AgentFlow：这条命令已经由你批准");
    }
    if !sensitive && is_safe_read(command) {
        return HookVerdict::Allow("AgentFlow：只读命令，自动放行");
    }
    if read_only {
        return HookVerdict::Deny(READ_ONLY_DENIAL);
    }
    HookVerdict::Defer
}

fn print_decision(decision: &str, reason: &str) {
    println!(
        "{}",
        json!({
            "hookSpecificOutput": {
                "hookEventName": "PreToolUse",
                "permissionDecision": decision,
                "permissionDecisionReason": reason
            }
        })
    );
}

fn claude_permission_hook(
    output: &Path,
    allowed: &[String],
    exact: &[String],
    read_only: bool,
) -> anyhow::Result<()> {
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
    match hook_verdict(command, allowed, exact, read_only) {
        HookVerdict::PassThrough => {
            println!("{{}}");
            return Ok(());
        }
        HookVerdict::Allow(reason) => {
            print_decision("allow", reason);
            return Ok(());
        }
        HookVerdict::Deny(reason) => {
            print_decision("deny", reason);
            return Ok(());
        }
        HookVerdict::Defer => {}
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

/// A shell line that only reads: every segment (split on `;`, `&&`, `||`, `|`, newlines) is a
/// known read-only program with read-only arguments, and the line has no redirection,
/// substitution, background job or env-var prefix. Anything else fails closed.
fn is_safe_read(command: &str) -> bool {
    if command.is_empty()
        || ["`", "$(", "${", "<(", ">(", ">", "<"]
            .iter()
            .any(|token| command.contains(token))
    {
        return false;
    }
    let normalized = command
        .replace("&&", "\n")
        .replace("||", "\n")
        .replace([';', '|'], "\n");
    if normalized.contains('&') {
        return false;
    }
    normalized
        .lines()
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .all(|segment| segment_is_read_only(segment) || segment_is_result_protocol(segment))
}

/// The two steps AgentFlow's own result contract asks every developer to run:
/// create `.agentflow-out/` and atomically rename the finished result into place.
fn segment_is_result_protocol(segment: &str) -> bool {
    let words: Vec<&str> = segment.split_whitespace().collect();
    matches!(
        words.as_slice(),
        ["mkdir", "-p", ".agentflow-out" | ".agentflow-out/"]
            | [
                "mv",
                ".agentflow-out/result.tmp.json",
                ".agentflow-out/result.json"
            ]
            | [
                "mv",
                "-f",
                ".agentflow-out/result.tmp.json",
                ".agentflow-out/result.json"
            ]
    )
}

fn segment_is_read_only(segment: &str) -> bool {
    let words: Vec<&str> = segment.split_whitespace().collect();
    let Some((program, args)) = words.split_first() else {
        return true;
    };
    match *program {
        "cat" | "ls" | "head" | "tail" | "wc" | "grep" | "rg" | "file" | "stat" | "pwd"
        | "echo" | "tree" | "sort" | "uniq" | "cut" | "diff" | "basename" | "dirname"
        | "realpath" | "nl" | "cd" | "true" | "which" => true,
        "find" => !args.iter().any(|arg| {
            matches!(
                *arg,
                "-exec"
                    | "-execdir"
                    | "-delete"
                    | "-ok"
                    | "-okdir"
                    | "-fprint"
                    | "-fprint0"
                    | "-fprintf"
                    | "-fls"
            )
        }),
        "sed" => args.first() == Some(&"-n") && !args.iter().any(|arg| arg.starts_with("-i")),
        "git" => match args.first().copied() {
            Some(
                "status" | "diff" | "log" | "show" | "ls-files" | "rev-parse" | "blame" | "grep"
                | "cat-file" | "shortlog",
            ) => true,
            Some("branch") => args[1..].iter().all(|arg| {
                matches!(
                    *arg,
                    "--show-current" | "-a" | "-l" | "--list" | "-v" | "-vv"
                )
            }),
            _ => false,
        },
        _ => false,
    }
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
    let label = service_label(data_dir)?;
    let plist = agents.join(format!("{label}.plist"));
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict>
<key>Label</key><string>{label}</string>
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
    let plist = home.join(format!(
        "Library/LaunchAgents/{}.plist",
        service_label(data_dir)?
    ));
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
async fn uninstall_service(data_dir: &Path) -> anyhow::Result<()> {
    let plist = user_home()?.join(format!(
        "Library/LaunchAgents/{}.plist",
        service_label(data_dir)?
    ));
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
async fn uninstall_service(_data_dir: &Path) -> anyhow::Result<()> {
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
    fn safe_reads_are_recognised_and_writes_fail_closed() {
        // What Claude actually ran first in the 2026-11-07 live test, per role.
        assert!(is_safe_read(
            "cat .agentflow-in/plan-input.md; ls -la .agentflow-in"
        ));
        assert!(is_safe_read(
            "cat .agentflow-in/input.md; ls -la .agentflow-in; git ls-files"
        ));
        assert!(is_safe_read(
            "cat .agentflow-in/review-input.md; ls -la .agentflow-in; git show --stat HEAD"
        ));
        assert!(is_safe_read("grep -rn word_count textkit | head -20"));
        assert!(is_safe_read("sed -n '1,40p' textkit/__init__.py"));
        assert!(is_safe_read("find . -name '*.py'"));
        assert!(is_safe_read("git branch --show-current"));
        // AgentFlow's own result protocol, as Claude runs it after writing the temp file.
        assert!(is_safe_read(
            "cd \"/Users/x/Library/Application Support/com.agentflow.desktop/wt/p1/t5\" && mkdir -p .agentflow-out && mv .agentflow-out/result.tmp.json .agentflow-out/result.json"
        ));
        assert!(!is_safe_read(
            "mv .agentflow-out/result.tmp.json src/main.rs"
        ));
        assert!(!is_safe_read("mkdir -p src/evil"));
        // Writing the result through a heredoc is still a shell write and must ask.
        assert!(!is_safe_read(
            "mkdir -p .agentflow-out && cat > .agentflow-out/result.tmp.json <<'EOF'"
        ));

        for unsafe_line in [
            "cat a > b",
            "echo hi >> notes.md",
            "ls && rm -rf build",
            "cat $(which python3)",
            "cat `ls`",
            "find . -name '*.pyc' -delete",
            "find . -exec rm {} ;",
            "sed -i 's/a/b/' file",
            "git commit -m x",
            "git branch -D main",
            "python3 -m unittest",
            "curl https://example.test",
            "FOO=1 ls",
            "sleep 100 &",
            "",
        ] {
            assert!(
                !is_safe_read(unsafe_line),
                "should not auto-allow: {unsafe_line}"
            );
        }
    }

    #[test]
    fn verdicts_per_role_and_approval() {
        let allow = vec!["git status".to_string()];
        let none: Vec<String> = Vec::new();
        assert_eq!(
            hook_verdict("git status", &allow, &none, true),
            HookVerdict::PassThrough
        );
        assert!(matches!(
            hook_verdict("cat README.md", &allow, &none, true),
            HookVerdict::Allow(_)
        ));
        // Read-only roles cannot wait for a human: refuse with guidance instead of failing.
        assert!(matches!(
            hook_verdict("python3 -m unittest", &allow, &none, true),
            HookVerdict::Deny(_)
        ));
        // Developers ask the broker...
        assert_eq!(
            hook_verdict("python3 -m unittest", &allow, &none, false),
            HookVerdict::Defer
        );
        // ...and once approved, the exact same command goes through on the resumed run.
        let approved = vec!["python3 -m unittest".to_string()];
        assert!(matches!(
            hook_verdict("python3 -m unittest", &allow, &approved, false),
            HookVerdict::Allow(_)
        ));
        assert_eq!(
            hook_verdict("python3 -m unittest -v", &allow, &approved, false),
            HookVerdict::Defer
        );
        // Secrets are never auto-allowed, even when approved text matches.
        let secret = vec!["curl --token=abc x".to_string()];
        assert_eq!(
            hook_verdict("curl --token=abc x", &allow, &secret, false),
            HookVerdict::Defer
        );
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

/// The app's own data dir keeps the historical LaunchAgent label.
const DEFAULT_SERVICE_LABEL: &str = "com.agentflow.daemon";

/// LaunchAgent label for a data dir. Every non-default data dir (a debug sandbox via
/// `AGENTFLOW_DATA_DIR`, a QA profile) gets its own label, so installing a daemon there can
/// never rewrite or stop the user's real service.
fn service_label(data_dir: &Path) -> anyhow::Result<String> {
    Ok(label_for(data_dir, &default_data_dir()?))
}

fn label_for(data_dir: &Path, default_dir: &Path) -> String {
    let normal = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.components().collect());
    if normal(data_dir) == normal(default_dir) {
        return DEFAULT_SERVICE_LABEL.to_string();
    }
    // FNV-1a: stable across runs and builds (std's hasher is randomly seeded).
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in normal(data_dir).to_string_lossy().bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("{DEFAULT_SERVICE_LABEL}.{hash:016x}")
}

#[cfg(test)]
mod service_label_tests {
    use super::{DEFAULT_SERVICE_LABEL, label_for};
    use std::path::Path;

    #[test]
    fn default_dir_keeps_the_historical_label() {
        let home = Path::new("/Users/x/Library/Application Support/com.agentflow.desktop");
        assert_eq!(label_for(home, home), DEFAULT_SERVICE_LABEL);
        let trailing = Path::new("/Users/x/Library/Application Support/com.agentflow.desktop/");
        assert_eq!(label_for(trailing, home), DEFAULT_SERVICE_LABEL);
    }

    #[test]
    fn other_dirs_get_their_own_stable_label() {
        let home = Path::new("/Users/x/Library/Application Support/com.agentflow.desktop");
        let sandbox = Path::new("/Users/x/project/.live");
        let a = label_for(sandbox, home);
        assert_ne!(a, DEFAULT_SERVICE_LABEL);
        assert!(a.starts_with("com.agentflow.daemon."));
        assert_eq!(a, label_for(sandbox, home), "same dir, same label");
        assert_ne!(a, label_for(Path::new("/Users/x/project/.qa"), home));
    }
}
