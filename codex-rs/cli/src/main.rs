use clap::Args;
use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use codex_arg0::Arg0DispatchPaths;
use codex_arg0::arg0_dispatch_or_else;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_chatgpt::apply_command::run_apply_command;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cli::WindowsCommand;
use codex_cli::read_api_key_from_stdin;
use codex_cli::run_login_status;
use codex_cli::run_login_with_api_key;
use codex_cli::run_login_with_chatgpt;
use codex_cli::run_login_with_device_code;
use codex_cli::run_logout;
use codex_cloud_tasks::Cli as CloudTasksCli;
use codex_exec::Cli as ExecCli;
use codex_exec::Command as ExecCommand;
use codex_exec::ReviewArgs;
use codex_execpolicy::ExecPolicyCheckCommand;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;
use codex_rollout_trace::REDUCED_STATE_FILE_NAME;
use codex_rollout_trace::replay_bundle;
use codex_state::StateRuntime;
use codex_state::state_db_path;
use codex_tui::AppExitInfo;
use codex_tui::Cli as TuiCli;
use codex_tui::ExitReason;
use codex_tui::UpdateAction;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use owo_colors::OwoColorize;
use std::io::IsTerminal;
use std::path::PathBuf;
use supports_color::Stream;

#[cfg(any(target_os = "macos", target_os = "windows"))]
mod app_cmd;
#[cfg(any(target_os = "macos", target_os = "windows"))]
mod desktop_app;
mod marketplace_cmd;
mod mcp_cmd;
mod responses_cmd;
#[cfg(not(windows))]
mod wsl_paths;

use crate::marketplace_cmd::MarketplaceCli;
use crate::mcp_cmd::McpCli;
use crate::responses_cmd::ResponsesCommand;
use crate::responses_cmd::run_responses_command;

use codex_core::build_models_manager;
use codex_core::clear_memory_roots_contents;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::edit::ConfigEditsBuilder;
use codex_core::config::find_codex_home;
use codex_features::FEATURES;
use codex_features::Stage;
use codex_features::is_known_feature_key;
use codex_models_manager::AuthManager;
use codex_models_manager::bundled_models_response;
use codex_models_manager::collaboration_mode_presets::CollaborationModesConfig;
use codex_models_manager::manager::RefreshStrategy;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::user_input::UserInput;
use codex_terminal_detection::TerminalName;

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
    remote: InteractiveRemoteOptions,

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

    /// Manage external MCP servers for Codex.
    Mcp(McpCli),

    /// Manage Codex plugins.
    Plugin(PluginCli),

    /// Start Codex as an MCP server (stdio).
    McpServer,

    /// [experimental] Run the app server or related tooling.
    AppServer(AppServerCommand),

    /// Launch the Codex desktop app (opens the app installer if missing).
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    App(app_cmd::AppCommand),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Run commands within a Codex-provided sandbox.
    Sandbox(SandboxArgs),

    /// Debugging tools.
    Debug(DebugCommand),

    /// Execpolicy tooling.
    #[clap(hide = true)]
    Execpolicy(ExecpolicyCommand),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Resume a previous interactive session (picker by default; use --last to continue the most recent).
    Resume(ResumeCommand),

    /// Fork a previous interactive session (picker by default; use --last to fork the most recent).
    Fork(ForkCommand),

    /// [EXPERIMENTAL] Browse tasks from Codex Cloud and apply changes locally.
    #[clap(name = "cloud", alias = "cloud-tasks")]
    Cloud(CloudTasksCli),

    /// Internal: run the responses API proxy.
    #[clap(hide = true)]
    ResponsesApiProxy(ResponsesApiProxyArgs),

    /// Internal: send one raw Responses API payload through Codex auth.
    #[clap(hide = true)]
    Responses(ResponsesCommand),

    /// Internal: relay stdio to a Unix domain socket.
    #[clap(hide = true, name = "stdio-to-uds")]
    StdioToUds(StdioToUdsCommand),

    /// [EXPERIMENTAL] Run the standalone exec-server service.
    ExecServer(ExecServerCommand),

    /// Inspect feature flags.
    Features(FeaturesCli),
}

#[derive(Debug, Parser)]
#[command(bin_name = "codex plugin")]
struct PluginCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[command(subcommand)]
    subcommand: PluginSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum PluginSubcommand {
    /// Manage plugin marketplaces for Codex.
    Marketplace(MarketplaceCli),
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct DebugCommand {
    #[command(subcommand)]
    subcommand: DebugSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugSubcommand {
    /// Render the raw model catalog as JSON.
    Models(DebugModelsCommand),

    /// Tooling: helps debug the app server.
    AppServer(DebugAppServerCommand),

    /// Render the model-visible prompt input list as JSON.
    PromptInput(DebugPromptInputCommand),

    /// Replay a rollout trace bundle and write reduced state JSON.
    #[clap(hide = true)]
    TraceReduce(DebugTraceReduceCommand),

    /// Internal: reset local memory state for a fresh start.
    #[clap(hide = true)]
    ClearMemories,
}

#[derive(Debug, Parser)]
struct DebugAppServerCommand {
    #[command(subcommand)]
    subcommand: DebugAppServerSubcommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugAppServerSubcommand {
    // Send message to app server V2.
    SendMessageV2(DebugAppServerSendMessageV2Command),
}

#[derive(Debug, Parser)]
struct DebugAppServerSendMessageV2Command {
    #[arg(value_name = "USER_MESSAGE", required = true)]
    user_message: String,
}

#[derive(Debug, Parser)]
struct DebugPromptInputCommand {
    /// Optional user prompt to append after session context.
    #[arg(value_name = "PROMPT")]
    prompt: Option<String>,

    /// Optional image(s) to attach to the user prompt.
    #[arg(long = "image", short = 'i', value_name = "FILE", value_delimiter = ',', num_args = 1..)]
    images: Vec<PathBuf>,
}

#[derive(Debug, Parser)]
struct DebugModelsCommand {
    /// Skip refresh and dump only the bundled catalog shipped with this binary.
    #[arg(long = "bundled", default_value_t = false)]
    bundled: bool,
}

#[derive(Debug, Parser)]
struct DebugTraceReduceCommand {
    /// Trace bundle directory containing manifest.json and trace.jsonl.
    #[arg(value_name = "TRACE_BUNDLE")]
    trace_bundle: PathBuf,

    /// Output path for reduced RolloutTrace JSON. Defaults to TRACE_BUNDLE/state.json.
    #[arg(long = "output", short = 'o', value_name = "FILE")]
    output: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ResumeCommand {
    /// Conversation/session id (UUID) or thread name. UUIDs take precedence if it parses.
    /// If omitted, use --last to pick the most recent recorded session.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,

    /// Continue the most recent session without showing the picker.
    #[arg(long = "last", default_value_t = false)]
    last: bool,

    /// Show all sessions (disables cwd filtering and shows CWD column).
    #[arg(long = "all", default_value_t = false)]
    all: bool,

    /// Include non-interactive sessions in the resume picker and --last selection.
    #[arg(long = "include-non-interactive", default_value_t = false)]
    include_non_interactive: bool,

    #[clap(flatten)]
    remote: InteractiveRemoteOptions,

    #[clap(flatten)]
    config_overrides: TuiCli,
}

#[derive(Debug, Parser)]
struct ForkCommand {
    /// Conversation/session id (UUID). When provided, forks this session.
    /// If omitted, use --last to pick the most recent recorded session.
    #[arg(value_name = "SESSION_ID")]
    session_id: Option<String>,

    /// Fork the most recent session without showing the picker.
    #[arg(long = "last", default_value_t = false, conflicts_with = "session_id")]
    last: bool,

    /// Show all sessions (disables cwd filtering and shows CWD column).
    #[arg(long = "all", default_value_t = false)]
    all: bool,

    #[clap(flatten)]
    remote: InteractiveRemoteOptions,

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

    /// Run a command under the Linux sandbox (bubblewrap by default).
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
        num_args = 0..=1,
        default_missing_value = "",
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

    /// Transport endpoint URL. Supported values: `stdio://` (default),
    /// `unix://`, `unix://PATH`, `ws://IP:PORT`, `off`.
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = codex_app_server::AppServerTransport::DEFAULT_LISTEN_URL
    )]
    listen: codex_app_server::AppServerTransport,

    /// Controls whether analytics are enabled by default.
    ///
    /// Analytics are disabled by default for app-server. Users have to explicitly opt in
    /// via the `analytics` section in the config.toml file.
    ///
    /// However, for first-party use cases like the VSCode IDE extension, we default analytics
    /// to be enabled by default by setting this flag. Users can still opt out by setting this
    /// in their config.toml:
    ///
    /// ```toml
    /// [analytics]
    /// enabled = false
    /// ```
    ///
    /// See https://developers.openai.com/codex/config-advanced/#metrics for more details.
    #[arg(long = "analytics-default-enabled")]
    analytics_default_enabled: bool,

    #[command(flatten)]
    auth: codex_app_server::AppServerWebsocketAuthArgs,
}

#[derive(Debug, Parser)]
struct ExecServerCommand {
    /// Transport endpoint URL. Supported values: `ws://IP:PORT` (default).
    #[arg(
        long = "listen",
        value_name = "URL",
        default_value = "ws://127.0.0.1:0"
    )]
    listen: String,
}

