use chrono::Utc;
use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use codex_arg0::arg0_dispatch_or_else;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_chatgpt::apply_command::run_apply_command;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cli::WindowsCommand;
use codex_cli::login::read_api_key_from_stdin;
use codex_cli::login::run_login_status;
use codex_cli::login::run_login_with_api_key;
use codex_cli::login::run_login_with_chatgpt;
use codex_cli::login::run_login_with_device_code;
use codex_cli::login::run_logout;
use codex_cloud_tasks::Cli as CloudTasksCli;
use codex_common::CliConfigOverrides;
use codex_exec::Cli as ExecCli;
use codex_exec::Command as ExecCommand;
use codex_exec::ReviewArgs;
use codex_execpolicy::ExecPolicyCheckCommand;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;
use codex_tui::AppExitInfo;
use codex_tui::Cli as TuiCli;
use codex_tui::update_action::UpdateAction;
use codex_tui2 as tui2;
use owo_colors::OwoColorize;
use sha2::Digest;
use sha2::Sha256;
use std::path::PathBuf;
use supports_color::Stream;

mod mcp_cmd;
#[cfg(not(windows))]
mod wsl_paths;

use crate::mcp_cmd::McpCli;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::types::GraphitiGroupIdStrategy;
use codex_core::config::types::GraphitiRecallScopesMode;
use codex_core::features::Feature;
use codex_core::features::FeatureOverrides;
use codex_core::features::Features;
use codex_core::features::is_known_feature_key;
use codex_core::git_info::get_git_repo_root;
use codex_core::graphiti::client::AddMessagesRequest;
use codex_core::graphiti::client::GraphitiClient;
use codex_core::graphiti::client::GraphitiMessage;
use codex_core::graphiti::client::GraphitiRoleType;

/// Codex CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true,
    // The executable is sometimes invoked via a platform‑specific name like
    // `codex-x86_64-unknown-linux-musl`, but the help output should always use
    // the generic `codex` command name that users run.
    bin_name = "codex",
    override_usage = "codex [OPTIONS] [PROMPT]\n       codex [OPTIONS] <COMMAND> [ARGS]"
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    pub feature_toggles: FeatureToggles,

    #[clap(flatten)]
    interactive: TuiCli,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Run a code review non-interactively.
    Review(ReviewArgs),

    /// Manage login.
    Login(LoginCommand),

    /// Remove stored authentication credentials.
    Logout(LogoutCommand),

    /// [experimental] Run Codex as an MCP server and manage MCP servers.
    Mcp(McpCli),

    /// [experimental] Run the Codex MCP server (stdio transport).
    McpServer,

    /// [experimental] Run the app server or related tooling.
    AppServer(AppServerCommand),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Run commands within a Codex-provided sandbox.
    #[clap(visible_alias = "debug")]
    Sandbox(SandboxArgs),

    /// Execpolicy tooling.
    #[clap(hide = true)]
    Execpolicy(ExecpolicyCommand),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Resume a previous interactive session (picker by default; use --last to continue the most recent).
    Resume(ResumeCommand),

    /// [EXPERIMENTAL] Browse tasks from Codex Cloud and apply changes locally.
    #[clap(name = "cloud", alias = "cloud-tasks")]
    Cloud(CloudTasksCli),

    /// Internal: run the responses API proxy.
    #[clap(hide = true)]
    ResponsesApiProxy(ResponsesApiProxyArgs),

    /// Internal: relay stdio to a Unix domain socket.
    #[clap(hide = true, name = "stdio-to-uds")]
    StdioToUds(StdioToUdsCommand),

    /// Inspect feature flags.
    Features(FeaturesCli),

    /// Graphiti memory integration tooling.
    Graphiti(GraphitiCli),
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct ResumeCommand {
    /// Conversation/session id (UUID). When provided, resumes this session.
    /// If omitted, use --last to pick the most recent recorded session.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,

    /// Continue the most recent session without showing the picker.
    #[arg(long = "last", default_value_t = false, conflicts_with = "session_id")]
    last: bool,

    /// Show all sessions (disables cwd filtering and shows CWD column).
    #[arg(long = "all", default_value_t = false)]
    all: bool,

    #[clap(flatten)]
    config_overrides: TuiCli,
}

#[derive(Debug, Parser)]
struct SandboxArgs {
    #[command(subcommand)]
    cmd: SandboxCommand,
}

#[derive(Debug, clap::Subcommand)]
enum SandboxCommand {
    /// Run a command under Seatbelt (macOS only).
    #[clap(visible_alias = "seatbelt")]
    Macos(SeatbeltCommand),

    /// Run a command under Landlock+seccomp (Linux only).
    #[clap(visible_alias = "landlock")]
    Linux(LandlockCommand),

    /// Run a command under Windows restricted token (Windows only).
    Windows(WindowsCommand),
}

