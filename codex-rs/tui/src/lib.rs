// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]

use app::App;
use codex_core::util::is_inside_git_repo;
use log_layer::TuiLogLayer;
use std::fs::OpenOptions;
use tracing_appender::non_blocking;
use tracing_subscriber::prelude::*;
use tracing_subscriber::EnvFilter;

mod app;
mod app_event;
mod bottom_pane;
mod chatwidget;
mod cli;
mod conversation_history_widget;
mod exec_command;
mod git_warning_screen;
mod history_cell;
mod log_layer;
mod status_indicator_widget;
mod tui;
mod user_approval_widget;

pub use cli::Cli;

pub fn run_main(cli: Cli) -> std::io::Result<()> {
    assert_env_var_set();

    // Open (or create) your log file, appending to it.
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open("/tmp/codex-rs.log")?;

    // Wrap file in non‑blocking writer.
    let (non_blocking, _guard) = non_blocking(file);

    // use RUST_LOG env var, default to trace for codex crates.
    let env_filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("codex=trace,codex_tui=trace"))
    };

    // Build layered subscriber:
    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .with_filter(env_filter());

    // Channel that carries formatted log lines to the UI.
    let (log_tx, log_rx) = tokio::sync::mpsc::unbounded_channel::<String>();
    let tui_layer = TuiLogLayer::new(log_tx.clone(), 120).with_filter(env_filter());

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(tui_layer)
        .try_init();

    // Determine whether we need to display the "not a git repo" warning
    // modal. The flag is shown when the current working directory is *not*
    // inside a Git repository **and** the user did *not* pass the
    // `--allow-no-git-exec` flag.
    let show_git_warning = !cli.skip_git_repo_check && !is_inside_git_repo();

    try_run_ratatui_app(cli, show_git_warning, log_rx);
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "Resort to stderr in exceptional situations."
)]
fn try_run_ratatui_app(
    cli: Cli,
    show_git_warning: bool,
    log_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    if let Err(report) = run_ratatui_app(cli, show_git_warning, log_rx) {
        eprintln!("Error: {report:?}");
    }
}

fn run_ratatui_app(
    cli: Cli,
    show_git_warning: bool,
    mut log_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) -> color_eyre::Result<()> {
    color_eyre::install()?;

    // Forward panic reports through the tracing stack so that they appear in
    // the status indicator instead of breaking the alternate screen – the
    // normal colour‑eyre hook writes to stderr which would corrupt the UI.
    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
    let mut terminal = tui::init()?;
    terminal.clear()?;

    let Cli {
        prompt,
        images,
        approval_policy,
        sandbox_policy: sandbox,
        model,
        ..
    } = cli;

    let approval_policy = approval_policy.into();
    let sandbox_policy = sandbox.into();

    let mut app = App::new(
        approval_policy,
        sandbox_policy,
        prompt,
        show_git_warning,
        images,
        model,
    );

    // Bridge log receiver into the AppEvent channel so latest log lines update the UI.
    {
        let app_event_tx = app.event_sender();
        tokio::spawn(async move {
            while let Some(line) = log_rx.recv().await {
                let _ = app_event_tx.send(crate::app_event::AppEvent::LatestLog(line));
            }
        });
    }

    let app_result = app.run(&mut terminal);

    restore();
    app_result
}

#[expect(
    clippy::print_stderr,
    reason = "TUI should not have been displayed yet, so we can write to stderr."
)]
fn assert_env_var_set() {
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!("Welcome to codex! It looks like you're missing: `OPENAI_API_KEY`");
        eprintln!(
            "Create an API key (https://platform.openai.com) and export as an environment variable"
        );
        std::process::exit(1);
    }
}

#[expect(
    clippy::print_stderr,
    reason = "TUI should no longer be displayed, so we can write to stderr."
)]
fn restore() {
    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {}",
            err
        );
    }
}
