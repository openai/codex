use clap::CommandFactory;
use clap::Parser;
use clap_complete::generate;
use clap_complete::Shell;
use codex_arg0::arg0_dispatch_or_else;
use codex_chatgpt::apply_command::run_apply_command;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_cli::login::read_api_key_from_stdin;
use codex_cli::login::run_login_status;
use codex_cli::login::run_login_with_api_key;
use codex_cli::login::run_login_with_chatgpt;
use codex_cli::login::run_login_with_device_code;
use codex_cli::login::run_logout;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cloud_tasks::Cli as CloudTasksCli;
use codex_common::CliConfigOverrides;
use codex_exec::Cli as ExecCli;
use codex_responses_api_proxy::Args as ResponsesApiProxyArgs;
use codex_tui::AppExitInfo;
use codex_tui::Cli as TuiCli;
use owo_colors::OwoColorize;
use std::path::PathBuf;
use supports_color::Stream;

mod mcp_cmd;

use crate::mcp_cmd::McpCli;

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
    bin_name = "codex"
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

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

    /// Manage login.
    Login(LoginCommand),

    /// Remove stored authentication credentials.
    Logout(LogoutCommand),

    /// [experimental] Run Codex as an MCP server and manage MCP servers.
    Mcp(McpCli),

    /// [experimental] Run the Codex MCP server (stdio transport).
    McpServer,

    /// [experimental] Run the app server.
    AppServer,

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Run commands within a Codex-provided sandbox.
    #[clap(visible_alias = "debug")]
    Sandbox(SandboxArgs),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Resume a previous interactive session (picker by default; use --last to continue the most recent).
    Resume(ResumeCommand),

    /// Internal: generate TypeScript protocol bindings.
    #[clap(hide = true)]
    GenerateTs(GenerateTsCommand),
    /// [EXPERIMENTAL] Browse tasks from Codex Cloud and apply changes locally.
    #[clap(name = "cloud", alias = "cloud-tasks")]
    Cloud(CloudTasksCli),

    /// [EXPERIMENTAL] Delegate task to a sub-agent.
    Delegate(DelegateCommand),

    /// [EXPERIMENTAL] Delegate tasks to multiple agents in parallel
    #[clap(name = "delegate-parallel")]
    DelegateParallel(DelegateParallelCommand),

    /// [EXPERIMENTAL] Create and run a custom agent from a prompt
    #[clap(name = "agent-create")]
    AgentCreate(AgentCreateCommand),

    /// [EXPERIMENTAL] Conduct deep research on a topic.
    Research(ResearchCommand),

    /// Internal: run the responses API proxy.
    #[clap(hide = true)]
    ResponsesApiProxy(ResponsesApiProxyArgs),
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

    #[clap(flatten)]
    config_overrides: TuiCli,
}

#[derive(Debug, Parser)]
struct DelegateCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    /// Agent name to delegate to
    #[arg(value_name = "AGENT")]
    agent: String,

    /// Goal or task description
    #[arg(short, long, value_name = "GOAL")]
    goal: Option<String>,

    /// Scope path (files or directories)
    #[arg(long, value_name = "PATH")]
    scope: Option<PathBuf>,

    /// Token budget for the agent
    #[arg(long, value_name = "TOKENS")]
    budget: Option<usize>,

    /// Deadline in minutes
    #[arg(long, value_name = "MINUTES")]
    deadline: Option<u64>,

    /// Output file for the result
    #[arg(short, long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct DelegateParallelCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    /// Comma-separated agent names
    #[arg(value_name = "AGENTS", value_delimiter = ',')]
    agents: Vec<String>,

    /// Comma-separated goals (must match number of agents)
    #[arg(long, value_delimiter = ',')]
    goals: Vec<String>,

    /// Comma-separated scope paths (optional, must match number of agents if provided)
    #[arg(long, value_delimiter = ',')]
    scopes: Vec<PathBuf>,

    /// Comma-separated budgets (optional, must match number of agents if provided)
    #[arg(long, value_delimiter = ',')]
    budgets: Vec<usize>,

    /// Deadline in minutes (applies to all agents)
    #[arg(long, value_name = "MINUTES")]
    deadline: Option<u64>,

    /// Output file for combined results
    #[arg(short, long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct AgentCreateCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,

    /// Prompt describing the agent's purpose and tasks
    #[arg(value_name = "PROMPT")]
    prompt: String,

    /// Token budget for the custom agent
    #[arg(long, value_name = "TOKENS")]
    budget: Option<usize>,

    /// Save the generated agent definition to .codex/agents/
    #[arg(long, default_value = "false")]
    save: bool,

    /// Output file for the result
    #[arg(short, long, value_name = "FILE")]
    out: Option<PathBuf>,
}

