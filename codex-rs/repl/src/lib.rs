use std::io::stdin;
use std::io::stdout;
use std::io::Write;
use std::sync::Arc;

use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol;
use codex_core::protocol::FileChange;
use codex_core::util::is_inside_git_repo;
use codex_core::util::notify_on_sigint;
use codex_core::Codex;
use owo_colors::OwoColorize;
use owo_colors::Style;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::io::Lines;
use tokio::io::Stdin;
use tokio::sync::Notify;
use tracing::debug;
use tracing_subscriber::EnvFilter;

mod cli;
pub use cli::Cli;

/// Initialize the global logger once at startup based on the `--verbose` flag.
fn init_logger(verbose: u8, allow_ansi: bool) {
    // Map -v occurrences to explicit log levels:
    //   0 → warn (default)
    //   1 → info
    //   2 → debug
    //   ≥3 → trace

    let default_level = match verbose {
        0 => "warn",
        1 => "info",
        2 => "codex=debug",
        _ => "codex=trace",
    };

    // Only initialize the logger once – repeated calls are ignored. `try_init` will return an
    // error if another crate (like tests) initialized it first, which we can safely ignore.
    // By default `tracing_subscriber::fmt()` writes formatted logs to stderr. That is fine when
    // running the CLI manually but in our smoke tests we capture **stdout** (via `assert_cmd`) and
    // ignore stderr. As a result none of the `tracing::info!` banners or warnings show up in the
    // recorded output making it much harder to debug live runs.

    // Switch the logger's writer to stdout so both human runs and the integration tests see the
    // same stream. Disable ANSI colors because the binary already prints plain text and color
    // escape codes make predicate matching brittle.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap(),
        )
        .with_ansi(allow_ansi)
        .with_writer(std::io::stdout)
        .try_init();
}

pub async fn run_main(cli: Cli) -> anyhow::Result<()> {
    let ctrl_c = notify_on_sigint();

    // Abort early when the user runs Codex outside a Git repository unless
    // they explicitly acknowledged the risks with `--allow-no-git-exec`.
    if !cli.allow_no_git_exec && !is_inside_git_repo() {
        eprintln!(
            "We recommend running codex inside a git repository. \
            If you understand the risks, you can proceed with \
            `--allow-no-git-exec`."
        );
        std::process::exit(1);
    }

    // Initialize logging before any other work so early errors are captured.
    init_logger(cli.verbose, !cli.no_ansi);

    // Load config file and apply CLI overrides (model & approval policy)
    let overrides = ConfigOverrides {
        model: cli.model.clone(),
        approval_policy: cli.approval_policy.map(Into::into),
        sandbox_policy: cli.sandbox_policy.map(Into::into),
        disable_response_storage: if cli.disable_response_storage {
            Some(true)
        } else {
            None
        },
    };
    let config = Config::load_with_overrides(overrides)?;

    codex_main(cli, config, ctrl_c).await
}

