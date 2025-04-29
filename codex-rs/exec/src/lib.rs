mod cli;
mod event_processor;

use std::io::IsTerminal;
use std::sync::Arc;

pub use cli::Cli;
use codex_core::codex_wrapper;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use codex_core::util::is_inside_git_repo;
use event_processor::EventProcessor;
use owo_colors::OwoColorize;
use owo_colors::Style;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::EnvFilter;

pub async fn run_main(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        images,
        model,
        sandbox_policy,
        skip_git_repo_check,
        disable_response_storage,
        color,
        prompt,
    } = cli;

    let (stdout_with_ansi, stderr_with_ansi) = match color {
        cli::Color::Always => (true, true),
        cli::Color::Never => (false, false),
        cli::Color::Auto => (
            std::io::stdout().is_terminal(),
            std::io::stderr().is_terminal(),
        ),
    };

    assert_api_key(stderr_with_ansi);

    if !skip_git_repo_check && !is_inside_git_repo() {
        eprintln!("Not inside a Git repo and --skip-git-repo-check was not specified.");
        std::process::exit(1);
    }

    // TODO(mbolin): Take a more thoughtful approach to logging.
    let default_level = "error";
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .or_else(|_| EnvFilter::try_new(default_level))
                .unwrap(),
        )
        .with_ansi(stderr_with_ansi)
        .with_writer(std::io::stderr)
        .try_init();

    // Load configuration and determine approval policy
    let overrides = ConfigOverrides {
        model,
        // This CLI is intended to be headless and has no affordances for asking
        // the user for approval.
        approval_policy: Some(AskForApproval::Never),
        sandbox_policy: sandbox_policy.map(Into::into),
        disable_response_storage: if disable_response_storage {
            Some(true)
        } else {
            None
        },
    };
    let config = Config::load_with_overrides(overrides)?;
    let (codex_wrapper, event, ctrl_c) = codex_wrapper::init_codex(config).await?;
    let codex = Arc::new(codex_wrapper);
    info!("Codex initialized with event: {event:?}");

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<Event>();
    {
        let codex = codex.clone();
        tokio::spawn(async move {
            loop {
                let interrupted = ctrl_c.notified();
                tokio::select! {
                    _ = interrupted => {
                        // Forward an interrupt to the codex so it can abort any in‑flight task.
                        let _ = codex
                            .submit(
                                Op::Interrupt,
                            )
                            .await;

                        // Exit the inner loop and return to the main input prompt.  The codex
                        // will emit a `TurnInterrupted` (Error) event which is drained later.
                        break;
                    }
                    res = codex.next_event() => match res {
                        Ok(event) => {
                            debug!("Received event: {event:?}");
                            if let Err(e) = tx.send(event) {
                                error!("Error sending event: {e:?}");
                                break;
                            }
                        },
                        Err(e) => {
                            error!("Error receiving event: {e:?}");
                            break;
                        }
                    }
                }
            }
        });
    }

    // Send images first, if any.
    if !images.is_empty() {
        let items: Vec<InputItem> = images
            .into_iter()
            .map(|path| InputItem::LocalImage { path })
            .collect();
        let initial_images_event_id = codex.submit(Op::UserInput { items }).await?;
        info!("Sent images with event ID: {initial_images_event_id}");
        while let Ok(event) = codex.next_event().await {
            if event.id == initial_images_event_id && matches!(event.msg, EventMsg::TaskComplete) {
                break;
            }
        }
    }

    // Send the prompt.
    let items: Vec<InputItem> = vec![InputItem::Text { text: prompt }];
    let initial_prompt_task_id = codex.submit(Op::UserInput { items }).await?;
    info!("Sent prompt with event ID: {initial_prompt_task_id}");

    // Run the loop until the task is complete.
    let mut event_processor = EventProcessor::create_with_ansi(stdout_with_ansi);
    while let Some(event) = rx.recv().await {
        let last_event =
            event.id == initial_prompt_task_id && matches!(event.msg, EventMsg::TaskComplete);
        event_processor.process_event(event);
        if last_event {
            break;
        }
    }

    Ok(())
}

/// If a valid API key is not present in the environment, print an error to
/// stderr and exits with 1; otherwise, does nothing.
fn assert_api_key(stderr_with_ansi: bool) {
    if !has_api_key() {
        let (msg_style, var_style, url_style) = if stderr_with_ansi {
            (
                Style::new().red(),
                Style::new().bold(),
                Style::new().bold().underline(),
            )
        } else {
            (Style::new(), Style::new(), Style::new())
        };

        eprintln!(
            "\n{msg}\n\nSet the environment variable {var} and re-run this command.\nYou can create a key here: {url}\n",
            msg = "Missing OpenAI API key.".style(msg_style),
            var = "OPENAI_API_KEY".style(var_style),
            url = "https://platform.openai.com/account/api-keys".style(url_style),
        );
        std::process::exit(1);
    }
}

/// Returns `true` if a recognized API key is present in the environment.
///
/// At present we only support `OPENAI_API_KEY`, mirroring the behavior of the
/// Node-based `codex-cli`. Additional providers can be added here when the
/// Rust implementation gains first-class support for them.
fn has_api_key() -> bool {
    std::env::var("OPENAI_API_KEY")
        .map(|s| !s.trim().is_empty())
        .unwrap_or(false)
}
