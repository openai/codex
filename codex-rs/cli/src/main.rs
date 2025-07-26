use clap::CommandFactory;
use clap::Parser;
use clap_complete::Shell;
use clap_complete::generate;
use codex_chatgpt::apply_command::ApplyCommand;
use codex_chatgpt::apply_command::run_apply_command;
use codex_cli::concurrent::maybe_spawn_concurrent;
use codex_cli::LandlockCommand;
use codex_cli::SeatbeltCommand;
use codex_cli::login::run_login_with_chatgpt;
use codex_cli::proto;
use codex_common::CliConfigOverrides;
use codex_exec::Cli as ExecCli;
use codex_tui::Cli as TuiCli;
use std::path::PathBuf;

use crate::proto::ProtoCli;
use codex_mcp_client::McpClient;
use mcp_types::{ClientCapabilities, Implementation};
use serde_json::json;
use std::time::Duration;
use tracing::{debug, info};

/// Codex CLI
///
/// If no subcommand is specified, options will be forwarded to the interactive CLI.
#[derive(Debug, Parser)]
#[clap(
    author,
    version,
    // If a sub‑command is given, ignore requirements of the default args.
    subcommand_negates_reqs = true
)]
struct MultitoolCli {
    #[clap(flatten)]
    pub config_overrides: CliConfigOverrides,

    #[clap(flatten)]
    interactive: TuiCli,

    /// Autonomous mode: run the command in the background & concurrently using a git worktree.
    /// Requires the current directory (or --cd provided path) to be a git repository.
    #[clap(long)]
    concurrent: bool,

    /// Control whether the concurrent run auto-merges the worktree branch back into the original branch.
    /// Defaults to true (may also be set via CONCURRENT_AUTOMERGE env var).
    #[clap(long = "concurrent-automerge", value_name = "BOOL")]
    concurrent_automerge: Option<bool>,

    /// Explicit branch name to use for the concurrent worktree instead of the default `codex/<slug>`.
    /// May also be set via CONCURRENT_BRANCH_NAME env var.
    #[clap(long = "concurrent-branch-name", value_name = "BRANCH")]
    concurrent_branch_name: Option<String>,

    /// Best-of-n: run n concurrent worktrees (1-4) and let user pick the best result. Implies --concurrent and disables automerge.
    #[clap(long = "best-of-n", short = 'n', value_name = "N", default_value_t = 1)]
    pub best_of_n: u8,

    #[clap(subcommand)]
    subcommand: Option<Subcommand>,
}

#[derive(Debug, clap::Subcommand)]
enum Subcommand {
    /// Run Codex non-interactively.
    #[clap(visible_alias = "e")]
    Exec(ExecCli),

    /// Login with ChatGPT.
    Login(LoginCommand),

    /// Experimental: run Codex as an MCP server.
    Mcp,

    /// Run the Protocol stream via stdin/stdout
    #[clap(visible_alias = "p")]
    Proto(ProtoCli),

    /// Generate shell completion scripts.
    Completion(CompletionCommand),

    /// Internal debugging commands.
    Debug(DebugArgs),

    /// Apply the latest diff produced by Codex agent as a `git apply` to your local working tree.
    #[clap(visible_alias = "a")]
    Apply(ApplyCommand),

    /// Manage / inspect concurrent background tasks.
    Tasks(codex_cli::tasks::TasksCli),

    /// Show or follow logs for a specific task.
    Logs(codex_cli::logs::LogsCli),

    /// Inspect full metadata for a task.
    Inspect(codex_cli::inspect::InspectCli),

    /// Hidden: internal worker used for --concurrent MCP-based runs.
    #[clap(hide = true)]
    Worker(ConcurrentWorkerCli),
}

#[derive(Debug, Parser)]
struct CompletionCommand {
    /// Shell to generate completions for
    #[clap(value_enum, default_value_t = Shell::Bash)]
    shell: Shell,
}

#[derive(Debug, Parser)]
struct DebugArgs {
    #[command(subcommand)]
    cmd: DebugCommand,
}

#[derive(Debug, clap::Subcommand)]
enum DebugCommand {
    /// Run a command under Seatbelt (macOS only).
    Seatbelt(SeatbeltCommand),

    /// Run a command under Landlock+seccomp (Linux only).
    Landlock(LandlockCommand),
}