#[derive(Debug, Parser)]
struct ExecpolicyCommand {
    #[command(subcommand)]
    sub: ExecpolicySubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum ExecpolicySubcommand {
    /// Check execpolicy files against a command.
    #[clap(name = "check")]
    Check(ExecPolicyCheckCommand),
}

#[derive(Debug, Parser)]
struct LoginCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    #[arg(
        long = "with-api-key",
        help = "Read the API key from stdin (e.g. `printenv OPENAI_API_KEY | codex login --with-api-key`)"
    )]
    with_api_key: bool,

    #[arg(
        long = "api-key",
        value_name = "API_KEY",
        help = "(deprecated) Previously accepted the API key directly; now exits with guidance to use --with-api-key",
        hide = true
    )]
    api_key: Option<String>,

    #[arg(long = "device-auth")]
    use_device_code: bool,

    /// EXPERIMENTAL: Use custom OAuth issuer base URL (advanced)
    /// Override the OAuth issuer base URL (advanced)
    #[arg(long = "experimental_issuer", value_name = "URL", hide = true)]
    issuer_base_url: Option<String>,

    /// EXPERIMENTAL: Use custom OAuth client ID (advanced)
    #[arg(long = "experimental_client-id", value_name = "CLIENT_ID", hide = true)]
    client_id: Option<String>,

    #[command(subcommand)]
    action: Option<LoginSubcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum LoginSubcommand {
    /// Show login status.
    Status,
}

#[derive(Debug, Parser)]
struct LogoutCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,
}

#[derive(Debug, Parser)]
struct AppServerCommand {
    /// Omit to run the app server; specify a subcommand for tooling.
    #[command(subcommand)]
    subcommand: Option<AppServerSubcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum AppServerSubcommand {
    /// [experimental] Generate TypeScript bindings for the app server protocol.
    GenerateTs(GenerateTsCommand),

    /// [experimental] Generate JSON Schema for the app server protocol.
    GenerateJsonSchema(GenerateJsonSchemaCommand),
}

#[derive(Debug, Args)]
struct GenerateTsCommand {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct GenerateJsonSchemaCommand {
    /// Output directory where the schema bundle will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,
}

#[derive(Debug, Parser)]
struct StdioToUdsCommand {
    /// Path to the Unix domain socket to connect to.
    #[arg(value_name = "SOCKET_PATH")]
    socket_path: PathBuf,
}

fn format_exit_messages(exit_info: AppExitInfo, color_enabled: bool) -> Vec<String> {
    let AppExitInfo {
        token_usage,
        conversation_id,
        ..
    } = exit_info;

    if token_usage.is_zero() {
        return Vec::new();
    }

    let mut lines = vec![format!(
        "{}",
        codex_core::protocol::FinalOutput::from(token_usage)
    )];

    if let Some(session_id) = conversation_id {
        let resume_cmd = format!("codex resume {session_id}");
        let command = if color_enabled {
            resume_cmd.cyan().to_string()
        } else {
            resume_cmd
        };
        lines.push(format!("To continue this session, run {command}"));
    }

    lines
}

/// Handle the app exit and print the results. Optionally run the update action.
fn handle_app_exit(exit_info: AppExitInfo) -> anyhow::Result<()> {
    let update_action = exit_info.update_action;
    let color_enabled = supports_color::on(Stream::Stdout).is_some();
    for line in format_exit_messages(exit_info, color_enabled) {
        println!("{line}");
    }
    if let Some(action) = update_action {
        run_update_action(action)?;
    }
    Ok(())
}

/// Run the update action and print the result.
fn run_update_action(action: UpdateAction) -> anyhow::Result<()> {
    println!();
    let cmd_str = action.command_str();
    println!("Updating Codex via `{cmd_str}`...");

    let status = {
        #[cfg(windows)]
        {
            // On Windows, run via cmd.exe so .CMD/.BAT are correctly resolved (PATHEXT semantics).
            std::process::Command::new("cmd")
                .args(["/C", &cmd_str])
                .status()?
        }
        #[cfg(not(windows))]
        {
            let (cmd, args) = action.command_args();
            let command_path = crate::wsl_paths::normalize_for_wsl(cmd);
            let normalized_args: Vec<String> = args
                .iter()
                .map(crate::wsl_paths::normalize_for_wsl)
                .collect();
            std::process::Command::new(&command_path)
                .args(&normalized_args)
                .status()?
        }
    };
    if !status.success() {
        anyhow::bail!("`{cmd_str}` failed with status {status}");
    }
    println!();
    println!("🎉 Update ran successfully! Please restart Codex.");
    Ok(())
}

fn run_execpolicycheck(cmd: ExecPolicyCheckCommand) -> anyhow::Result<()> {
    cmd.run()
}

#[derive(Debug, Default, Parser, Clone)]
struct FeatureToggles {
    /// Enable a feature (repeatable). Equivalent to `-c features.<name>=true`.
    #[arg(long = "enable", value_name = "FEATURE", action = clap::ArgAction::Append, global = true)]
    enable: Vec<String>,

    /// Disable a feature (repeatable). Equivalent to `-c features.<name>=false`.
    #[arg(long = "disable", value_name = "FEATURE", action = clap::ArgAction::Append, global = true)]
    disable: Vec<String>,
}

impl FeatureToggles {
    fn to_overrides(&self) -> anyhow::Result<Vec<String>> {
        let mut v = Vec::new();
        for feature in &self.enable {
            Self::validate_feature(feature)?;
            v.push(format!("features.{feature}=true"));
        }
        for feature in &self.disable {
            Self::validate_feature(feature)?;
            v.push(format!("features.{feature}=false"));
        }
        Ok(v)
    }