#[derive(Debug, clap::Subcommand)]
#[allow(clippy::enum_variant_names)]
enum AppServerSubcommand {
    /// Proxy stdio bytes to the running app-server control socket.
    Proxy(AppServerProxyCommand),

    /// [experimental] Generate TypeScript bindings for the app server protocol.
    GenerateTs(GenerateTsCommand),

    /// [experimental] Generate JSON Schema for the app server protocol.
    GenerateJsonSchema(GenerateJsonSchemaCommand),

    /// [internal] Generate internal JSON Schema artifacts for Codex tooling.
    #[clap(hide = true)]
    GenerateInternalJsonSchema(GenerateInternalJsonSchemaCommand),
}

#[derive(Debug, Args)]
struct AppServerProxyCommand {
    /// Path to the app-server Unix domain socket to connect to.
    #[arg(long = "sock", value_name = "SOCKET_PATH", value_parser = parse_socket_path)]
    socket_path: Option<AbsolutePathBuf>,
}

#[derive(Debug, Args)]
struct GenerateTsCommand {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,

    /// Include experimental methods and fields in the generated output
    #[arg(long = "experimental", default_value_t = false)]
    experimental: bool,
}

#[derive(Debug, Args)]
struct GenerateJsonSchemaCommand {
    /// Output directory where the schema bundle will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Include experimental methods and fields in the generated output
    #[arg(long = "experimental", default_value_t = false)]
    experimental: bool,
}

#[derive(Debug, Args)]
struct GenerateInternalJsonSchemaCommand {
    /// Output directory where internal JSON Schema artifacts will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,
}

#[derive(Debug, Parser)]
struct StdioToUdsCommand {
    /// Path to the Unix domain socket to connect to.
    #[arg(value_name = "SOCKET_PATH", value_parser = parse_socket_path)]
    socket_path: AbsolutePathBuf,
}

fn parse_socket_path(raw: &str) -> Result<AbsolutePathBuf, String> {
    AbsolutePathBuf::relative_to_current_dir(raw)
        .map_err(|err| format!("failed to resolve socket path `{raw}`: {err}"))
}

