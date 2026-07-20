use agentflow_contracts::{AgentKind, Project, StorageCleanupScope, TaskDetail, TaskPolicy};
use agentflow_daemon::{
    DaemonRequest, DaemonResponse, default_data_dir, request as daemon_request,
};
use agentflow_orchestrator::Orchestrator;
use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use serde::de::DeserializeOwned;
use std::path::PathBuf;
#[cfg(target_os = "macos")]
use std::process::Stdio;

#[derive(Parser)]
#[command(
    name = "agentflow-cli",
    version,
    about = "AgentFlow headless orchestrator"
)]
struct Cli {
    #[arg(long, env = "AGENTFLOW_DATA_DIR")]
    data_dir: Option<PathBuf>,
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    EnvCheck,
    Setup {
        #[command(subcommand)]
        action: SetupAction,
    },
    Storage {
        #[command(subcommand)]
        action: StorageAction,
    },
    Task {
        #[command(subcommand)]
        action: TaskAction,
    },
    RunTask {
        #[arg(long)]
        repo: PathBuf,
        #[arg(long)]
        title: String,
        #[arg(long)]
        desc: String,
        #[arg(long, value_enum, default_value = "claude-code")]
        developer: AgentArg,
        #[arg(long, value_enum, default_value = "codex")]
        reviewer: AgentArg,
        #[arg(long, default_value_t = 3)]
        max_revisions: i64,
        /// Allow task context and diffs to be sent to configured API review Providers.
        #[arg(long)]
        allow_api_egress: bool,
    },
    EventsExport {
        #[arg(long)]
        project_id: String,
    },
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },
    ApiKey {
        #[command(subcommand)]
        action: ApiKeyAction,
    },
}

#[derive(Subcommand)]
enum SetupAction {
    Check,
    Complete,
}

#[derive(Subcommand)]
enum StorageAction {
    Status,
    Cleanup,
    Task {
        #[arg(long)]
        task_id: String,
        #[arg(long, value_enum, default_value = "logs")]
        scope: StorageScopeArg,
    },
    TrashList,
    Restore {
        #[arg(long)]
        task_id: String,
    },
    EmptyTrash,
}