async fn codex_main(cli: Cli, cfg: Config, ctrl_c: Arc<Notify>) -> anyhow::Result<()> {
    let mut builder = Codex::builder();
    if let Some(path) = cli.record_submissions {
        builder = builder.record_submissions(path);
    }
    if let Some(path) = cli.record_events {
        builder = builder.record_events(path);
    }
    let codex = builder.spawn(Arc::clone(&ctrl_c))?;
    let init_id = random_id();
    let init = protocol::Submission {
        id: init_id.clone(),
        op: protocol::Op::ConfigureSession {
            model: cfg.model,
            instructions: cfg.instructions,
            approval_policy: cfg.approval_policy,
            sandbox_policy: cfg.sandbox_policy,
            disable_response_storage: cfg.disable_response_storage,
        },
    };

    out(
        "initializing session",
        MessagePriority::BackgroundEvent,
        MessageActor::User,
    );
    codex.submit(init).await?;

    // init
    loop {
        out(
            "waiting for session initialization",
            MessagePriority::BackgroundEvent,
            MessageActor::User,
        );
        let event = codex.next_event().await?;
        if event.id == init_id {
            if let protocol::EventMsg::Error { message } = event.msg {
                anyhow::bail!("Error during initialization: {message}");
            } else {
                out(
                    "session initialized",
                    MessagePriority::BackgroundEvent,
                    MessageActor::User,
                );
                break;
            }
        }
    }

    // run loop
    let mut reader = InputReader::new(ctrl_c.clone());
    loop {
        let text = match &cli.prompt {
            Some(input) => input.clone(),
            None => match reader.request_input().await? {
                Some(input) => input,
                None => {
                    // ctrl + d
                    println!();
                    return Ok(());
                }
            },
        };
        if text.trim().is_empty() {
            continue;
        }
        // Interpret certain single‑word commands as immediate termination requests.
        let trimmed = text.trim();
        if trimmed == "q" {
            // Exit gracefully.
            println!("Exiting…");
            return Ok(());
        }

        let sub = protocol::Submission {
            id: random_id(),
            op: protocol::Op::UserInput {
                items: vec![protocol::InputItem::Text { text }],
            },
        };

        out(
            "sending request to model",
            MessagePriority::TaskProgress,
            MessageActor::User,
        );
        codex.submit(sub).await?;

        // Wait for agent events **or** user interrupts (Ctrl+C).
        'inner: loop {
            // Listen for either the next agent event **or** a SIGINT notification.  Using
            // `tokio::select!` allows the user to cancel a long‑running request that would
            // otherwise leave the CLI stuck waiting for a server response.
            let event = {
                let interrupted = ctrl_c.notified();
                tokio::select! {
                    _ = interrupted => {
                        // Forward an interrupt to the agent so it can abort any in‑flight task.
                        let _ = codex
                            .submit(protocol::Submission {
                                id: random_id(),
                                op: protocol::Op::Interrupt,
                            })
                            .await;

                        // Exit the inner loop and return to the main input prompt.  The agent
                        // will emit a `TurnInterrupted` (Error) event which is drained later.
                        break 'inner;
                    }
                    res = codex.next_event() => res?
                }
            };

            debug!(?event, "Got event");
            let id = event.id;
            match event.msg {
                protocol::EventMsg::Error { message } => {
                    println!("Error: {message}");
                    break 'inner;
                }
                protocol::EventMsg::TaskComplete => break 'inner,
                protocol::EventMsg::AgentMessage { message } => {
                    out(&message, MessagePriority::UserMessage, MessageActor::Agent)
                }
                protocol::EventMsg::SessionConfigured { model } => {
                    debug!(model, "Session initialized");
                }
                protocol::EventMsg::ExecApprovalRequest {
                    command,
                    cwd,
                    reason,
                } => {
                    let reason_str = reason
                        .as_deref()
                        .map(|r| format!(" [{r}]"))
                        .unwrap_or_default();

                    let prompt = format!(
                        "approve command in {} {}{} (y/N): ",
                        cwd.display(),
                        command.join(" "),
                        reason_str
                    );
                    let decision = request_user_approval2(prompt)?;
                    let sub = protocol::Submission {
                        id: random_id(),
                        op: protocol::Op::ExecApproval { id, decision },
                    };
                    out(
                        "submitting command approval",
                        MessagePriority::TaskProgress,
                        MessageActor::User,
                    );
                    codex.submit(sub).await?;
                }
                protocol::EventMsg::ApplyPatchApprovalRequest {
                    changes,
                    reason: _,
                    grant_root: _,
                } => {
                    let file_list = changes
                        .keys()
                        .map(|path| path.to_string_lossy().to_string())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let request =
                        format!("approve apply_patch that will touch? {file_list} (y/N): ");
                    let decision = request_user_approval2(request)?;
                    let sub = protocol::Submission {
                        id: random_id(),
                        op: protocol::Op::PatchApproval { id, decision },
                    };
                    out(
                        "submitting patch approval",
                        MessagePriority::UserMessage,
                        MessageActor::Agent,
                    );
                    codex.submit(sub).await?;
                }
                protocol::EventMsg::ExecCommandBegin {
                    command,
                    cwd,
                    call_id: _,
                } => {
                    out(
                        &format!("running command: '{}' in '{}'", command.join(" "), cwd),
                        MessagePriority::BackgroundEvent,
                        MessageActor::Agent,
                    );
                }
                protocol::EventMsg::ExecCommandEnd {
                    stdout,
                    stderr,
                    exit_code,
                    call_id: _,
                } => {
                    let msg = if exit_code == 0 {
                        "command completed (exit 0)".to_string()
                    } else {
                        // Prefer stderr but fall back to stdout if empty.
                        let err_snippet = if !stderr.trim().is_empty() {
                            stderr.trim()
                        } else {
                            stdout.trim()
                        };
                        format!("command failed (exit {exit_code}): {err_snippet}")
                    };
                    out(&msg, MessagePriority::BackgroundEvent, MessageActor::Agent);
                    out(
                        "sending results to model",
                        MessagePriority::TaskProgress,
                        MessageActor::Agent,
                    );
                }
                protocol::EventMsg::PatchApplyBegin { changes, .. } => {
                    // Emit PatchApplyBegin so the front‑end can show progress.
                    let summary = changes
                        .iter()
                        .map(|(path, change)| match change {
                            FileChange::Add { .. } => format!("A {}", path.display()),
                            FileChange::Delete => format!("D {}", path.display()),
                            FileChange::Update { .. } => format!("M {}", path.display()),
                        })
                        .collect::<Vec<_>>()
                        .join(", ");

                    out(
                        &format!("applying patch: {summary}"),
                        MessagePriority::BackgroundEvent,
                        MessageActor::Agent,
                    );
                }
                protocol::EventMsg::PatchApplyEnd { success, .. } => {
                    let status = if success { "success" } else { "failed" };
                    out(
                        &format!("patch application {status}"),
                        MessagePriority::BackgroundEvent,
                        MessageActor::Agent,
                    );
                    out(
                        "sending results to model",
                        MessagePriority::TaskProgress,
                        MessageActor::Agent,
                    );
                }
                // Broad fallback; if the CLI is unaware of an event type, it will just
                // print it as a generic BackgroundEvent.
                e => {
                    out(
                        &format!("event: {e:?}"),
                        MessagePriority::BackgroundEvent,
                        MessageActor::Agent,
                    );
                }
            }
        }
    }
}