fn format_exit_messages(exit_info: AppExitInfo, color_enabled: bool) -> Vec<String> {
    let AppExitInfo {
        token_usage,
        thread_id: conversation_id,
        ..
    } = exit_info;

    let mut lines = Vec::new();
    if !token_usage.is_zero() {
        lines.push(format!(
            "{}",
            codex_protocol::protocol::FinalOutput::from(token_usage)
        ));
    }

    if let Some(resume_cmd) =
        codex_core::util::resume_command(/*thread_name*/ None, conversation_id)
    {
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
    match exit_info.exit_reason {
        ExitReason::Fatal(message) => {
            eprintln!("ERROR: {message}");
            std::process::exit(1);
        }
        ExitReason::UserRequested => { /* normal exit */ }
    }

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
            if action == UpdateAction::StandaloneWindows {
                let (cmd, args) = action.command_args();
                // Run the standalone PowerShell installer with PowerShell
                // itself. Routing this through `cmd.exe /C` would parse
                // PowerShell metacharacters like `|` before PowerShell sees
                // the installer command.
                std::process::Command::new(cmd).args(args).status()?
            } else {
                // On Windows, run via cmd.exe so .CMD/.BAT are correctly resolved (PATHEXT semantics).
                std::process::Command::new("cmd")
                    .args(["/C", &cmd_str])
                    .status()?
            }
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
    println!("\n🎉 Update ran successfully! Please restart Codex.");
    Ok(())
}

fn run_execpolicycheck(cmd: ExecPolicyCheckCommand) -> anyhow::Result<()> {
    cmd.run()
}

async fn run_debug_app_server_command(cmd: DebugAppServerCommand) -> anyhow::Result<()> {
    match cmd.subcommand {
        DebugAppServerSubcommand::SendMessageV2(cmd) => {
            let codex_bin = std::env::current_exe()?;
            codex_app_server_test_client::send_message_v2(&codex_bin, &[], cmd.user_message, &None)
                .await
        }
    }
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

#[derive(Debug, Default, Parser, Clone)]
struct InteractiveRemoteOptions {
    /// Connect the TUI to a remote app server websocket endpoint.
    ///
    /// Accepted forms: `ws://host:port` or `wss://host:port`.
    #[arg(long = "remote", value_name = "ADDR")]
    remote: Option<String>,

    /// Name of the environment variable containing the bearer token to send to
    /// a remote app server websocket.
    #[arg(long = "remote-auth-token-env", value_name = "ENV_VAR")]
    remote_auth_token_env: Option<String>,
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
    /// Enable a feature in config.toml.
    Enable(FeatureSetArgs),
    /// Disable a feature in config.toml.
    Disable(FeatureSetArgs),
}

#[derive(Debug, Parser)]
struct FeatureSetArgs {
    /// Feature key to update (for example: unified_exec).
    feature: String,
}

fn stage_str(stage: Stage) -> &'static str {
    match stage {
        Stage::UnderDevelopment => "under development",
        Stage::Experimental { .. } => "experimental",
        Stage::Stable => "stable",
        Stage::Deprecated => "deprecated",
        Stage::Removed => "removed",
    }
}

fn main() -> anyhow::Result<()> {
    arg0_dispatch_or_else(|arg0_paths: Arg0DispatchPaths| async move {
        cli_main(arg0_paths).await?;
        Ok(())
    })
}

async fn cli_main(arg0_paths: Arg0DispatchPaths) -> anyhow::Result<()> {
    let MultitoolCli {
        config_overrides: mut root_config_overrides,
        feature_toggles,
        remote,
        mut interactive,
        subcommand,
    } = MultitoolCli::parse();

    // Fold --enable/--disable into config overrides so they flow to all subcommands.
    let toggle_overrides = feature_toggles.to_overrides()?;
    root_config_overrides.raw_overrides.extend(toggle_overrides);
    let root_remote = remote.remote;
    let root_remote_auth_token_env = remote.remote_auth_token_env;

    match subcommand {
        None => {
            prepend_config_flags(
                &mut interactive.config_overrides,
                root_config_overrides.clone(),
            );
            let exit_info = run_interactive_tui(
                interactive,
                root_remote.clone(),
                root_remote_auth_token_env.clone(),
                arg0_paths.clone(),
            )
            .await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "exec",
            )?;
            exec_cli
                .shared
                .inherit_exec_root_options(&interactive.shared);
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, arg0_paths.clone()).await?;
        }
        Some(Subcommand::Review(review_args)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "review",
            )?;
            let mut exec_cli = ExecCli::try_parse_from(["codex", "exec"])?;
            exec_cli.command = Some(ExecCommand::Review(review_args));
            prepend_config_flags(
                &mut exec_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_exec::run_main(exec_cli, arg0_paths.clone()).await?;
        }
        Some(Subcommand::McpServer) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "mcp-server",
            )?;
            codex_mcp_server::run_main(arg0_paths.clone(), root_config_overrides).await?;
        }
        Some(Subcommand::Mcp(mut mcp_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "mcp",
            )?;
            // Propagate any root-level config overrides (e.g. `-c key=value`).
            prepend_config_flags(&mut mcp_cli.config_overrides, root_config_overrides.clone());
            mcp_cli.run().await?;
        }
        Some(Subcommand::Plugin(plugin_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "plugin",
            )?;
            let PluginCli {
                mut config_overrides,
                subcommand,
            } = plugin_cli;
            prepend_config_flags(&mut config_overrides, root_config_overrides.clone());
            match subcommand {
                PluginSubcommand::Marketplace(mut marketplace_cli) => {
                    prepend_config_flags(&mut marketplace_cli.config_overrides, config_overrides);
                    marketplace_cli.run().await?;
                }
            }
        }
        Some(Subcommand::AppServer(app_server_cli)) => {
            let AppServerCommand {
                subcommand,
                listen,
                analytics_default_enabled,
                auth,
            } = app_server_cli;
            reject_remote_mode_for_app_server_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                subcommand.as_ref(),
            )?;
            match subcommand {
                None => {
                    let transport = listen;
                    let auth = auth.try_into_settings()?;
                    codex_app_server::run_main_with_transport(
                        arg0_paths.clone(),
                        root_config_overrides,
                        codex_core::config_loader::LoaderOverrides::default(),
                        analytics_default_enabled,
                        transport,
                        codex_protocol::protocol::SessionSource::VSCode,
                        auth,
                    )
                    .await?;
                }
                Some(AppServerSubcommand::Proxy(proxy_cli)) => {
                    let socket_path = match proxy_cli.socket_path {
                        Some(socket_path) => socket_path,
                        None => {
                            let codex_home = find_codex_home()?;
                            codex_app_server::app_server_control_socket_path(&codex_home)?
                        }
                    };
                    codex_stdio_to_uds::run(socket_path.as_path()).await?;
                }
                Some(AppServerSubcommand::GenerateTs(gen_cli)) => {
                    let options = codex_app_server_protocol::GenerateTsOptions {
                        experimental_api: gen_cli.experimental,
                        ..Default::default()
                    };
                    codex_app_server_protocol::generate_ts_with_options(
                        &gen_cli.out_dir,
                        gen_cli.prettier.as_deref(),
                        options,
                    )?;
                }
                Some(AppServerSubcommand::GenerateJsonSchema(gen_cli)) => {
                    codex_app_server_protocol::generate_json_with_experimental(
                        &gen_cli.out_dir,
                        gen_cli.experimental,
                    )?;
                }
                Some(AppServerSubcommand::GenerateInternalJsonSchema(gen_cli)) => {
                    codex_app_server_protocol::generate_internal_json_schema(&gen_cli.out_dir)?;
                }
            }
        }
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        Some(Subcommand::App(app_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "app",
            )?;
            app_cmd::run_app(app_cli).await?;
        }
        Some(Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            include_non_interactive,
            remote,
            config_overrides,
        })) => {
            interactive = finalize_resume_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                all,
                include_non_interactive,
                config_overrides,
            );
            let exit_info = run_interactive_tui(
                interactive,
                remote.remote.or(root_remote.clone()),
                remote
                    .remote_auth_token_env
                    .or(root_remote_auth_token_env.clone()),
                arg0_paths.clone(),
            )
            .await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Fork(ForkCommand {
            session_id,
            last,
            all,
            remote,
            config_overrides,
        })) => {
            interactive = finalize_fork_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                all,
                config_overrides,
            );
            let exit_info = run_interactive_tui(
                interactive,
                remote.remote.or(root_remote.clone()),
                remote
                    .remote_auth_token_env
                    .or(root_remote_auth_token_env.clone()),
                arg0_paths.clone(),
            )
            .await?;
            handle_app_exit(exit_info)?;
        }
        Some(Subcommand::Login(mut login_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "login",
            )?;
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
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "logout",
            )?;
            prepend_config_flags(
                &mut logout_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_logout(logout_cli.config_overrides).await;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "completion",
            )?;
            print_completion(completion_cli);
        }
        Some(Subcommand::Cloud(mut cloud_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "cloud",
            )?;
            prepend_config_flags(
                &mut cloud_cli.config_overrides,
                root_config_overrides.clone(),
            );
            codex_cloud_tasks::run_main(cloud_cli, arg0_paths.codex_linux_sandbox_exe.clone())
                .await?;
        }
        Some(Subcommand::Sandbox(sandbox_args)) => match sandbox_args.cmd {
            SandboxCommand::Macos(mut seatbelt_cli) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "sandbox macos",
                )?;
                prepend_config_flags(
                    &mut seatbelt_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::run_command_under_seatbelt(
                    seatbelt_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
            SandboxCommand::Linux(mut landlock_cli) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "sandbox linux",
                )?;
                prepend_config_flags(
                    &mut landlock_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::run_command_under_landlock(
                    landlock_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
            SandboxCommand::Windows(mut windows_cli) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "sandbox windows",
                )?;
                prepend_config_flags(
                    &mut windows_cli.config_overrides,
                    root_config_overrides.clone(),
                );
                codex_cli::run_command_under_windows(
                    windows_cli,
                    arg0_paths.codex_linux_sandbox_exe.clone(),
                )
                .await?;
            }
        },
        Some(Subcommand::Debug(DebugCommand { subcommand })) => match subcommand {
            DebugSubcommand::Models(cmd) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "debug models",
                )?;
                run_debug_models_command(cmd, root_config_overrides).await?;
            }
            DebugSubcommand::AppServer(cmd) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "debug app-server",
                )?;
                run_debug_app_server_command(cmd).await?;
            }
            DebugSubcommand::PromptInput(cmd) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "debug prompt-input",
                )?;
                run_debug_prompt_input_command(
                    cmd,
                    root_config_overrides,
                    interactive,
                    arg0_paths.clone(),
                )
                .await?;
            }
            DebugSubcommand::TraceReduce(cmd) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "debug trace-reduce",
                )?;
                run_debug_trace_reduce_command(cmd).await?;
            }
            DebugSubcommand::ClearMemories => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "debug clear-memories",
                )?;
                run_debug_clear_memories_command(&root_config_overrides, &interactive).await?;
            }
        },
        Some(Subcommand::Execpolicy(ExecpolicyCommand { sub })) => match sub {
            ExecpolicySubcommand::Check(cmd) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "execpolicy check",
                )?;
                run_execpolicycheck(cmd)?
            }
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "apply",
            )?;
            prepend_config_flags(
                &mut apply_cli.config_overrides,
                root_config_overrides.clone(),
            );
            run_apply_command(apply_cli, /*cwd*/ None).await?;
        }
        Some(Subcommand::ResponsesApiProxy(args)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "responses-api-proxy",
            )?;
            tokio::task::spawn_blocking(move || codex_responses_api_proxy::run_main(args))
                .await??;
        }
        Some(Subcommand::Responses(ResponsesCommand {})) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "responses",
            )?;
            run_responses_command(root_config_overrides).await?;
        }
        Some(Subcommand::StdioToUds(cmd)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "stdio-to-uds",
            )?;
            let socket_path = cmd.socket_path;
            codex_stdio_to_uds::run(socket_path.as_path()).await?;
        }
        Some(Subcommand::ExecServer(cmd)) => {
            reject_remote_mode_for_subcommand(
                root_remote.as_deref(),
                root_remote_auth_token_env.as_deref(),
                "exec-server",
            )?;
            run_exec_server_command(cmd, &arg0_paths).await?;
        }
        Some(Subcommand::Features(FeaturesCli { sub })) => match sub {
            FeaturesSubcommand::List => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "features list",
                )?;
                // Respect root-level `-c` overrides plus top-level flags like `--profile`.
                let mut cli_kv_overrides = root_config_overrides
                    .parse_overrides()
                    .map_err(anyhow::Error::msg)?;

                // Honor `--search` via the canonical web_search mode.
                if interactive.web_search {
                    cli_kv_overrides.push((
                        "web_search".to_string(),
                        toml::Value::String("live".to_string()),
                    ));
                }

                // Thread through relevant top-level flags (at minimum, `--profile`).
                let overrides = ConfigOverrides {
                    config_profile: interactive.config_profile.clone(),
                    ..Default::default()
                };

                let config = Config::load_with_cli_overrides_and_harness_overrides(
                    cli_kv_overrides,
                    overrides,
                )
                .await?;
                let mut rows = Vec::with_capacity(FEATURES.len());
                let mut name_width = 0;
                let mut stage_width = 0;
                for def in FEATURES {
                    let name = def.key;
                    let stage = stage_str(def.stage);
                    let enabled = config.features.enabled(def.id);
                    name_width = name_width.max(name.len());
                    stage_width = stage_width.max(stage.len());
                    rows.push((name, stage, enabled));
                }
                rows.sort_unstable_by_key(|(name, _, _)| *name);

                for (name, stage, enabled) in rows {
                    println!("{name:<name_width$}  {stage:<stage_width$}  {enabled}");
                }
            }
            FeaturesSubcommand::Enable(FeatureSetArgs { feature }) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "features enable",
                )?;
                enable_feature_in_config(&interactive, &feature).await?;
            }
            FeaturesSubcommand::Disable(FeatureSetArgs { feature }) => {
                reject_remote_mode_for_subcommand(
                    root_remote.as_deref(),
                    root_remote_auth_token_env.as_deref(),
                    "features disable",
                )?;
                disable_feature_in_config(&interactive, &feature).await?;
            }
        },
    }

    Ok(())
}