#[derive(Subcommand)]
enum TaskAction {
    Status {
        task_id: String,
    },
    Resume {
        task_id: String,
        #[arg(long)]
        guidance: String,
    },
    Approve {
        task_id: String,
    },
    Merge {
        task_id: String,
    },
    Cancel {
        task_id: String,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum StorageScopeArg {
    Logs,
    Runtime,
    Everything,
}

impl From<StorageScopeArg> for StorageCleanupScope {
    fn from(value: StorageScopeArg) -> Self {
        match value {
            StorageScopeArg::Logs => Self::Logs,
            StorageScopeArg::Runtime => Self::Runtime,
            StorageScopeArg::Everything => Self::Everything,
        }
    }
}

#[derive(Subcommand)]
enum DaemonAction {
    Status,
    Enqueue { task_id: String },
    Stop,
}

#[derive(Subcommand)]
enum ApiKeyAction {
    Set {
        #[arg(value_enum)]
        provider: ApiProviderArg,
    },
    Delete {
        #[arg(value_enum)]
        provider: ApiProviderArg,
    },
}

#[derive(Clone, Copy, ValueEnum)]
enum ApiProviderArg {
    OpenAi,
    Anthropic,
    DeepSeek,
    Grok,
    MiniMax,
    Kimi,
}

#[cfg(target_os = "macos")]
impl ApiProviderArg {
    fn keychain_service(self) -> &'static str {
        match self {
            Self::OpenAi => "com.agentflow.openai-api",
            Self::Anthropic => "com.agentflow.anthropic-api",
            Self::DeepSeek => "com.agentflow.deepseek-api",
            Self::Grok => "com.agentflow.grok-api",
            Self::MiniMax => "com.agentflow.minimax-api",
            Self::Kimi => "com.agentflow.kimi-api",
        }
    }
}

#[derive(Clone, Copy, ValueEnum)]
enum AgentArg {
    ClaudeCode,
    Codex,
    GeminiCli,
    QwenCode,
    OpenAiApi,
    AnthropicApi,
    DeepSeekApi,
    GrokApi,
    MiniMaxApi,
    KimiApi,
}
impl From<AgentArg> for AgentKind {
    fn from(value: AgentArg) -> Self {
        match value {
            AgentArg::ClaudeCode => Self::ClaudeCode,
            AgentArg::Codex => Self::Codex,
            AgentArg::GeminiCli => Self::GeminiCli,
            AgentArg::QwenCode => Self::QwenCode,
            AgentArg::OpenAiApi => Self::OpenAiApi,
            AgentArg::AnthropicApi => Self::AnthropicApi,
            AgentArg::DeepSeekApi => Self::DeepSeekApi,
            AgentArg::GrokApi => Self::GrokApi,
            AgentArg::MiniMaxApi => Self::MiniMaxApi,
            AgentArg::KimiApi => Self::KimiApi,
        }
    }
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let data_dir = cli
        .data_dir
        .clone()
        .map(Ok)
        .unwrap_or_else(default_data_dir)?;
    let orchestrator = Orchestrator::open_client(&data_dir)
        .await
        .context("open AgentFlow data store")?;
    match cli.command {
        Commands::EnvCheck => println!(
            "{}",
            serde_json::to_string_pretty(&orchestrator.env_check().await)?
        ),
        Commands::Setup { action } => match action {
            SetupAction::Check => {
                let daemon_running = matches!(
                    daemon_request(&data_dir, &DaemonRequest::Ping).await,
                    Ok(DaemonResponse::Ok { .. })
                );
                println!(
                    "{}",
                    serde_json::to_string_pretty(
                        &orchestrator.onboarding_check(daemon_running).await?
                    )?
                );
            }
            SetupAction::Complete => {
                daemon_payload::<serde_json::Value>(&data_dir, &DaemonRequest::OnboardingComplete)
                    .await?;
                println!("onboarding marked complete");
            }
        },
        Commands::Storage { action } => match action {
            StorageAction::Status => println!(
                "{}",
                serde_json::to_string_pretty(&orchestrator.storage_report().await?)?
            ),
            StorageAction::Cleanup => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<serde_json::Value>(
                        &data_dir,
                        &DaemonRequest::StorageCleanup,
                    )
                    .await?
                )?
            ),
            StorageAction::Task { task_id, scope } => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<serde_json::Value>(
                        &data_dir,
                        &DaemonRequest::TaskCleanup {
                            task_id,
                            scope: scope.into(),
                        },
                    )
                    .await?
                )?
            ),
            StorageAction::TrashList => println!(
                "{}",
                serde_json::to_string_pretty(&orchestrator.trash_list().await?)?
            ),
            StorageAction::Restore { task_id } => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<serde_json::Value>(
                        &data_dir,
                        &DaemonRequest::TaskRestore { task_id },
                    )
                    .await?
                )?
            ),
            StorageAction::EmptyTrash => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<serde_json::Value>(&data_dir, &DaemonRequest::TrashEmpty,)
                        .await?
                )?
            ),
        },
        Commands::Task { action } => match action {
            TaskAction::Status { task_id } => println!(
                "{}",
                serde_json::to_string_pretty(&orchestrator.task_get(&task_id).await?)?
            ),
            TaskAction::Resume { task_id, guidance } => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<TaskDetail>(
                        &data_dir,
                        &DaemonRequest::TaskResumeWithGuidance { task_id, guidance },
                    )
                    .await?
                )?
            ),
            TaskAction::Approve { task_id } => {
                let task = orchestrator.task_get(&task_id).await?;
                let revision = task.summary.current_revision;
                let diff = orchestrator.diff_get(&task_id, revision).await?;
                let approved = daemon_payload::<TaskDetail>(
                    &data_dir,
                    &DaemonRequest::TaskApprove {
                        task_id,
                        revision,
                        commit_sha: diff.commit_sha,
                        diff_sha256: diff.diff_sha256,
                    },
                )
                .await?;
                println!("{}", serde_json::to_string_pretty(&approved)?);
            }
            TaskAction::Merge { task_id } => println!(
                "{}",
                serde_json::to_string_pretty(&daemon_payload::<TaskDetail>(
                    &data_dir,
                    &DaemonRequest::TaskMerge { task_id },
                )
                .await?)?
            ),
            TaskAction::Cancel { task_id } => println!(
                "{}",
                serde_json::to_string_pretty(
                    &daemon_payload::<TaskDetail>(
                        &data_dir,
                        &DaemonRequest::TaskCancel { task_id },
                    )
                    .await?
                )?
            ),
        },
        Commands::RunTask {
            repo,
            title,
            desc,
            developer,
            reviewer,
            max_revisions,
            allow_api_egress,
        } => {
            let project = daemon_payload::<Project>(
                &data_dir,
                &DaemonRequest::ProjectImport {
                    path: repo.to_string_lossy().into_owned(),
                },
            )
            .await
            .context("import project through daemon")?;
            let task = daemon_payload::<TaskDetail>(
                &data_dir,
                &DaemonRequest::TaskCreate {
                    project_id: project.id,
                    title,
                    description: desc,
                    developer_agent: developer.into(),
                    reviewer_agent: reviewer.into(),
                    target_branch: None,
                    max_revisions: Some(max_revisions),
                    allow_api_egress,
                    policy: TaskPolicy {
                        require_plan_approval: false,
                        ..TaskPolicy::default()
                    },
                },
            )
            .await
            .context("create task through daemon")?;
            let started = daemon_payload::<TaskDetail>(
                &data_dir,
                &DaemonRequest::TaskStart {
                    task_id: task.summary.id.clone(),
                },
            )
            .await
            .context("start task through daemon")?;
            println!("{}", serde_json::to_string_pretty(&started)?);
        }
        Commands::EventsExport { project_id } => println!(
            "{}",
            orchestrator.events_export(&project_id).await?.display()
        ),
        Commands::Daemon { action } => {
            let command = match action {
                DaemonAction::Status => DaemonRequest::Ping,
                DaemonAction::Enqueue { task_id } => DaemonRequest::Enqueue { task_id },
                DaemonAction::Stop => DaemonRequest::Shutdown,
            };
            println!(
                "{}",
                serde_json::to_string_pretty(&daemon_request(&data_dir, &command).await?)?
            );
        }
        Commands::ApiKey { action } => manage_api_key(action).await?,
    }
    Ok(())
}