    fn validate_feature(feature: &str) -> anyhow::Result<()> {
        if is_known_feature_key(feature) {
            Ok(())
        } else {
            anyhow::bail!("Unknown feature flag: {feature}")
        }
    }
}

#[derive(Debug, Parser)]
struct FeaturesCli {
    #[command(subcommand)]
    sub: FeaturesSubcommand,
}

#[derive(Debug, Parser)]
enum FeaturesSubcommand {
    /// List known features with their stage and effective state.
    List,
}

#[derive(Debug, Parser)]
struct GraphitiCli {
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    sub: GraphitiSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum GraphitiSubcommand {
    /// Test connectivity to the configured Graphiti endpoint.
    #[clap(name = "test-connection")]
    TestConnection(GraphitiTestConnectionArgs),

    /// Print Graphiti config and derived group ids for the current cwd.
    Status(GraphitiStatusArgs),

    /// Create a durable memory episode (promotion).
    Promote(GraphitiPromoteArgs),

    /// Delete a Graphiti group by id.
    Purge(GraphitiPurgeArgs),
}

#[derive(Debug, Args)]
struct GraphitiTestConnectionArgs {
    /// Override Graphiti endpoint (otherwise uses config `[graphiti].endpoint`).
    #[arg(long = "endpoint")]
    endpoint: Option<String>,

    /// Request timeout (ms).
    #[arg(long = "timeout-ms", default_value_t = 1500)]
    timeout_ms: u64,

    /// Allow network calls in untrusted projects (use with care).
    #[arg(long = "allow-untrusted", default_value_t = false)]
    allow_untrusted: bool,

    /// Run a smoke test (writes a temporary group, polls episodes, then deletes it).
    #[arg(long = "smoke", default_value_t = false)]
    smoke: bool,
}

#[derive(Debug, Args)]
struct GraphitiStatusArgs {
    /// Also perform a `GET /healthcheck` request.
    #[arg(long = "healthcheck", default_value_t = false)]
    healthcheck: bool,
}

#[derive(Debug, Clone, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum GraphitiPromotionScope {
    Workspace,
    Global,
}

#[derive(Debug, Clone, clap::ValueEnum)]
#[clap(rename_all = "kebab-case")]
enum GraphitiEpisodeKind {
    Decision,
    LessonLearned,
    Preference,
    Procedure,
    TaskUpdate,
    Terminology,
}

#[derive(Debug, Args)]
struct GraphitiPromoteArgs {
    /// Override Graphiti endpoint (otherwise uses config `[graphiti].endpoint`).
    #[arg(long = "endpoint")]
    endpoint: Option<String>,

    #[arg(long = "scope", value_enum, default_value_t = GraphitiPromotionScope::Workspace)]
    scope: GraphitiPromotionScope,

    #[arg(long = "kind", value_enum)]
    kind: GraphitiEpisodeKind,

    #[arg(long = "title")]
    title: Option<String>,

    /// Episode content. If omitted, use `--stdin`.
    #[arg(long = "text")]
    text: Option<String>,

    /// Read episode content from stdin.
    #[arg(long = "stdin", default_value_t = false)]
    stdin: bool,
}

#[derive(Debug, Args)]
struct GraphitiPurgeArgs {
    /// Override Graphiti endpoint (otherwise uses config `[graphiti].endpoint`).
    #[arg(long = "endpoint")]
    endpoint: Option<String>,

    /// Allow network calls in untrusted projects (use with care).
    #[arg(long = "allow-untrusted", default_value_t = false)]
    allow_untrusted: bool,