#[derive(Debug, Parser)]
struct ResearchCommand {
    /// Topic to research
    #[arg(value_name = "TOPIC")]
    topic: String,

    /// Research depth (1-5)
    #[arg(short, long, value_name = "DEPTH", default_value = "3")]
    depth: u8,

    /// Search breadth (number of sources)
    #[arg(short, long, value_name = "BREADTH", default_value = "8")]
    breadth: u8,

    /// Token budget
    #[arg(long, value_name = "TOKENS", default_value = "60000")]
    budget: usize,

    /// Require citations
    #[arg(long, default_value = "true")]
    citations: bool,

    /// MCP tools to use (comma-separated)
    #[arg(long, value_name = "TOOLS")]
    mcp: Option<String>,

    /// Enable lightweight fallback
    #[arg(long, default_value = "false")]
    lightweight_fallback: bool,

    /// Use Gemini CLI with Google Search (requires gemini CLI and GOOGLE_API_KEY)
    #[arg(long, default_value = "false")]
    gemini: bool,

    /// Output file for the report
    #[arg(short, long, value_name = "FILE")]
    out: Option<PathBuf>,
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

    #[arg(long = "use-device-code")]
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
struct GenerateTsCommand {
    /// Output directory where .ts files will be written
    #[arg(short = 'o', long = "out", value_name = "DIR")]
    out_dir: PathBuf,