async fn run_exec_server_command(
    cmd: ExecServerCommand,
    arg0_paths: &Arg0DispatchPaths,
) -> anyhow::Result<()> {
    let codex_self_exe = arg0_paths
        .codex_self_exe
        .clone()
        .ok_or_else(|| anyhow::anyhow!("Codex executable path is not configured"))?;
    let runtime_paths = codex_exec_server::ExecServerRuntimePaths::new(
        codex_self_exe,
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    codex_exec_server::run_main(&cmd.listen, runtime_paths)
        .await
        .map_err(anyhow::Error::from_boxed)
}

async fn enable_feature_in_config(interactive: &TuiCli, feature: &str) -> anyhow::Result<()> {
    FeatureToggles::validate_feature(feature)?;
    let codex_home = find_codex_home()?;
    ConfigEditsBuilder::new(&codex_home)
        .with_profile(interactive.config_profile.as_deref())
        .set_feature_enabled(feature, /*enabled*/ true)
        .apply()
        .await?;
    println!("Enabled feature `{feature}` in config.toml.");
    maybe_print_under_development_feature_warning(&codex_home, interactive, feature);
    Ok(())
}

async fn disable_feature_in_config(interactive: &TuiCli, feature: &str) -> anyhow::Result<()> {
    FeatureToggles::validate_feature(feature)?;
    let codex_home = find_codex_home()?;
    ConfigEditsBuilder::new(&codex_home)
        .with_profile(interactive.config_profile.as_deref())
        .set_feature_enabled(feature, /*enabled*/ false)
        .apply()
        .await?;
    println!("Disabled feature `{feature}` in config.toml.");
    Ok(())
}

fn maybe_print_under_development_feature_warning(
    codex_home: &std::path::Path,
    interactive: &TuiCli,
    feature: &str,
) {
    if interactive.config_profile.is_some() {
        return;
    }

    let Some(spec) = FEATURES.iter().find(|spec| spec.key == feature) else {
        return;
    };
    if !matches!(spec.stage, Stage::UnderDevelopment) {
        return;
    }

    let config_path = codex_home.join(codex_config::CONFIG_TOML_FILE);
    eprintln!(
        "Under-development features enabled: {feature}. Under-development features are incomplete and may behave unpredictably. To suppress this warning, set `suppress_unstable_features_warning = true` in {}.",
        config_path.display()
    );
}

async fn run_debug_trace_reduce_command(cmd: DebugTraceReduceCommand) -> anyhow::Result<()> {
    let output = cmd
        .output
        .unwrap_or_else(|| cmd.trace_bundle.join(REDUCED_STATE_FILE_NAME));

    let trace = replay_bundle(&cmd.trace_bundle)?;
    let reduced_json = serde_json::to_vec_pretty(&trace)?;
    tokio::fs::write(&output, reduced_json).await?;
    println!("{}", output.display());

    Ok(())
}

async fn run_debug_prompt_input_command(
    cmd: DebugPromptInputCommand,
    root_config_overrides: CliConfigOverrides,
    interactive: TuiCli,
    arg0_paths: Arg0DispatchPaths,
) -> anyhow::Result<()> {
    let shared = interactive.shared.into_inner();
    let mut cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    if interactive.web_search {
        cli_kv_overrides.push((
            "web_search".to_string(),
            toml::Value::String("live".to_string()),
        ));
    }

    let approval_policy = if shared.full_auto {
        Some(AskForApproval::OnRequest)
    } else if shared.dangerously_bypass_approvals_and_sandbox {
        Some(AskForApproval::Never)
    } else {
        interactive.approval_policy.map(Into::into)
    };
    let sandbox_mode = if shared.full_auto {
        Some(codex_protocol::config_types::SandboxMode::WorkspaceWrite)
    } else if shared.dangerously_bypass_approvals_and_sandbox {
        Some(codex_protocol::config_types::SandboxMode::DangerFullAccess)
    } else {
        shared.sandbox_mode.map(Into::into)
    };
    let overrides = ConfigOverrides {
        model: shared.model,
        config_profile: shared.config_profile,
        approval_policy,
        sandbox_mode,
        cwd: shared.cwd,
        codex_self_exe: arg0_paths.codex_self_exe,
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe,
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe,
        show_raw_agent_reasoning: shared.oss.then_some(true),
        ephemeral: Some(true),
        additional_writable_roots: shared.add_dir,
        ..Default::default()
    };
    let config =
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides).await?;

    let mut input = shared
        .images
        .into_iter()
        .chain(cmd.images)
        .map(|path| UserInput::LocalImage { path })
        .collect::<Vec<_>>();
    if let Some(prompt) = cmd.prompt.or(interactive.prompt) {
        input.push(UserInput::Text {
            text: prompt.replace("\r\n", "\n").replace('\r', "\n"),
            text_elements: Vec::new(),
        });
    }

    let prompt_input = codex_core::build_prompt_input(config, input).await?;
    println!("{}", serde_json::to_string_pretty(&prompt_input)?);

    Ok(())
}