fn random_id() -> String {
    let id: u64 = rand::random();
    id.to_string()
}

fn request_user_approval2(request: String) -> anyhow::Result<protocol::ReviewDecision> {
    println!("{}", request);

    let mut line = String::new();
    stdin().read_line(&mut line)?;
    let answer = line.trim().to_ascii_lowercase();
    let is_accepted = answer == "y" || answer == "yes";
    let decision = if is_accepted {
        protocol::ReviewDecision::Approved
    } else {
        protocol::ReviewDecision::Denied
    };
    Ok(decision)
}

#[derive(Debug, Clone, Copy)]
enum MessagePriority {
    BackgroundEvent,
    TaskProgress,
    UserMessage,
}
enum MessageActor {
    Agent,
    User,
}

impl From<MessageActor> for String {
    fn from(actor: MessageActor) -> Self {
        match actor {
            MessageActor::Agent => "codex".to_string(),
            MessageActor::User => "user".to_string(),
        }
    }
}

fn out(msg: &str, priority: MessagePriority, actor: MessageActor) {
    let actor: String = actor.into();
    let style = match priority {
        MessagePriority::BackgroundEvent => Style::new().fg_rgb::<127, 127, 127>(),
        MessagePriority::TaskProgress => Style::new().fg_rgb::<200, 200, 200>(),
        MessagePriority::UserMessage => Style::new().white(),
    };

    println!("{}> {}", actor.bold(), msg.style(style));
}

struct InputReader {
    reader: Lines<BufReader<Stdin>>,
    ctrl_c: Arc<Notify>,
}

impl InputReader {
    pub fn new(ctrl_c: Arc<Notify>) -> Self {
        Self {
            reader: BufReader::new(tokio::io::stdin()).lines(),
            ctrl_c,
        }
    }

    pub async fn request_input(&mut self) -> std::io::Result<Option<String>> {
        print!("user> ");
        stdout().flush()?;
        let interrupted = self.ctrl_c.notified();
        tokio::select! {
            line = self.reader.next_line() => {
                match line? {
                    Some(input) => Ok(Some(input.trim().to_string())),
                    None => Ok(None),
                }
            }
            _ = interrupted => {
                println!();
                Ok(Some(String::new()))
            }
        }
    }
}