    /// Optional path to the Prettier executable to format generated files
    #[arg(short = 'p', long = "prettier", value_name = "PRETTIER_BIN")]
    prettier: Option<PathBuf>,
}

fn format_exit_messages(exit_info: AppExitInfo, color_enabled: bool) -> Vec<String> {
    let AppExitInfo {
        token_usage,
        conversation_id,
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

fn print_exit_messages(exit_info: AppExitInfo) {
    let color_enabled = supports_color::on(Stream::Stdout).is_some();
    for line in format_exit_messages(exit_info, color_enabled) {
        println!("{line}");
    }
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
        config_overrides: root_config_overrides,
        mut interactive,
        subcommand,
    } = MultitoolCli::parse();

    match subcommand {
        None => {
            prepend_config_flags(
                &mut interactive.config_overrides,
                root_config_overrides.clone(),
            );
            let exit_info = codex_tui::run_main(interactive, codex_linux_sandbox_exe).await?;
            print_exit_messages(exit_info);
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
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
        Some(Subcommand::AppServer) => {
            codex_app_server::run_main(codex_linux_sandbox_exe, root_config_overrides).await?;
        }
        Some(Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            config_overrides,
        })) => {
            interactive = finalize_resume_interactive(
                interactive,
                root_config_overrides.clone(),
                session_id,
                last,
                config_overrides,
            );
            let exit_info = codex_tui::run_main(interactive, codex_linux_sandbox_exe).await?;
            print_exit_messages(exit_info);
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
        Some(Subcommand::Delegate(mut delegate_cmd)) => {
            prepend_config_flags(
                &mut delegate_cmd.config_overrides,
                root_config_overrides.clone(),
            );
            codex_cli::delegate_cmd::run_delegate_command(
                delegate_cmd.config_overrides,
                delegate_cmd.agent,
                delegate_cmd.goal,
                delegate_cmd.scope,
                delegate_cmd.budget,
                delegate_cmd.deadline,
                delegate_cmd.out,
            )
            .await?;
        }
        Some(Subcommand::DelegateParallel(mut parallel_cmd)) => {
            prepend_config_flags(
                &mut parallel_cmd.config_overrides,
                root_config_overrides.clone(),
            );

            // clap handles value_delimiter, so we get Vec<T> directly
            let agents = parallel_cmd.agents;
            let goals = parallel_cmd.goals;

            // Convert Vec<PathBuf> to Vec<Option<PathBuf>>
            let scopes: Vec<Option<PathBuf>> = parallel_cmd.scopes.into_iter().map(Some).collect();

            // Convert Vec<usize> to Vec<Option<usize>>
            let budgets: Vec<Option<usize>> = parallel_cmd.budgets.into_iter().map(Some).collect();

            codex_cli::parallel_delegate_cmd::run_parallel_delegate_command(
                agents,
                goals,
                scopes,
                budgets,
                parallel_cmd.deadline,
                parallel_cmd.out,
                parallel_cmd.config_overrides,
            )
            .await?;
        }
        Some(Subcommand::AgentCreate(mut agent_create_cmd)) => {
            prepend_config_flags(
                &mut agent_create_cmd.config_overrides,
                root_config_overrides.clone(),
            );

            codex_cli::agent_create_cmd::run_agent_create_command(
                agent_create_cmd.prompt,
                agent_create_cmd.budget,
                agent_create_cmd.save,
                agent_create_cmd.out,
                agent_create_cmd.config_overrides,
            )
            .await?;
        }
        Some(Subcommand::Research(research_cmd)) => {
            codex_cli::research_cmd::run_research_command(
                research_cmd.topic,
                research_cmd.depth,
                research_cmd.breadth,
                research_cmd.budget,
                research_cmd.citations,
                research_cmd.mcp,
                research_cmd.lightweight_fallback,
                research_cmd.out,
                research_cmd.gemini,
            )
            .await?;
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
        Some(Subcommand::GenerateTs(gen_cli)) => {
            codex_protocol_ts::generate_ts(&gen_cli.out_dir, gen_cli.prettier.as_deref())?;
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

/// Build the final `TuiCli` for a `codex resume` invocation.
fn finalize_resume_interactive(
    mut interactive: TuiCli,
    root_config_overrides: CliConfigOverrides,
    session_id: Option<String>,
    last: bool,
    resume_cli: TuiCli,
) -> TuiCli {
    // Start with the parsed interactive CLI so resume shares the same
    // configuration surface area as `codex` without additional flags.
    let resume_session_id = session_id;
    interactive.resume_picker = resume_session_id.is_none() && !last;
    interactive.resume_last = last;
    interactive.resume_session_id = resume_session_id;

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

    fn finalize_from_args(args: &[&str]) -> TuiCli {
        let cli = MultitoolCli::try_parse_from(args).expect("parse");
        let MultitoolCli {
            interactive,
            config_overrides: root_overrides,
            subcommand,
        } = cli;

        let Subcommand::Resume(ResumeCommand {
            session_id,
            last,
            config_overrides: resume_cli,
        }) = subcommand.expect("resume present")
        else {
            unreachable!()
        };

        finalize_resume_interactive(interactive, root_overrides, session_id, last, resume_cli)
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
        }
    }

    #[test]
    fn format_exit_messages_skips_zero_usage() {
        let exit_info = AppExitInfo {
            token_usage: TokenUsage::default(),
            conversation_id: None,
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
        let interactive = finalize_from_args(["codex", "resume", "-m", "gpt-5-test"].as_ref());

        assert_eq!(interactive.model.as_deref(), Some("gpt-5-test"));
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
    }

    #[test]
    fn resume_picker_logic_last() {
        let interactive = finalize_from_args(["codex", "resume", "--last"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(interactive.resume_last);
        assert_eq!(interactive.resume_session_id, None);
    }

    #[test]
    fn resume_picker_logic_with_session_id() {
        let interactive = finalize_from_args(["codex", "resume", "1234"].as_ref());
        assert!(!interactive.resume_picker);
        assert!(!interactive.resume_last);
        assert_eq!(interactive.resume_session_id.as_deref(), Some("1234"));
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
                "gpt-5-test",
                "-p",
                "my-profile",
                "-C",
                "/tmp",
                "-i",
                "/tmp/a.png,/tmp/b.png",
            ]
            .as_ref(),
        );

        assert_eq!(interactive.model.as_deref(), Some("gpt-5-test"));
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
}