#[derive(Debug, Parser)]
struct LoginCommand {
    #[clap(skip)]
    config_overrides: CliConfigOverrides,
}

#[derive(Debug, Parser)]
struct ConcurrentWorkerCli {
    #[clap(long)]
    prompt: String,
    #[clap(long)]
    model: Option<String>,
    #[clap(long)]
    profile: Option<String>,
    #[clap(long, value_name = "POLICY")] // untrusted | on-failure | never
    approval_policy: Option<String>,
    #[clap(long, value_name = "MODE")] // read-only | workspace-write | danger-full-access
    sandbox: Option<String>,
    #[clap(long)]
    cwd: Option<String>,
    #[clap(flatten)]
    config_overrides: CliConfigOverrides,
    /// Optional base instructions override
    #[clap(long = "base-instructions")]
    base_instructions: Option<String>,
}

fn main() -> anyhow::Result<()> {
    codex_linux_sandbox::run_with_sandbox(|codex_linux_sandbox_exe| async move {
        cli_main(codex_linux_sandbox_exe).await?;
        Ok(())
    })
}

async fn cli_main(codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let cli = MultitoolCli::parse();

    match cli.subcommand {
        None => {
            let mut tui_cli = cli.interactive;
            let root_raw_overrides = cli.config_overrides.raw_overrides.clone();
            prepend_config_flags(&mut tui_cli.config_overrides, cli.config_overrides);
            // Attempt concurrent background spawn; if it returns true we skip launching the TUI.
            if let Ok(spawned) = maybe_spawn_concurrent(
                &mut tui_cli,
                &root_raw_overrides,
                cli.concurrent,
                cli.concurrent_automerge,
                &cli.concurrent_branch_name,
            ) {
                if !spawned {
                    let usage = codex_tui::run_main(tui_cli, codex_linux_sandbox_exe)?;
                    println!("{}", codex_core::protocol::FinalOutput::from(usage));
                }
            } else {
                // On error fallback to interactive.
                let usage = codex_tui::run_main(tui_cli, codex_linux_sandbox_exe)?;
                println!("{}", codex_core::protocol::FinalOutput::from(usage));
            }
        }
        Some(Subcommand::Exec(mut exec_cli)) => {
            prepend_config_flags(&mut exec_cli.config_overrides, cli.config_overrides);
            codex_exec::run_main(exec_cli, codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Mcp) => {
            codex_mcp_server::run_main(codex_linux_sandbox_exe).await?;
        }
        Some(Subcommand::Worker(worker_cli)) => {
            // Internal worker invoked by maybe_spawn_concurrent. Runs a single Codex MCP tool-call.
            debug!(?worker_cli.prompt, "starting concurrent worker");
            // Build MCP client by spawning current binary with `mcp` subcommand.
            let exe = std::env::current_exe()?;
            let exe_str = exe.to_string_lossy().to_string();
            // Pass through OPENAI_API_KEY (and related) so MCP server can access the model provider.
            let mut extra_env: std::collections::HashMap<String, String> = std::collections::HashMap::new();
            // TODO: pap check if this is needed + check if we can use the same env vars as the main process (overall)
            if let Ok(v) = std::env::var("OPENAI_API_KEY") { extra_env.insert("OPENAI_API_KEY".into(), v); }
            if let Ok(v) = std::env::var("OPENAI_BASE_URL") { extra_env.insert("OPENAI_BASE_URL".into(), v); }
            let client = McpClient::new_stdio_client(exe_str, vec!["mcp".to_string()], Some(extra_env)).await?;
            // Initialize MCP session.
            let init_params = mcp_types::InitializeRequestParams {
                capabilities: ClientCapabilities { experimental: None, roots: None, sampling: None, elicitation: Some(json!({})) },
                client_info: Implementation { name: "codex-concurrent-worker".into(), version: env!("CARGO_PKG_VERSION").into(), title: Some("Codex Concurrent Worker".into()) },
                protocol_version: mcp_types::MCP_SCHEMA_VERSION.to_string(),
            };
            let _init_res = client.initialize(init_params, None, Some(Duration::from_secs(15))).await?;
            debug!("initialized MCP session for worker");
            // Build arguments for codex tool call using kebab-case keys expected by MCP server.
            let mut arg_obj = serde_json::Map::new();
            // todo: how to pass all variables dynamically?
            arg_obj.insert("prompt".to_string(), worker_cli.prompt.clone().into());
            if let Some(m) = worker_cli.model.clone() { arg_obj.insert("model".into(), m.into()); }
            if let Some(p) = worker_cli.profile.clone() { arg_obj.insert("profile".into(), p.into()); }
            if let Some(ap) = worker_cli.approval_policy.clone() { arg_obj.insert("approval-policy".into(), ap.into()); }
            if let Some(sb) = worker_cli.sandbox.clone() { arg_obj.insert("sandbox".into(), sb.into()); }
            if let Some(cwd) = worker_cli.cwd.clone() { arg_obj.insert("cwd".into(), cwd.into()); }
            if let Some(bi) = worker_cli.base_instructions.clone() { arg_obj.insert("base-instructions".into(), bi.into()); }
            let config_json = serde_json::to_value(&worker_cli.config_overrides)?;
            arg_obj.insert("config".into(), config_json);
            let args_json = serde_json::Value::Object(arg_obj);
            debug!(?args_json, "calling codex tool via MCP");
            let mut session_id: Option<String> = None;
            // Grab notifications receiver to watch for SessionConfigured (to extract sessionId) while first tool call runs.
            let mut notif_rx = client.take_notification_receiver().await;
            // Spawn a task to extract sessionId and print filtered events.
            if let Some(mut rx) = notif_rx.take() {
                tokio::spawn(async move {
                    use serde_json::Value;
                    while let Some(n) = rx.recv().await {
                        if let Some(p) = &n.params {
                            if let Some(root) = p.as_object() {
                                if let Some(val) = root.get("sessionId").or_else(|| root.get("session_id")) {
                                    // todo: reuse session id as task id
                                    if let Some(s) = val.as_str() { if !s.is_empty() { println!("SESSION ID: {}", s); } }
                                }
                                if let Some(Value::Object(msg)) = root.get("msg") {
                                    if let Some(Value::String(typ)) = msg.get("type") {
                                        if typ.ends_with("_delta") { continue; }
                                        // todo: use the tui once it manages multi processes
                                        match typ.as_str() {
                                            "agent_reasoning" => {
                                                if let Some(Value::String(text)) = msg.get("text") { println!("\x1b[36mreasoning:\x1b[0m {}", text); }
                                            }
                                            "exec_approval_request" => {
                                                let cmd = msg.get("command").and_then(|v| v.as_array()).map(|arr| arr.iter().filter_map(|x| x.as_str()).collect::<Vec<_>>().join(" ")).unwrap_or_default();
                                                let cwd = msg.get("cwd").and_then(|v| v.as_str()).unwrap_or("");
                                                println!("\x1b[33mexec approval requested:\x1b[0m {cmd} (cwd: {cwd})");
                                            }
                                            "apply_patch_approval_request" => {
                                                let reason = msg.get("reason").and_then(|v| v.as_str()).unwrap_or("");
                                                println!("\x1b[35mpatch approval requested:\x1b[0m {reason}");
                                            }
                                            "task_complete" => {
                                                println!("\x1b[32mtask complete\x1b[0m");
                                            }
                                            _ => { /* suppress other event types */ }
                                        }
                                    }
                                }
                            }
                        }
                    }
                });
            }
            let first_result = client.call_tool("codex".to_string(), Some(args_json), None).await;
            // todo: to test we have not implemented the tool call yet
            match &first_result {
                Ok(r) => debug!(blocks = r.content.len(), "codex initial tool call completed"),
                Err(e) => debug!(error = %e, "codex tool call failed"),
            }
            let first_result = first_result?;
            // Print any text content to stdout.
            let mut printed_any = false;
            for block in &first_result.content {
                if let mcp_types::ContentBlock::TextContent(t) = block { println!("{}", t.text); printed_any = true; }
            }
            if !printed_any { info!("no text content blocks returned from initial codex tool call"); }
            // Attempt to parse session id from printed notifications (fallback approach): scan stdout not feasible here; so rely on user-visible marker.
            // Interactive loop for follow-up prompts.
            use std::io::{stdin, stdout, Write};
            loop {
                print!("codex> "); let _ = stdout().flush();
                let mut line = String::new();
                if stdin().read_line(&mut line).is_err() { break; }
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed == "/exit" || trimmed == ":q" { break; }
                // If session id still unknown, ask user to paste it.
                if session_id.is_none() {
                    if trimmed.starts_with("session ") { session_id = Some(trimmed[8..].trim().to_string()); println!("Stored session id."); continue; }
                }
                if session_id.is_none() { println!("(Need session id; when you see 'SESSION ID: <uuid>' above, copy it or type 'session <uuid>')"); continue; }
                let args = serde_json::json!({ "sessionId": session_id.clone().unwrap(), "prompt": trimmed });
                let reply = client.call_tool("codex-reply".to_string(), Some(args), None).await;
                match reply {
                    Ok(r) => {
                        for block in r.content {
                            if let mcp_types::ContentBlock::TextContent(t) = block { println!("{}", t.text); }
                        }
                    }
                    Err(e) => println!("Error: {e}"),
                }
            }
            // Append completion record to tasks.jsonl now that interactive loop ends.
            if let Ok(task_id) = std::env::var("CODEX_TASK_ID") {
                if let Some(base) = codex_base_dir_for_worker() {
                    let tasks_path = base.join("tasks.jsonl");
                    let ts = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
                    let obj = serde_json::json!({
                        "task_id": task_id,
                        "completion_time": ts,
                        "end_time": ts,
                        "state": "done",
                    });
                    let _ = append_json_line(&tasks_path, &obj);
                }
            }
        }
        Some(Subcommand::Login(mut login_cli)) => {
            prepend_config_flags(&mut login_cli.config_overrides, cli.config_overrides);
            run_login_with_chatgpt(login_cli.config_overrides).await;
        }
        Some(Subcommand::Proto(mut proto_cli)) => {
            prepend_config_flags(&mut proto_cli.config_overrides, cli.config_overrides);
            proto::run_main(proto_cli).await?;
        }
        Some(Subcommand::Completion(completion_cli)) => {
            print_completion(completion_cli);
        }
        Some(Subcommand::Debug(debug_args)) => match debug_args.cmd {
            DebugCommand::Seatbelt(mut seatbelt_cli) => {
                prepend_config_flags(&mut seatbelt_cli.config_overrides, cli.config_overrides);
                codex_cli::debug_sandbox::run_command_under_seatbelt(
                    seatbelt_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
            DebugCommand::Landlock(mut landlock_cli) => {
                prepend_config_flags(&mut landlock_cli.config_overrides, cli.config_overrides);
                codex_cli::debug_sandbox::run_command_under_landlock(
                    landlock_cli,
                    codex_linux_sandbox_exe,
                )
                .await?;
            }
        },
        Some(Subcommand::Apply(mut apply_cli)) => {
            prepend_config_flags(&mut apply_cli.config_overrides, cli.config_overrides);
            run_apply_command(apply_cli, None).await?;
        }
        Some(Subcommand::Tasks(tasks_cli)) => {
            codex_cli::tasks::run_tasks(tasks_cli)?;
        }
        Some(Subcommand::Logs(logs_cli)) => {
            codex_cli::logs::run_logs(logs_cli)?;
        }
        Some(Subcommand::Inspect(inspect_cli)) => {
            codex_cli::inspect::run_inspect(inspect_cli)?;
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

fn print_completion(cmd: CompletionCommand) {
    let mut app = MultitoolCli::command();
    let name = "codex";
    generate(cmd.shell, &mut app, name, &mut std::io::stdout());
}

// Helper functions for worker
fn codex_base_dir_for_worker() -> Option<std::path::PathBuf> {
    if let Ok(val) = std::env::var("CODEX_HOME") { if !val.is_empty() { return std::fs::canonicalize(val).ok(); } }
    let home = std::env::var_os("HOME")?;
    let base = std::path::PathBuf::from(home).join(".codex");
    let _ = std::fs::create_dir_all(&base);
    Some(base)
}

fn append_json_line(path: &std::path::PathBuf, val: &serde_json::Value) -> std::io::Result<()> {
    use std::io::Write;
    let mut f = std::fs::OpenOptions::new().create(true).append(true).open(path)?;
    writeln!(f, "{}", val.to_string())
}
