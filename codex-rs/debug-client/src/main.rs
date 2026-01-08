mod client;
mod commands;
mod output;
mod reader;
mod state;

use std::io;
use std::io::BufRead;
use std::sync::mpsc;

use anyhow::Context;
use anyhow::Result;
use clap::ArgAction;
use clap::Parser;
use codex_app_server_protocol::AskForApproval;

use crate::client::AppServerClient;
use crate::client::build_thread_resume_params;
use crate::client::build_thread_start_params;
use crate::commands::InputAction;
use crate::commands::UserCommand;
use crate::commands::parse_input;
use crate::output::Output;
use crate::state::ReaderEvent;

#[derive(Parser)]
#[command(author = "Codex", version, about = "Minimal app-server client")]
struct Cli {
    /// Path to the `codex` CLI binary.
    #[arg(long, default_value = "codex")]
    codex_bin: String,

    /// Forwarded to the `codex` CLI as `--config key=value`. Repeatable.
    #[arg(short = 'c', long = "config", value_name = "key=value", action = ArgAction::Append)]
    config_overrides: Vec<String>,

    /// Resume an existing thread instead of starting a new one.
    #[arg(long)]
    thread_id: Option<String>,

    /// Set the approval policy for the thread.
    #[arg(long, default_value = "on-request")]
    approval_policy: String,

    /// Auto-approve command/file-change approvals.
    #[arg(long, default_value_t = false)]
    auto_approve: bool,

    /// Only show final assistant messages and tool calls.
    #[arg(long, default_value_t = false)]
    final_only: bool,

    /// Optional model override when starting/resuming a thread.
    #[arg(long)]
    model: Option<String>,

    /// Optional model provider override when starting/resuming a thread.
    #[arg(long)]
    model_provider: Option<String>,

    /// Optional working directory override when starting/resuming a thread.
    #[arg(long)]
    cwd: Option<String>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let output = Output::new();
    let approval_policy = parse_approval_policy(&cli.approval_policy)?;

    let mut client = AppServerClient::spawn(
        &cli.codex_bin,
        &cli.config_overrides,
        output.clone(),
        cli.final_only,
    )?;
    client.initialize()?;

    let thread_id = if let Some(thread_id) = cli.thread_id.as_ref() {
        client.resume_thread(build_thread_resume_params(
            thread_id.clone(),
            approval_policy,
            cli.model.clone(),
            cli.model_provider.clone(),
            cli.cwd.clone(),
        ))?
    } else {
        client.start_thread(build_thread_start_params(
            approval_policy,
            cli.model.clone(),
            cli.model_provider.clone(),
            cli.cwd.clone(),
        ))?
    };

    output
        .client_line(&format!("connected to thread {thread_id}"))
        .ok();
    output.set_prompt(&thread_id);

    let (event_tx, event_rx) = mpsc::channel();
    client.start_reader(event_tx, cli.auto_approve, cli.final_only)?;

    print_help(&output);

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        drain_events(&event_rx, &output);
        let prompt_thread = client
            .thread_id()
            .unwrap_or_else(|| "no-thread".to_string());
        output.prompt(&prompt_thread).ok();

        let Some(line) = lines.next() else {
            break;
        };
        let line = line.context("read stdin")?;

        match parse_input(&line) {
            Ok(None) => continue,
            Ok(Some(InputAction::Message(message))) => {
                let Some(active_thread) = client.thread_id() else {
                    output
                        .client_line("no active thread; use :new or :resume <id>")
                        .ok();
                    continue;
                };
                if let Err(err) = client.send_turn(&active_thread, message) {
                    output
                        .client_line(&format!("failed to send turn: {err}"))
                        .ok();
                }
            }
            Ok(Some(InputAction::Command(command))) => {
                if !handle_command(command, &client, &output, approval_policy, &cli) {
                    break;
                }
            }
            Err(err) => {
                output.client_line(&err.message()).ok();
            }
        }
    }

    client.shutdown();
    Ok(())
}