async fn run_debug_models_command(
    cmd: DebugModelsCommand,
    root_config_overrides: CliConfigOverrides,
) -> anyhow::Result<()> {
    let catalog = if cmd.bundled {
        bundled_models_response()?
    } else {
        let cli_overrides = root_config_overrides
            .parse_overrides()
            .map_err(anyhow::Error::msg)?;
        let config = Config::load_with_cli_overrides(cli_overrides).await?;
        let auth_manager =
            AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ true);
        let models_manager =
            build_models_manager(&config, auth_manager, CollaborationModesConfig::default());
        models_manager
            .raw_model_catalog(RefreshStrategy::OnlineIfUncached)
            .await
    };

    serde_json::to_writer(std::io::stdout(), &catalog)?;
    println!();
    Ok(())
}

async fn run_debug_clear_memories_command(
    root_config_overrides: &CliConfigOverrides,
    interactive: &TuiCli,
) -> anyhow::Result<()> {
    let cli_kv_overrides = root_config_overrides
        .parse_overrides()
        .map_err(anyhow::Error::msg)?;
    let overrides = ConfigOverrides {
        config_profile: interactive.config_profile.clone(),
        ..Default::default()
    };
    let config =
        Config::load_with_cli_overrides_and_harness_overrides(cli_kv_overrides, overrides).await?;

    let state_path = state_db_path(config.sqlite_home.as_path());
    let mut cleared_state_db = false;
    if tokio::fs::try_exists(&state_path).await? {
        let state_db =
            StateRuntime::init(config.sqlite_home.clone(), config.model_provider_id.clone())
                .await?;
        state_db.clear_memory_data().await?;
        cleared_state_db = true;
    }

    clear_memory_roots_contents(&config.codex_home).await?;

    let mut message = if cleared_state_db {
        format!("Cleared memory state from {}.", state_path.display())
    } else {
        format!("No state db found at {}.", state_path.display())
    };
    message.push_str(&format!(
        " Cleared memory directories under {}.",
        config.codex_home.display()
    ));

    println!("{message}");

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

fn reject_remote_mode_for_subcommand(
    remote: Option<&str>,
    remote_auth_token_env: Option<&str>,
    subcommand: &str,
) -> anyhow::Result<()> {
    if let Some(remote) = remote {
        anyhow::bail!(
            "`--remote {remote}` is only supported for interactive TUI commands, not `codex {subcommand}`"
        );
    }
    if remote_auth_token_env.is_some() {
        anyhow::bail!(
            "`--remote-auth-token-env` is only supported for interactive TUI commands, not `codex {subcommand}`"
        );
    }
    Ok(())
}

fn reject_remote_mode_for_app_server_subcommand(
    remote: Option<&str>,
    remote_auth_token_env: Option<&str>,
    subcommand: Option<&AppServerSubcommand>,
) -> anyhow::Result<()> {
    let subcommand_name = match subcommand {
        None => "app-server",
        Some(AppServerSubcommand::Proxy(_)) => "app-server proxy",
        Some(AppServerSubcommand::GenerateTs(_)) => "app-server generate-ts",
        Some(AppServerSubcommand::GenerateJsonSchema(_)) => "app-server generate-json-schema",
        Some(AppServerSubcommand::GenerateInternalJsonSchema(_)) => {
            "app-server generate-internal-json-schema"
        }
    };
    reject_remote_mode_for_subcommand(remote, remote_auth_token_env, subcommand_name)
}

fn read_remote_auth_token_from_env_var_with<F>(
    env_var_name: &str,
    get_var: F,
) -> anyhow::Result<String>
where
    F: FnOnce(&str) -> Result<String, std::env::VarError>,
{
    let auth_token = get_var(env_var_name)
        .map_err(|_| anyhow::anyhow!("environment variable `{env_var_name}` is not set"))?;
    let auth_token = auth_token.trim().to_string();
    if auth_token.is_empty() {
        anyhow::bail!("environment variable `{env_var_name}` is empty");
    }
    Ok(auth_token)
}

fn read_remote_auth_token_from_env_var(env_var_name: &str) -> anyhow::Result<String> {
    read_remote_auth_token_from_env_var_with(env_var_name, |name| std::env::var(name))
}

async fn run_interactive_tui(
    mut interactive: TuiCli,
    remote: Option<String>,
    remote_auth_token_env: Option<String>,
    arg0_paths: Arg0DispatchPaths,
) -> std::io::Result<AppExitInfo> {
    if let Some(prompt) = interactive.prompt.take() {
        // Normalize CRLF/CR to LF so CLI-provided text can't leak `\r` into TUI state.
        interactive.prompt = Some(prompt.replace("\r\n", "\n").replace('\r', "\n"));
    }

    let terminal_info = codex_terminal_detection::terminal_info();
    if terminal_info.name == TerminalName::Dumb {
        if !(std::io::stdin().is_terminal() && std::io::stderr().is_terminal()) {
            return Ok(AppExitInfo::fatal(
                "TERM is set to \"dumb\". Refusing to start the interactive TUI because no terminal is available for a confirmation prompt (stdin/stderr is not a TTY). Run in a supported terminal or unset TERM.",
            ));
        }

        eprintln!(
            "WARNING: TERM is set to \"dumb\". Codex's interactive TUI may not work in this terminal."
        );
        if !confirm("Continue anyway? [y/N]: ")? {
            return Ok(AppExitInfo::fatal(
                "Refusing to start the interactive TUI because TERM is set to \"dumb\". Run in a supported terminal or unset TERM.",
            ));
        }
    }

    let normalized_remote = remote
        .as_deref()
        .map(codex_tui::normalize_remote_addr)
        .transpose()
        .map_err(std::io::Error::other)?;
    if remote_auth_token_env.is_some() && normalized_remote.is_none() {
        return Ok(AppExitInfo::fatal(
            "`--remote-auth-token-env` requires `--remote`.",
        ));
    }
    let remote_auth_token = remote_auth_token_env
        .as_deref()
        .map(read_remote_auth_token_from_env_var)
        .transpose()
        .map_err(std::io::Error::other)?;
    codex_tui::run_main(
        interactive,
        arg0_paths,
        codex_core::config_loader::LoaderOverrides::default(),
        normalized_remote,
        remote_auth_token,
    )
    .await
}

fn confirm(prompt: &str) -> std::io::Result<bool> {
    eprintln!("{prompt}");

    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let answer = input.trim();
    Ok(answer.eq_ignore_ascii_case("y") || answer.eq_ignore_ascii_case("yes"))
}

/// Build the final `TuiCli` for a `codex resume` invocation.
fn finalize_resume_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    show_all: bool,
    include_non_interactive: bool,
    resume_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so resume shares the same
    // configuration surface area as `codex` without additional flags.
    let resume_session_id = session_id;
    interactive.resume_picker = resume_session_id.is_none() && !last;
    interactive.resume_last = last;
    interactive.resume_session_id = resume_session_id;
    interactive.resume_show_all = show_all;
    interactive.resume_include_non_interactive = include_non_interactive;

    // Merge resume-scoped flags and overrides with highest precedence.
    merge_interactive_cli_flags(&mut interactive, resume_cli);

    // Propagate any root-level config overrides (e.g. `-c key=value`).
    prepend_config_flags(&mut interactive.config_overrides, root_config_overrides);

    interactive
}

/// Build the final `TuiCli` for a `codex fork` invocation.
fn finalize_fork_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    show_all: bool,
    fork_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so fork shares the same
    // configuration surface area as `codex` without additional flags.
    let fork_session_id = session_id;
    interactive.fork_picker = fork_session_id.is_none() && !last;
    interactive.fork_last = last;
    interactive.fork_session_id = fork_session_id;
    interactive.fork_show_all = show_all;

    // Merge fork-scoped flags and overrides with highest precedence.
    merge_interactive_cli_flags(&mut interactive, fork_cli);

    // Propagate any root-level config overrides (e.g. `-c key=value`).
    prepend_config_flags(&mut interactive.config_overrides, root_config_overrides);

    interactive
}