    group_id: String,
}

fn stage_str(stage: codex_core::features::Stage) -> &'static str {
    use codex_core::features::Stage;
    match stage {
        Stage::Experimental => "experimental",
        Stage::Beta => "beta",
        Stage::Stable => "stable",
        Stage::Deprecated => "deprecated",
        Stage::Removed => "removed",
    }
}

async fn run_graphiti_cli(cli: GraphitiCli, config_profile: Option<String>) -> anyhow::Result<()> {
    let cli_kv_overrides = cli
        .config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let overrides = ConfigOverrides {
        config_profile,
        ..Default::default()
    };
    let config = Config::load_with_cli_overrides(cli_kv_overrides, overrides).await?;

    match cli.sub {
        GraphitiSubcommand::Status(args) => {
            print_graphiti_status(&config, args.healthcheck).await?;
        }
        GraphitiSubcommand::TestConnection(args) => {
            ensure_graphiti_cli_allowed(&config, args.allow_untrusted)?;
            let endpoint = resolve_graphiti_endpoint(&config, args.endpoint.as_deref())?;
            let client = build_graphiti_client(&config, &endpoint)?;
            let timeout = std::time::Duration::from_millis(args.timeout_ms);

            let health = client.healthcheck(timeout).await?;
            println!("Graphiti healthcheck: {}", health.status);

            if args.smoke {
                run_graphiti_smoke_test(&config, &client, timeout).await?;
            }
        }
        GraphitiSubcommand::Promote(args) => {
            ensure_graphiti_cli_allowed(&config, false)?;
            ensure_graphiti_enabled_and_consented(&config)?;

            let endpoint = resolve_graphiti_endpoint(&config, args.endpoint.as_deref())?;
            let client = build_graphiti_client(&config, &endpoint)?;

            let text = match (args.text, args.stdin) {
                (Some(text), _) => text,
                (None, true) => read_stdin_to_string()?,
                (None, false) => anyhow::bail!("Missing --text (or pass --stdin)"),
            };

            let (group_id, scope_label) = match args.scope {
                GraphitiPromotionScope::Workspace => {
                    (derive_workspace_group_id(&config), "workspace".to_string())
                }
                GraphitiPromotionScope::Global => {
                    if !config.graphiti.global.enabled {
                        anyhow::bail!(
                            "Global scope is disabled (set [graphiti.global].enabled = true)"
                        );
                    }
                    (derive_global_group_id(&config), "global".to_string())
                }
            };

            let template = build_promotion_template(args.kind, args.title.as_deref(), &text);
            let request = AddMessagesRequest {
                group_id: group_id.clone(),
                messages: vec![GraphitiMessage {
                    content: template,
                    uuid: None,
                    name: String::new(),
                    role_type: GraphitiRoleType::System,
                    role: None,
                    timestamp: Utc::now(),
                    source_description: format!("codex promotion scope={scope_label}"),
                }],
            };

            client
                .add_messages(
                    request,
                    std::time::Duration::from_millis(config.graphiti.ingest.timeout_ms),
                    config.graphiti.ingest.max_batch_size,
                )
                .await?;

            println!("Promoted episode to group_id={group_id}");
        }
        GraphitiSubcommand::Purge(args) => {
            ensure_graphiti_cli_allowed(&config, args.allow_untrusted)?;
            let endpoint = resolve_graphiti_endpoint(&config, args.endpoint.as_deref())?;
            let client = build_graphiti_client(&config, &endpoint)?;
            let timeout = std::time::Duration::from_millis(config.graphiti.ingest.timeout_ms);
            client.delete_group(&args.group_id, timeout).await?;
            println!("Deleted group_id={}", args.group_id);
        }
    }

    Ok(())
}

fn ensure_graphiti_cli_allowed(config: &Config, allow_untrusted: bool) -> anyhow::Result<()> {
    if config.active_project.is_trusted() || allow_untrusted {
        return Ok(());
    }
    anyhow::bail!(
        "Project is not trusted; set `[projects.\"{}\"].trust_level = \"trusted\"` in config.toml or pass --allow-untrusted",
        config.cwd.display()
    )
}

fn ensure_graphiti_enabled_and_consented(config: &Config) -> anyhow::Result<()> {
    if !config.graphiti.enabled {
        anyhow::bail!("Graphiti is disabled (set [graphiti].enabled = true)");
    }
    if !config.graphiti.consent {
        anyhow::bail!("Graphiti consent not granted (set [graphiti].consent = true)");
    }
    Ok(())
}

fn resolve_graphiti_endpoint<'a>(
    config: &'a Config,
    override_endpoint: Option<&'a str>,
) -> anyhow::Result<String> {
    if let Some(endpoint) = override_endpoint {
        return Ok(endpoint.to_string());
    }
    config
        .graphiti
        .endpoint
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Missing [graphiti].endpoint (or pass --endpoint)"))
}

fn build_graphiti_client(config: &Config, endpoint: &str) -> anyhow::Result<GraphitiClient> {
    let bearer_token = config
        .graphiti
        .bearer_token_env_var
        .as_deref()
        .and_then(|key| std::env::var(key).ok());
    Ok(GraphitiClient::from_base_url_str(endpoint, bearer_token)?)
}