fn handle_command(
    command: UserCommand,
    client: &AppServerClient,
    output: &Output,
    approval_policy: AskForApproval,
    cli: &Cli,
) -> bool {
    match command {
        UserCommand::Help => {
            print_help(output);
            true
        }
        UserCommand::Quit => false,
        UserCommand::NewThread => {
            match client.request_thread_start(build_thread_start_params(
                approval_policy,
                cli.model.clone(),
                cli.model_provider.clone(),
                cli.cwd.clone(),
            )) {
                Ok(request_id) => {
                    output
                        .client_line(&format!("requested new thread ({request_id:?})"))
                        .ok();
                }
                Err(err) => {
                    output
                        .client_line(&format!("failed to start thread: {err}"))
                        .ok();
                }
            }
            true
        }
        UserCommand::Resume(thread_id) => {
            match client.request_thread_resume(build_thread_resume_params(
                thread_id,
                approval_policy,
                cli.model.clone(),
                cli.model_provider.clone(),
                cli.cwd.clone(),
            )) {
                Ok(request_id) => {
                    output
                        .client_line(&format!("requested thread resume ({request_id:?})"))
                        .ok();
                }
                Err(err) => {
                    output
                        .client_line(&format!("failed to resume thread: {err}"))
                        .ok();
                }
            }
            true
        }
        UserCommand::Use(thread_id) => {
            let known = client.use_thread(thread_id.clone());
            output.set_prompt(&thread_id);
            if known {
                output
                    .client_line(&format!("switched active thread to {thread_id}"))
                    .ok();
            } else {
                output
                    .client_line(&format!(
                        "switched active thread to {thread_id} (unknown; use :resume to load)"
                    ))
                    .ok();
            }
            true
        }
        UserCommand::RefreshThread => {
            match client.request_thread_list(None) {
                Ok(request_id) => {
                    output
                        .client_line(&format!("requested thread list ({request_id:?})"))
                        .ok();
                }
                Err(err) => {
                    output
                        .client_line(&format!("failed to list threads: {err}"))
                        .ok();
                }
            }
            true
        }
    }
}

fn parse_approval_policy(value: &str) -> Result<AskForApproval> {
    match value {
        "untrusted" | "unless-trusted" | "unlessTrusted" => Ok(AskForApproval::UnlessTrusted),
        "on-failure" | "onFailure" => Ok(AskForApproval::OnFailure),
        "on-request" | "onRequest" => Ok(AskForApproval::OnRequest),
        "never" => Ok(AskForApproval::Never),
        _ => anyhow::bail!(
            "unknown approval policy: {value}. Expected one of: untrusted, on-failure, on-request, never"
        ),
    }
}

fn drain_events(event_rx: &mpsc::Receiver<ReaderEvent>, output: &Output) {
    while let Ok(event) = event_rx.try_recv() {
        match event {
            ReaderEvent::ThreadReady { thread_id } => {
                output
                    .client_line(&format!("active thread is now {thread_id}"))
                    .ok();
                output.set_prompt(&thread_id);
            }
            ReaderEvent::ThreadList {
                thread_ids,
                next_cursor,
            } => {
                if thread_ids.is_empty() {
                    output.client_line("threads: (none)").ok();
                } else {
                    output.client_line("threads:").ok();
                    for thread_id in thread_ids {
                        output.client_line(&format!("  {thread_id}")).ok();
                    }
                }
                if let Some(next_cursor) = next_cursor {
                    output
                        .client_line(&format!(
                            "more threads available, next cursor: {next_cursor}"
                        ))
                        .ok();
                }
            }
        }
    }
}

fn print_help(output: &Output) {
    let _ = output.client_line("commands:");
    let _ = output.client_line("  :help                 show this help");
    let _ = output.client_line("  :new                  start a new thread");
    let _ = output.client_line("  :resume <thread-id>   resume an existing thread");
    let _ = output.client_line("  :use <thread-id>      switch the active thread");
    let _ = output.client_line("  :refresh-thread       list available threads");
    let _ = output.client_line("  :quit                 exit");
    let _ = output.client_line("type a message to send it as a new turn");
}