/// Merge flags provided to `codex resume`/`codex fork` so they take precedence over any
/// root-level flags. Only overrides fields explicitly set on the subcommand-scoped
/// CLI. Also appends `-c key=value` overrides with highest precedence.
fn merge_interactive_cli_flags(interactive: &mut TuiCli, subcommand_cli: TuiCli) {
    let TuiCli {
        shared,
        approval_policy,
        web_search,
        prompt,
        config_overrides,
        ..
    } = subcommand_cli;
    interactive
        .shared
        .apply_subcommand_overrides(shared.into_inner());
    if let Some(approval) = approval_policy {
        interactive.approval_policy = Some(approval);
    }
    if web_search {
        interactive.web_search = true;
    }
    if let Some(prompt) = prompt {
        // Normalize CRLF/CR to LF so CLI-provided text can't leak `\r` into TUI state.
        interactive.prompt = Some(prompt.replace("\r\n", "\n").replace('\r', "\n"));
    }

    interactive
        .config_overrides
        .raw_overrides
        .extend(config_overrides.raw_overrides);
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
    use codex_protocol::ThreadId;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;

    fn finalize_resume_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
            feature_toggles: _,
            remote: _,
        } = cli;

        let Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            all,
            include_non_interactive,
            remote: _,
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
            include_non_interactive,
            resume_cli,
        )
    }

    fn finalize_fork_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
            feature_toggles: _,
            remote: _,
        } = cli;

        let Subcommand::Fork(ForkCommand {
            session_id,
            last,
            all,
            remote: _,
            config_overrides: fork_cli,
        }) = subcommand.expect("fork present")
        else {
            unreachable!()
        };

        finalize_fork_interactive(interactive, root_overrides, session_id, last, all, fork_cli)
    }

    #[test]
    fn exec_resume_last_accepts_prompt_positional() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "exec", "--json", "resume", "--last", "2+2"])
                .expect("parse should succeed");

        let Some(Subcommand::Exec(exec)) = cli.subcommand else {
            panic!("expected exec subcommand");
        };
        let Some(codex_exec::Command::Resume(args)) = exec.command else {
            panic!("expected exec resume");
        };

        assert!(args.last);
        assert_eq!(args.session_id, None);
        assert_eq!(args.prompt.as_deref(), Some("2+2"));
    }

    #[test]
    fn exec_resume_accepts_output_last_message_flag_after_subcommand() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "exec",
            "resume",
            "session-123",
            "-o",
            "/tmp/resume-output.md",
            "re-review",
        ])
        .expect("parse should succeed");

        let Some(Subcommand::Exec(exec)) = cli.subcommand else {
            panic!("expected exec subcommand");
        };
        let Some(codex_exec::Command::Resume(args)) = exec.command else {
            panic!("expected exec resume");
        };

        assert_eq!(
            exec.last_message_file,
            Some(std::path::PathBuf::from("/tmp/resume-output.md"))
        );
        assert_eq!(args.session_id.as_deref(), Some("session-123"));
        assert_eq!(args.prompt.as_deref(), Some("re-review"));
    }

    #[test]
    fn dangerous_bypass_conflicts_with_approval_policy() {
        let err = MultitoolCli::try_parse_from([
            "codex",
            "--dangerously-bypass-approvals-and-sandbox",
            "--ask-for-approval",
            "on-request",
        ])
        .expect_err("conflicting permission flags should be rejected");

        assert_eq!(err.kind(), clap::error::ErrorKind::ArgumentConflict);
    }

    fn app_server_from_args(args: &[&str]) -> AppServerCommand {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let Subcommand::AppServer(app_server) = cli.subcommand.expect("app-server present") else {
            unreachable!()
        };
        app_server
    }

    fn default_app_server_socket_path() -> AbsolutePathBuf {
        let codex_home = find_codex_home().expect("codex home");
        codex_app_server::app_server_control_socket_path(&codex_home)
            .expect("default app-server socket path")
    }

    #[test]
    fn debug_prompt_input_parses_prompt_and_images() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "debug",
            "prompt-input",
            "hello",
            "--image",
            "/tmp/a.png,/tmp/b.png",
        ])
        .expect("parse");

        let Some(Subcommand::Debug(DebugCommand {
            subcommand: DebugSubcommand::PromptInput(cmd),
        })) = cli.subcommand
        else {
            panic!("expected debug prompt-input subcommand");
        };

        assert_eq!(cmd.prompt.as_deref(), Some("hello"));
        assert_eq!(
            cmd.images,
            vec![PathBuf::from("/tmp/a.png"), PathBuf::from("/tmp/b.png")]
        );
    }

    #[test]
    fn debug_models_parses_bundled_flag() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "debug", "models", "--bundled"]).expect("parse");

        let Some(Subcommand::Debug(DebugCommand {
            subcommand: DebugSubcommand::Models(cmd),
        })) = cli.subcommand
        else {
            panic!("expected debug models subcommand");
        };

        assert!(cmd.bundled);
    }

    #[test]
    fn responses_subcommand_is_hidden_from_help_but_parses() {
        let help = MultitoolCli::command().render_help().to_string();
        assert!(!help.contains("responses"));

        let cli = MultitoolCli::try_parse_from(["codex", "responses"]).expect("parse");
        assert!(matches!(cli.subcommand, Some(Subcommand::Responses(_))));
    }

    fn help_from_args(args: &[&str]) -> String {
        let err = MultitoolCli::try_parse_from(args).expect_err("help should short-circuit");
        assert_eq!(err.kind(), clap::error::ErrorKind::DisplayHelp);
        err.to_string()
    }

    #[test]
    fn plugin_marketplace_help_uses_plugin_namespace() {
        let help = help_from_args(&["codex", "plugin", "marketplace", "--help"]);
        assert!(
            help.contains("Usage: codex plugin marketplace [OPTIONS] <COMMAND>"),
            "{help}"
        );

        for (subcommand, usage) in [
            ("add", "Usage: codex plugin marketplace add"),
            ("upgrade", "Usage: codex plugin marketplace upgrade"),
            ("remove", "Usage: codex plugin marketplace remove"),
        ] {
            let help = help_from_args(&["codex", "plugin", "marketplace", subcommand, "--help"]);
            assert!(help.contains(usage), "{help}");
        }
    }

    #[test]
    fn plugin_marketplace_add_parses_under_plugin() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "plugin", "marketplace", "add", "owner/repo"])
                .expect("parse");

        assert!(matches!(cli.subcommand, Some(Subcommand::Plugin(_))));
    }

    #[test]
    fn plugin_marketplace_upgrade_parses_under_plugin() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "plugin", "marketplace", "upgrade", "debug"])
                .expect("parse");

        assert!(matches!(cli.subcommand, Some(Subcommand::Plugin(_))));
    }

    #[test]
    fn plugin_marketplace_remove_parses_under_plugin() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "plugin", "marketplace", "remove", "debug"])
                .expect("parse");

        assert!(matches!(cli.subcommand, Some(Subcommand::Plugin(_))));
    }

    #[test]
    fn marketplace_no_longer_parses_at_top_level() {
        let add_result =
            MultitoolCli::try_parse_from(["codex", "marketplace", "add", "owner/repo"]);
        assert!(add_result.is_err());

        let upgrade_result =
            MultitoolCli::try_parse_from(["codex", "marketplace", "upgrade", "debug"]);
        assert!(upgrade_result.is_err());

        let remove_result =
            MultitoolCli::try_parse_from(["codex", "marketplace", "remove", "debug"]);
        assert!(remove_result.is_err());
    }

    fn sample_exit_info(conversation_id: Option<&str>, thread_name: Option<&str>) -> AppExitInfo {
        let token_usage = TokenUsage {
            output_tokens: 2,
            total_tokens: 2,
            ..Default::default()
        };
        AppExitInfo {
            token_usage,
            thread_id: conversation_id
                .map(ThreadId::from_string)
                .map(Result::unwrap),
            thread_name: thread_name.map(str::to_string),
            update_action: None,
            exit_reason: ExitReason::UserRequested,
        }
    }

    #[test]
    fn format_exit_messages_skips_zero_usage() {
        let exit_info = AppExitInfo {
            token_usage: TokenUsage::default(),
            thread_id: None,
            thread_name: None,
            update_action: None,
            exit_reason: ExitReason::UserRequested,
        };
        let lines = format_exit_messages(exit_info, /*color_enabled*/ false);
        assert!(lines.is_empty());
    }

    #[test]
    fn format_exit_messages_includes_resume_hint_without_color() {
        let exit_info = sample_exit_info(
            Some("123e4567-e89b-12d3-a456-426614174000"),
            /*thread_name*/ None,
        );
        let lines = format_exit_messages(exit_info, /*color_enabled*/ false);
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
        let exit_info = sample_exit_info(
            Some("123e4567-e89b-12d3-a456-426614174000"),
            /*thread_name*/ None,
        );
        let lines = format_exit_messages(exit_info, /*color_enabled*/ true);
        assert_eq!(lines.len(), 2);
        assert!(lines[1].contains("\u{1b}[36m"));
    }

    #[test]
    fn format_exit_messages_uses_id_even_when_thread_has_name() {
        let exit_info = sample_exit_info(
            Some("123e4567-e89b-12d3-a456-426614174000"),
            Some("my-thread"),
        );
        let lines = format_exit_messages(exit_info, /*color_enabled*/ false);
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
    fn resume_model_flag_applies_when_no_root_flags() {
        let interactive =
            finalize_resume_from_args(["codex", "resume", "-m", "gpt-5.1-test"].as_ref());

        assert_eq!(interactive.model.as_deref(), Some("gpt-5.1-test"));
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn resume_picker_logic_none_and_not_last() {
        let interactive = finalize_resume_from_args(["codex", "resume"].as_ref());
        assert!(interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_last() {
        let interactive = finalize_resume_from_args(["codex", "resume", "--last"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_picker_logic_with_session_id() {
        let interactive = finalize_resume_from_args(["codex", "resume", "1234"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("1234"));
        assert!(!interactive.resume_show_all);
    }

    #[test]
    fn resume_all_flag_sets_show_all() {
        let interactive = finalize_resume_from_args(["codex", "resume", "--all"].as_ref());
        assert!(interactive.resume_picker);
        assert!(interactive.resume_show_all);
    }

    #[test]
    fn resume_include_non_interactive_flag_sets_source_filter_override() {
        let interactive =
            finalize_resume_from_args(["codex", "resume", "--include-non-interactive"].as_ref());

        assert!(interactive.resume_picker);
        assert!(interactive.resume_include_non_interactive);
    }

    #[test]
    fn resume_merges_option_flags_and_full_auto() {
        let interactive = finalize_resume_from_args(
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
            Some(codex_utils_cli::SandboxModeCliArg::WorkspaceWrite)
        );
        assert_matches!(
            interactive.approval_policy,
            Some(codex_utils_cli::ApprovalModeCliArg::OnRequest)
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
        let interactive = finalize_resume_from_args(
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
    fn fork_picker_logic_none_and_not_last() {
        let interactive = finalize_fork_from_args(["codex", "fork"].as_ref());
        assert!(interactive.fork_picker);
        assert!(!interactive.fork_last);
        assert_eq!(interactive.fork_session_id, None);
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_picker_logic_last() {
        let interactive = finalize_fork_from_args(["codex", "fork", "--last"].as_ref());
        assert!(!interactive.fork_picker);
        assert!(interactive.fork_last);
        assert_eq!(interactive.fork_session_id, None);
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_picker_logic_with_session_id() {
        let interactive = finalize_fork_from_args(["codex", "fork", "1234"].as_ref());
        assert!(!interactive.fork_picker);
        assert!(!interactive.fork_last);
        assert_eq!(interactive.fork_session_id.as_deref(), Some("1234"));
        assert!(!interactive.fork_show_all);
    }

    #[test]
    fn fork_all_flag_sets_show_all() {
        let interactive = finalize_fork_from_args(["codex", "fork", "--all"].as_ref());
        assert!(interactive.fork_picker);
        assert!(interactive.fork_show_all);
    }

    #[test]
    fn app_server_analytics_default_disabled_without_flag() {
        let app_server = app_server_from_args(["codex", "app-server"].as_ref());
        assert!(!app_server.analytics_default_enabled);
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::Stdio
        );
    }

    #[test]
    fn app_server_analytics_default_enabled_with_flag() {
        let app_server =
            app_server_from_args(["codex", "app-server", "--analytics-default-enabled"].as_ref());
        assert!(app_server.analytics_default_enabled);
    }

    #[test]
    fn remote_flag_parses_for_interactive_root() {
        let cli = MultitoolCli::try_parse_from(["codex", "--remote", "ws://127.0.0.1:4500"])
            .expect("parse");
        assert_eq!(cli.remote.remote.as_deref(), Some("ws://127.0.0.1:4500"));
    }

    #[test]
    fn remote_auth_token_env_flag_parses_for_interactive_root() {
        let cli = MultitoolCli::try_parse_from([
            "codex",
            "--remote-auth-token-env",
            "CODEX_REMOTE_AUTH_TOKEN",
            "--remote",
            "ws://127.0.0.1:4500",
        ])
        .expect("parse");
        assert_eq!(
            cli.remote.remote_auth_token_env.as_deref(),
            Some("CODEX_REMOTE_AUTH_TOKEN")
        );
    }

    #[test]
    fn remote_flag_parses_for_resume_subcommand() {
        let cli =
            MultitoolCli::try_parse_from(["codex", "resume", "--remote", "ws://127.0.0.1:4500"])
                .expect("parse");
        let Subcommand::Resume(ResumeCommand { remote, .. }) =
            cli.subcommand.expect("resume present")
        else {
            panic!("expected resume subcommand");
        };
        assert_eq!(remote.remote.as_deref(), Some("ws://127.0.0.1:4500"));
    }

    #[test]
    fn reject_remote_mode_for_non_interactive_subcommands() {
        let err = reject_remote_mode_for_subcommand(
            Some("127.0.0.1:4500"),
            /*remote_auth_token_env*/ None,
            "exec",
        )
        .expect_err("non-interactive subcommands should reject --remote");
        assert!(
            err.to_string()
                .contains("only supported for interactive TUI commands")
        );
    }

    #[test]
    fn reject_remote_auth_token_env_for_non_interactive_subcommands() {
        let err = reject_remote_mode_for_subcommand(
            /*remote*/ None,
            Some("CODEX_REMOTE_AUTH_TOKEN"),
            "exec",
        )
        .expect_err("non-interactive subcommands should reject --remote-auth-token-env");
        assert!(
            err.to_string()
                .contains("only supported for interactive TUI commands")
        );
    }

    #[test]
    fn reject_remote_auth_token_env_for_app_server_generate_internal_json_schema() {
        let subcommand =
            AppServerSubcommand::GenerateInternalJsonSchema(GenerateInternalJsonSchemaCommand {
                out_dir: PathBuf::from("/tmp/out"),
            });
        let err = reject_remote_mode_for_app_server_subcommand(
            /*remote*/ None,
            Some("CODEX_REMOTE_AUTH_TOKEN"),
            Some(&subcommand),
        )
        .expect_err("non-interactive app-server subcommands should reject --remote-auth-token-env");
        assert!(err.to_string().contains("generate-internal-json-schema"));
    }

    #[test]
    fn read_remote_auth_token_from_env_var_reports_missing_values() {
        let err = read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
            Err(std::env::VarError::NotPresent)
        })
        .expect_err("missing env vars should be rejected");
        assert!(err.to_string().contains("is not set"));
    }

    #[test]
    fn read_remote_auth_token_from_env_var_trims_values() {
        let auth_token =
            read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
                Ok("  bearer-token  ".to_string())
            })
            .expect("env var should parse");
        assert_eq!(auth_token, "bearer-token");
    }

    #[test]
    fn read_remote_auth_token_from_env_var_rejects_empty_values() {
        let err = read_remote_auth_token_from_env_var_with("CODEX_REMOTE_AUTH_TOKEN", |_| {
            Ok(" \n\t ".to_string())
        })
        .expect_err("empty env vars should be rejected");
        assert!(err.to_string().contains("is empty"));
    }

    #[test]
    fn app_server_listen_websocket_url_parses() {
        let app_server = app_server_from_args(
            ["codex", "app-server", "--listen", "ws://127.0.0.1:4500"].as_ref(),
        );
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::WebSocket {
                bind_address: "127.0.0.1:4500".parse().expect("valid socket address"),
            }
        );
    }

    #[test]
    fn app_server_listen_stdio_url_parses() {
        let app_server =
            app_server_from_args(["codex", "app-server", "--listen", "stdio://"].as_ref());
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::Stdio
        );
    }

    #[test]
    fn app_server_listen_unix_socket_url_parses() {
        let app_server =
            app_server_from_args(["codex", "app-server", "--listen", "unix://"].as_ref());
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::UnixSocket {
                socket_path: default_app_server_socket_path()
            }
        );
    }

    #[test]
    fn app_server_listen_unix_socket_path_parses() {
        let app_server = app_server_from_args(
            ["codex", "app-server", "--listen", "unix:///tmp/codex.sock"].as_ref(),
        );
        assert_eq!(
            app_server.listen,
            codex_app_server::AppServerTransport::UnixSocket {
                socket_path: AbsolutePathBuf::from_absolute_path("/tmp/codex.sock")
                    .expect("absolute path should parse")
            }
        );
    }

    #[test]
    fn app_server_listen_off_parses() {
        let app_server = app_server_from_args(["codex", "app-server", "--listen", "off"].as_ref());
        assert_eq!(app_server.listen, codex_app_server::AppServerTransport::Off);
    }

    #[test]
    fn app_server_listen_invalid_url_fails_to_parse() {
        let parse_result =
            MultitoolCli::try_parse_from(["codex", "app-server", "--listen", "http://foo"]);
        assert!(parse_result.is_err());
    }

    #[test]
    fn app_server_proxy_subcommand_parses() {
        let app_server = app_server_from_args(["codex", "app-server", "proxy"].as_ref());
        assert!(matches!(
            app_server.subcommand,
            Some(AppServerSubcommand::Proxy(AppServerProxyCommand {
                socket_path: None
            }))
        ));
    }

    #[test]
    fn app_server_proxy_sock_path_parses() {
        let app_server =
            app_server_from_args(["codex", "app-server", "proxy", "--sock", "codex.sock"].as_ref());
        let Some(AppServerSubcommand::Proxy(proxy)) = app_server.subcommand else {
            panic!("expected proxy subcommand");
        };
        assert_eq!(
            proxy.socket_path,
            Some(
                AbsolutePathBuf::relative_to_current_dir("codex.sock")
                    .expect("relative path should resolve")
            )
        );
    }

    #[test]
    fn reject_remote_auth_token_env_for_app_server_proxy() {
        let subcommand = AppServerSubcommand::Proxy(AppServerProxyCommand { socket_path: None });
        let err = reject_remote_mode_for_app_server_subcommand(
            /*remote*/ None,
            Some("CODEX_REMOTE_AUTH_TOKEN"),
            Some(&subcommand),
        )
        .expect_err("app-server proxy should reject --remote-auth-token-env");
        assert!(err.to_string().contains("app-server proxy"));
    }

    #[test]
    fn app_server_capability_token_flags_parse() {
        let app_server = app_server_from_args(
            [
                "codex",
                "app-server",
                "--ws-auth",
                "capability-token",
                "--ws-token-file",
                "/tmp/codex-token",
            ]
            .as_ref(),
        );
        assert_eq!(
            app_server.auth.ws_auth,
            Some(codex_app_server::WebsocketAuthCliMode::CapabilityToken)
        );
        assert_eq!(
            app_server.auth.ws_token_file,
            Some(PathBuf::from("/tmp/codex-token"))
        );
    }

    #[test]
    fn app_server_signed_bearer_flags_parse() {
        let app_server = app_server_from_args(
            [
                "codex",
                "app-server",
                "--ws-auth",
                "signed-bearer-token",
                "--ws-shared-secret-file",
                "/tmp/codex-secret",
                "--ws-issuer",
                "issuer",
                "--ws-audience",
                "audience",
                "--ws-max-clock-skew-seconds",
                "9",
            ]
            .as_ref(),
        );
        assert_eq!(
            app_server.auth.ws_auth,
            Some(codex_app_server::WebsocketAuthCliMode::SignedBearerToken)
        );
        assert_eq!(
            app_server.auth.ws_shared_secret_file,
            Some(PathBuf::from("/tmp/codex-secret"))
        );
        assert_eq!(app_server.auth.ws_issuer.as_deref(), Some("issuer"));
        assert_eq!(app_server.auth.ws_audience.as_deref(), Some("audience"));
        assert_eq!(app_server.auth.ws_max_clock_skew_seconds, Some(9));
    }

    #[test]
    fn app_server_rejects_removed_insecure_non_loopback_flag() {
        let parse_result = MultitoolCli::try_parse_from([
            "codex",
            "app-server",
            "--allow-unauthenticated-non-loopback-ws",
        ]);
        assert!(parse_result.is_err());
    }

    #[test]
    fn features_enable_parses_feature_name() {
        let cli = MultitoolCli::try_parse_from(["codex", "features", "enable", "unified_exec"])
            .expect("parse should succeed");
        let Some(Subcommand::Features(FeaturesCli { sub })) = cli.subcommand else {
            panic!("expected features subcommand");
        };
        let FeaturesSubcommand::Enable(FeatureSetArgs { feature }) = sub else {
            panic!("expected features enable");
        };
        assert_eq!(feature, "unified_exec");
    }

    #[test]
    fn features_disable_parses_feature_name() {
        let cli = MultitoolCli::try_parse_from(["codex", "features", "disable", "shell_tool"])
            .expect("parse should succeed");
        let Some(Subcommand::Features(FeaturesCli { sub })) = cli.subcommand else {
            panic!("expected features subcommand");
        };
        let FeaturesSubcommand::Disable(FeatureSetArgs { feature }) = sub else {
            panic!("expected features disable");
        };
        assert_eq!(feature, "shell_tool");
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
    fn feature_toggles_accept_legacy_linux_sandbox_flag() {
        let toggles = FeatureToggles {
            enable: vec!["use_linux_sandbox_bwrap".to_string()],
            disable: Vec::new(),
        };
        let overrides = toggles.to_overrides().expect("valid features");
        assert_eq!(
            overrides,
            vec!["features.use_linux_sandbox_bwrap=true".to_string(),]
        );
    }

    #[test]
    fn feature_toggles_accept_removed_image_detail_original_flag() {
        let toggles = FeatureToggles {
            enable: vec!["image_detail_original".to_string()],
            disable: Vec::new(),
        };
        let overrides = toggles.to_overrides().expect("valid features");
        assert_eq!(
            overrides,
            vec!["features.image_detail_original=true".to_string(),]
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