async fn run_graphiti_smoke_test(
    config: &Config,
    client: &GraphitiClient,
    timeout: std::time::Duration,
) -> anyhow::Result<()> {
    use std::time::SystemTime;
    use std::time::UNIX_EPOCH;

    let now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let group_id = format!("codex-smoke-{now_ms}");
    let marker = format!("codex-graphiti-smoke-{now_ms}");

    client
        .add_messages(
            AddMessagesRequest {
                group_id: group_id.clone(),
                messages: vec![GraphitiMessage {
                    content: marker.clone(),
                    uuid: None,
                    name: String::new(),
                    role_type: GraphitiRoleType::User,
                    role: None,
                    timestamp: Utc::now(),
                    source_description: "codex graphiti smoke".to_string(),
                }],
            },
            timeout,
            config.graphiti.ingest.max_batch_size,
        )
        .await?;

    let mut saw_episode = false;
    for _ in 0..20 {
        let episodes = client.get_episodes(&group_id, 5, timeout).await?;
        if episodes.as_array().is_some_and(|arr| !arr.is_empty()) {
            saw_episode = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }

    if saw_episode {
        println!("Smoke test: episodes observed for group_id={group_id}");
    } else {
        println!(
            "Smoke test: no episodes observed yet for group_id={group_id} (Graphiti may be slow to process)"
        );
    }

    let _ = client.delete_group(&group_id, timeout).await;
    Ok(())
}

async fn print_graphiti_status(config: &Config, healthcheck: bool) -> anyhow::Result<()> {
    println!("cwd: {}", config.cwd.display());
    println!("trusted: {}", config.active_project.is_trusted());

    println!("graphiti.enabled: {}", config.graphiti.enabled);
    println!("graphiti.consent: {}", config.graphiti.consent);
    println!(
        "graphiti.endpoint: {}",
        config.graphiti.endpoint.as_deref().unwrap_or("<unset>")
    );
    println!(
        "graphiti.group_id_strategy: {}",
        match config.graphiti.group_id_strategy {
            GraphitiGroupIdStrategy::Raw => "raw",
            GraphitiGroupIdStrategy::Hashed => "hashed",
        }
    );
    println!(
        "graphiti.ingest_scopes: {}",
        config
            .graphiti
            .ingest_scopes
            .iter()
            .map(|scope| format!("{scope:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "graphiti.recall.enabled: {}",
        config.graphiti.recall.enabled
    );
    println!(
        "graphiti.recall.scopes_mode: {}",
        match config.graphiti.recall.scopes_mode {
            GraphitiRecallScopesMode::Static => "static",
            GraphitiRecallScopesMode::Auto => "auto",
        }
    );
    println!(
        "graphiti.recall.scopes: {}",
        config
            .graphiti
            .recall
            .scopes
            .iter()
            .map(|scope| format!("{scope:?}"))
            .collect::<Vec<_>>()
            .join(", ")
    );
    println!(
        "graphiti.global.enabled: {}",
        config.graphiti.global.enabled
    );
    if config.graphiti.global.enabled {
        println!(
            "graphiti.global.group_id: {}",
            config.graphiti.global.group_id
        );
        println!(
            "derived.global_group_id: {}",
            derive_global_group_id(config)
        );
    }

    println!(
        "derived.workspace_group_id: {}",
        derive_workspace_group_id(config)
    );

    println!(
        "graphiti.include_system_messages: {}",
        config.graphiti.include_system_messages
    );
    println!(
        "graphiti.user_scope_key: {}",
        if config.graphiti.user_scope_key.is_some() {
            "<set>"
        } else {
            "<unset>"
        }
    );
    println!(
        "graphiti.auto_promote.enabled: {}",
        config.graphiti.auto_promote.enabled
    );

    if healthcheck {
        ensure_graphiti_cli_allowed(config, false)?;
        let endpoint = resolve_graphiti_endpoint(config, None)?;
        let client = build_graphiti_client(config, &endpoint)?;
        let health = client
            .healthcheck(std::time::Duration::from_millis(1500))
            .await?;
        println!("graphiti.healthcheck: {}", health.status);
    }

    Ok(())
}

fn derive_workspace_group_id(config: &Config) -> String {
    let workspace_key = get_git_repo_root(&config.cwd)
        .unwrap_or_else(|| config.cwd.clone())
        .to_string_lossy()
        .to_string();
    graphiti_make_group_id(
        "codex-workspace",
        &workspace_key,
        &config.graphiti.group_id_strategy,
    )
}

fn derive_global_group_id(config: &Config) -> String {
    if let Some(user_scope_key) = config.graphiti.user_scope_key.as_deref() {
        return graphiti_make_group_id(
            "codex-global",
            user_scope_key,
            &config.graphiti.group_id_strategy,
        );
    }

    config.graphiti.global.group_id.clone()
}

fn graphiti_make_group_id(
    prefix: &str,
    raw_key: &str,
    strategy: &GraphitiGroupIdStrategy,
) -> String {
    match strategy {
        GraphitiGroupIdStrategy::Hashed => {
            let mut hasher = Sha256::new();
            hasher.update(raw_key.as_bytes());
            let digest = hasher.finalize();
            let hex = graphiti_hex_encode(digest.as_slice());
            format!("{prefix}-{}", &hex[..16])
        }
        GraphitiGroupIdStrategy::Raw => {
            let safe = raw_key
                .chars()
                .map(|ch| {
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                        ch
                    } else {
                        '_'
                    }
                })
                .collect::<String>();
            let mut candidate = format!("{prefix}-{safe}");
            if candidate.len() <= 120 {
                return candidate;
            }

            let mut hasher = Sha256::new();
            hasher.update(raw_key.as_bytes());
            let digest = hasher.finalize();
            let hex = graphiti_hex_encode(digest.as_slice());
            let suffix = format!("-{}", &hex[..16]);
            let max_prefix = 120usize.saturating_sub(suffix.len());
            candidate.truncate(max_prefix);
            candidate.push_str(&suffix);
            candidate
        }
    }
}

fn graphiti_hex_encode(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn build_promotion_template(kind: GraphitiEpisodeKind, title: Option<&str>, text: &str) -> String {
    let kind_str = match kind {
        GraphitiEpisodeKind::Decision => "decision",
        GraphitiEpisodeKind::LessonLearned => "lesson_learned",
        GraphitiEpisodeKind::Preference => "preference",
        GraphitiEpisodeKind::Procedure => "procedure",
        GraphitiEpisodeKind::TaskUpdate => "task_update",
        GraphitiEpisodeKind::Terminology => "terminology",
    };
    let mut out = format!("<graphiti_episode kind=\"{kind_str}\">\n");
    if let Some(title) = title {
        out.push_str(&format!("title: {title}\n"));
    }
    out.push_str("content:\n");
    out.push_str(text.trim());
    out.push_str("\n</graphiti_episode>");
    out
}

fn read_stdin_to_string() -> anyhow::Result<String> {
    use std::io::Read;

    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    Ok(buf)
}

/// As early as possible in the process lifecycle, apply hardening measures. We
/// skip this in debug builds to avoid interfering with debugging.
#[ctor::ctor]
#[cfg(not(debug_assertions))]
fn pre_main_hardening() {
    codex_process_hardening::pre_main_hardening();
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|codex_linux_sandbox_exe| async move {
        cli_main(codex_linux_sandbox_exe).await?;
        Ok(())
    })
}

async fn cli_main(codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let MultitoolCli {
        config_overrides: mut root_config_overrides,
        feature_toggles,
        mut interactive,
        subcommand,
    } = MultitoolCli::parse();

    // Fold --enable/--disable into config overrides so they flow to all subcommands.
    let toggle_overrides = feature_toggles.to_overrides()?;
    root_config_overrides.raw_overrides.extend(toggle_overrides);

    match subcommand {
        None => {
            prepend_config_flags(
                &mut interactive.config_overrides,
                root_config_overrides.clone(),
            );
            let exit_info = run_interactive_tui(interactive, codex_linux_sandbox_exe).await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Review(review_args)) => {
            let mut exec_cli = ExecCli::try_parse_from(["codex", "exec"])?;
            exec_cli.command = Some(ExecCommand::Review(review_args));
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::McpServer) => {
            codex_mcp_server::run_main(codex_linux_sandbox_exe, root_config_overrides).await?;
        }
        Some(Subcommand::Mcp(mut mcp_cli)) => {
            // Propagate any root-level config overrides (e.g. `-c key=value`).
            prepend_config_flags(&mut mcp_cli.config_overrides, root_config_overrides.clone());
            mcp_cli.run().await?;
        }
        Some(Subcommand::AppServer(app_server_cli)) => match app_server_cli.subcommand {
            None => {
                codex_app_server::run_main(codex_linux_sandbox_exe, root_config_overrides).await?;
            }
            Some(AppServerSubcommand::GenerateTs(gen_cli)) => {
                codex_app_server_protocol::generate_ts(
                    &gen_cli.out_dir,
                    gen_cli.prettier.as_deref(),
                )?;
            }
            Some(AppServerSubcommand::GenerateJsonSchema(gen_cli)) => {
                codex_app_server_protocol::generate_json(&gen_cli.out_dir)?;
            }
        },
        Some(Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            config_overrides,
        })) => {
            interactive = finalize_resume_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                all,
                config_overrides,
            );
            let exit_info = run_interactive_tui(interactive, codex_linux_sandbox_exe).await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Login(mut login_cli)) => {
            prepend_config_flags(
                &mut login_cli.config_overrides,
                root_config_overrides.clone(),
            );
            match login_cli.action {
                Some(LoginSubcommand::Status) => {
                    run_login_status(login_cli.config_overrides).await;
                }
                None => {
                    if login_cli.use_device_code {
                        run_login_with_device_code(
                            login_cli.config_overrides,
                            login_cli.issuer_base_url,
                            login_cli.client_id,
                        )
                        .await;
                    } else if login_cli.api_key.is_some() {
                        eprintln!(
                            "The --api-key flag is no longer supported. Pipe the key instead, e.g. `printenv OPENAI_API_KEY | codex login --with-api-key`."
                        );
                        std::process::exit(1);
                    } else if login_cli.with_api_key {
                        let api_key = read_api_key_from_stdin();
                        run_login_with_api_key(login_cli.config_overrides, api_key).await;
                    } else {
                        run_login_with_chatgpt(login_cli.config_overrides).await;
                    }
                }
            }
        }
        Some(Subcommand::Logout(mut logout_cli)) => {
            prepend_config_flags(
                &mut logout_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_logout(logout_cli.config_overrides).await;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            print_completion(completion_cli);
        }
        Some(Subcommand::Cloud(mut cloud_cli)) => {
            prepend_config_flags(
                &mut cloud_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_cloud_tasks::run_main(cloud_cli, codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Sandbox(sandbox_args)) => match sandbox_args.cmd {
            SandboxCommand::Macos(mut seatbelt_cli) => {
                prepend_config_flags(
                    &mut seatbelt_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_seatbelt(
                    seatbelt_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
            SandboxCommand::Linux(mut landlock_cli) => {
                prepend_config_flags(
                    &mut landlock_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_landlock(
                    landlock_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
            SandboxCommand::Windows(mut windows_cli) => {
                prepend_config_flags(
                    &mut windows_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::debug_sandbox::run_command_under_windows(
                    windows_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
        },
        Some(Subcommand::Execpolicy(ExecpolicyCommand { sub })) => match sub {
            ExecpolicySubcommand::Check(cmd) => run_execpolicycheck(cmd)?,
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            prepend_config_flags(
                &mut apply_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_apply_command(apply_cli, None).await?;
        }
        Some(Subcommand::ResponsesApiProxy(args)) => {
            tokio::task::spawn_blocking(move || codex_responses_api_proxy::run_main(args))
                .await??;
        }
        Some(Subcommand::StdioToUds(cmd)) => {
            let socket_path = cmd.socket_path;
            tokio::task::spawn_blocking(move || codex_stdio_to_uds::run(socket_path.as_path()))
                .await??;
        }
        Some(Subcommand::Features(FeaturesCli { sub })) => match sub {
            FeaturesSubcommand::List => {
                // Respect root-level `-c` overrides plus top-level flags like `--profile`.
                let mut cli_kv_overrides = root_config_overrides
                    .parse_overrides()
                    .map_err(anyhow::Error::msg)?;

                // Honor `--search` via the new feature toggle.
                if interactive.web_search {
                    cli_kv_overrides.push((
                        "features.web_search_request".to_string(),
                        toml::Value::Boolean(true),
                    ));
                }

                // Thread through relevant top-level flags (at minimum, `--profile`).
                let overrides = ConfigOverrides {
                    config_profile: interactive.config_profile.clone(),
                    ..Default::default()
                };

                let config = Config::load_with_cli_overrides(cli_kv_overrides, overrides).await?;
                for def in codex_core::features::FEATURES.iter() {
                    let name = def.key;
                    let stage = stage_str(def.stage);
                    let enabled = config.features.enabled(def.id);
                    println!("{name}\t{stage}\t{enabled}");
                }
            }
        },
        Some(Subcommand::Graphiti(mut graphiti_cli)) => {
            prepend_config_flags(
                &mut graphiti_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_graphiti_cli(graphiti_cli, interactive.config_profile.clone()).await?;
        }
    }

    Ok(())
}

/// Prepend root-level overrides so they have lower precedence than
/// CLI-specific ones specified after the subcommand (if any).
fn prepend_config_flags(
    subcommand_config_overrides: &mut CliConfigOverrides,
    cli_config_overrides: CliConfigOverrides,
) {
    subcommand_config_overrides
        .raw_overrides
        .splice(0..0, cli_config_overrides.raw_overrides);
}

/// Run the interactive Codex TUI, dispatching to either the legacy implementation or the
/// experimental TUI v2 shim based on feature flags resolved from config.
async fn run_interactive_tui(
    interactive: TuiCli,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<AppExitInfo> {
    if is_tui2_enabled(&interactive).await? {
        let result = tui2::run_main(interactive.into(), codex_linux_sandbox_exe).await?;
        Ok(result.into())
    } else {
        codex_tui::run_main(interactive, codex_linux_sandbox_exe).await
    }
}

/// Returns `Ok(true)` when the resolved configuration enables the `tui2` feature flag.
///
/// This performs a lightweight config load (honoring the same precedence as the lower-level TUI
/// bootstrap: `$CODEX_HOME`, config.toml, profile, and CLI `-c` overrides) solely to decide which
/// TUI frontend to launch. The full configuration is still loaded later by the interactive TUI.
async fn is_tui2_enabled(cli: &TuiCli) -> std::io::Result<bool> {
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = codex_common::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = overrides_cli
        .parse_overrides()
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidInput, e))?;

    let codex_home = find_codex_home()?;
    let config_toml = load_config_as_toml_with_cli_overrides(&codex_home, cli_kv_overrides).await?;
    let config_profile = config_toml.get_config_profile(cli.config_profile.clone())?;
    let overrides = FeatureOverrides::default();
    let features = Features::from_config(&config_toml, &config_profile, overrides);
    Ok(features.enabled(Feature::Tui2))
}

/// Build the final `TuiCli` for a `codex resume` invocation.
fn finalize_resume_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    show_all: bool,
    resume_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so resume shares the same
    // configuration surface area as `codex` without additional flags.
    let resume_session_id = session_id;
    interactive.resume_picker = resume_session_id.is_none() && !last;
    interactive.resume_last = last;
    interactive.resume_session_id = resume_session_id;
    interactive.resume_show_all = show_all;

    // Merge resume-scoped flags and overrides with highest precedence.
    merge_resume_cli_flags(&mut interactive, resume_cli);

    // Propagate any root-level config overrides (e.g. `-c key=value`).
    prepend_config_flags(&mut interactive.config_overrides, root_config_overrides);

    interactive
}

/// Merge flags provided to `codex resume` so they take precedence over any
/// root-level flags. Only overrides fields explicitly set on the resume-scoped
/// CLI. Also appends `-c key=value` overrides with highest precedence.
fn merge_resume_cli_flags(interactive: &mut TuiCli, resume_cli: TuiCli) {
    if let Some(model) = resume_cli.model {
        interactive.model = Some(model);
    }
    if resume_cli.oss {
        interactive.oss = true;
    }
    if let Some(profile) = resume_cli.config_profile {
        interactive.config_profile = Some(profile);
    }
    if let Some(sandbox) = resume_cli.sandbox_mode {
        interactive.sandbox_mode = Some(sandbox);
    }
    if let Some(approval) = resume_cli.approval_policy {
        interactive.approval_policy = Some(approval);
    }
    if resume_cli.full_auto {
        interactive.full_auto = true;
    }
    if resume_cli.dangerously_bypass_approvals_and_sandbox {
        interactive.dangerously_bypass_approvals_and_sandbox = true;
    }
    if let Some(cwd) = resume_cli.cwd {
        interactive.cwd = Some(cwd);
    }
    if resume_cli.web_search {
        interactive.web_search = true;
    }
    if !resume_cli.images.is_empty() {
        interactive.images = resume_cli.images;
    }
    if !resume_cli.add_dir.is_empty() {
        interactive.add_dir.extend(resume_cli.add_dir);
    }
    if let Some(prompt) = resume_cli.prompt {
        interactive.prompt = Some(prompt);
    }

    interactive
        .config_overrides
        .raw_overrides
        .extend(resume_cli.config_overrides.raw_overrides);
}

fn print_completion(cmd: CompletionCommand) {
    let mut app = MultitoolCli::command();
    let name = "codex";
    generate(cmd.shell, &mut app, name, &mut std::io::stdout());
}

#[cfg(test)]
mod tests {
    use super::*;
    use assert_matches::assert_matches;
    use codex_core::protocol::TokenUsage;
    use codex_protocol::ConversationId;
    use pretty_assertions::assert_eq;

    fn finalize_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
            feature_toggles: _,
        } = cli;

        let Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            config_overrides: resume_cli,
        }) = subcommand.expect("resume present")
        else {
            unreachable!()
        };

        finalize_resume_interactive(
            interactive,
            root_overrides,
            session_id,
            last,
            all,
            resume_cli,
        )
    }

    fn sample_exit_info(conversation: Option<&str>) -> AppExitInfo {
        let token_usage = TokenUsage {
            output_tokens: 2,
            total_tokens: 2,
            ..Default::default()
        };
        AppExitInfo {
            token_usage,
            conversation_id: conversation
                .map(ConversationId::from_string)
                .map(Result::unwrap),
            update_action: None,
        }
    }

    #[test]
    fn format_exit_messages_skips_zero_usage() {
        let exit_info = AppExitInfo {
            token_usage: TokenUsage::default(),
            conversation_id: None,
            update_action: None,
        };
        let lines = format_exit_messages(exit_info, false);
        assert!(lines.is_empty());
    }

    #[test]
    fn format_exit_messages_includes_resume_hint_without_color() {
        let exit_info = sample_exit_info(Some("123e4567-e89b-12d3-a456-426614174000"));
        let lines = format_exit_messages(exit_info, false);
        assert_eq!(
            lines,
            vec![
                "Token usage: total=2 input=0 output=2".to_string(),
                "To continue this session, run codex resume 123e4567-e89b-12d3-a456-426614174000"
                    .to_string(),
            ]
        );
    }

    #[test]
    fn format_exit_messages_applies_color_when_enabled() {
        let exit_info = sample_exit_info(Some("123e4567-e89b-12d3-a456-426614174000"));
        let lines = format_exit_messages(exit_info, true);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("\u{1b}[36m"));
    }

    #[test]
    fn resume_model_flag_applies_when_no_root_flags() {
        let interactive = finalize_from_args(["codex", "resume", "-m", "gpt-5.1-test"].as_ref());

        assert_eq!(interactive.model.as_deref(), Some("gpt-5.1-test"));
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn resume_picker_logic_none_and_not_last() {
        let interactive = finalize_from_args(["codex", "resume"].as_ref());
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_last() {
        let interactive = finalize_from_args(["codex", "resume", "--last"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_with_session_id() {
        let interactive = finalize_from_args(["codex", "resume", "1234"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("1234"));
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_all_flag_sets_show_all() {
        let interactive = finalize_from_args(["codex", "resume", "--all"].as_ref());
        assert!(interactive.resume_picker);
        assert!(interactive.resume_show_all);
    }

    #[test]
    fn resume_merges_option_flags_and_full_auto() {
        let interactive = finalize_from_args(
            [
                "codex",
                "resume",
                "sid",
                "--oss",
                "--full-auto",
                "--search",
                "--sandbox",
                "workspace-write",
                "--ask-for-approval",
                "on-request",
                "-m",
                "gpt-5.1-test",
                "-p",
                "my-profile",
                "-C",
                "/tmp",
                "-i",
                "/tmp/a.png,/tmp/b.png",
            ]
            .as_ref(),
        );

        assert_eq!(interactive.model.as_deref(), Some("gpt-5.1-test"));
        assert!(interactive.oss);
        assert_eq!(interactive.config_profile.as_deref(), Some("my-profile"));
        assert_matches!(
            interactive.sandbox_mode,
            Some(codex_common::SandboxModeCliArg::WorkspaceWrite)
        );
        assert_matches!(
            interactive.approval_policy,
            Some(codex_common::ApprovalModeCliArg::OnRequest)
        );
        assert!(interactive.full_auto);
        assert_eq!(
            interactive.cwd.as_deref(),
            Some(std::path::Path::new("/tmp"))
        );
        assert!(interactive.web_search);
        let has_a = interactive
            .images
            .iter()
            .any(|p| p == std::path::Path::new("/tmp/a.png"));
        let has_b = interactive
            .images
            .iter()
            .any(|p| p == std::path::Path::new("/tmp/b.png"));
        assert!(has_a && has_b);
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("sid"));
    }

    #[test]
    fn resume_merges_dangerously_bypass_flag() {
        let interactive = finalize_from_args(
            [
                "codex",
                "resume",
                "--dangerously-bypass-approvals-and-sandbox",
            ]
            .as_ref(),
        );
        assert!(interactive.dangerously_bypass_approvals_and_sandbox);
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn feature_toggles_known_features_generate_overrides() {
        let toggles = FeatureToggles {
            enable: vec!["web_search_request".to_string()],
            disable: vec!["unified_exec".to_string()],
        };
        let overrides = toggles.to_overrides().expect("valid features");
        assert_eq!(
            overrides,
            vec![
                "features.web_search_request=true".to_string(),
                "features.unified_exec=false".to_string(),
            ]
        );
    }

    #[test]
    fn feature_toggles_unknown_feature_errors() {
        let toggles = FeatureToggles {
            enable: vec!["does_not_exist".to_string()],
            disable: Vec::new(),
        };
        let err = toggles
            .to_overrides()
            .expect_err("feature should be rejected");
        assert_eq!(err.to_string(), "Unknown feature flag: does_not_exist");
    }
}