async fn daemon_payload<T: DeserializeOwned>(
    data_dir: &std::path::Path,
    command: &DaemonRequest,
) -> anyhow::Result<T> {
    match daemon_request(data_dir, command).await? {
        DaemonResponse::Ok { payload } => Ok(serde_json::from_value(payload)?),
        DaemonResponse::Error { message } => anyhow::bail!("daemon rejected command: {message}"),
    }
}

#[cfg(target_os = "macos")]
async fn manage_api_key(action: ApiKeyAction) -> anyhow::Result<()> {
    let (verb, provider) = match action {
        ApiKeyAction::Set { provider } => ("set", provider),
        ApiKeyAction::Delete { provider } => ("delete", provider),
    };
    let service = provider.keychain_service();
    let mut command = tokio::process::Command::new("/usr/bin/security");
    if verb == "set" {
        println!("Enter the API key at the macOS Keychain prompt:");
        command.args([
            "add-generic-password",
            "-U",
            "-a",
            "AgentFlow",
            "-s",
            service,
            "-w",
        ]);
    } else {
        command.args(["delete-generic-password", "-s", service]);
    }
    let status = command
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .await?;
    if !status.success() {
        anyhow::bail!("Keychain command failed for {service}");
    }
    println!("Keychain credential {verb}: {service}");
    Ok(())
}

#[cfg(not(target_os = "macos"))]
async fn manage_api_key(_action: ApiKeyAction) -> anyhow::Result<()> {
    anyhow::bail!(
        "API key management currently uses macOS Keychain; set the provider environment variable on this platform"
    )
}
