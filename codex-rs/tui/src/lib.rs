// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
use app::App;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_core::util::is_inside_git_repo;
use log_layer::TuiLogLayer;
use std::fs::OpenOptions;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod app;
mod app_event;
mod app_event_sender;
mod bottom_pane;
mod chatwidget;
mod citation_regex;
mod cli;
mod conversation_history_widget;
mod exec_command;
mod git_warning_screen;
mod history_cell;
mod log_layer;
mod markdown;
mod mouse_capture;
mod scroll_event_helper;
mod slash_command;
mod status_indicator_widget;
mod tui;
mod user_approval_widget;

pub use cli::Cli;

pub fn run_main(cli: Cli) -> std::io::Result<()> {
    let (sandbox_policy, approval_policy) = if cli.full_auto {
        (
            Some(SandboxPolicy::new_full_auto_policy()),
            Some(AskForApproval::OnFailure),
        )
    } else {
        let sandbox_policy = cli.sandbox.permissions.clone().map(Into::into);
        (sandbox_policy, cli.approval_policy.map(Into::into))
    };

    let config = {
        // Load configuration and support CLI overrides.
        let overrides = ConfigOverrides {
            model: cli.model.clone(),
            approval_policy,
            sandbox_policy,
            disable_response_storage: if cli.disable_response_storage {
                Some(true)
            } else {
                None
            },
            cwd: cli.cwd.clone().map(|p| p.canonicalize().unwrap_or(p)),
            model_provider: None,
            config_profile: cli.config_profile.clone(),
        };
        #[allow(clippy::print_stderr)]
        match Config::load_with_overrides(overrides) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error loading configuration: {err}");
                std::process::exit(1);
            }
        }
    };

    let log_dir = codex_core::config::log_dir(&config)?;
    std::fs::create_dir_all(&log_dir)?;
    // Open (or create) your log file, appending to it.
    let mut log_file_opts = OpenOptions::new();
    log_file_opts.create(true).append(true);

    // Ensure the file is only readable and writable by the current user.
    // Doing the equivalent to `chmod 600` on Windows is quite a bit more code
    // and requires the Windows API crates, so we can reconsider that when
    // Codex CLI is officially supported on Windows.
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        log_file_opts.mode(0o600);
    }

    let log_file = log_file_opts.open(log_dir.join("codex-tui.log"))?;

    // Wrap file in non‑blocking writer.
    let (non_blocking, _guard) = non_blocking(log_file);

    // use RUST_LOG env var, default to info for codex crates.
    let env_filter = || {
        EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("codex_core=info,codex_tui=info"))
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
    let show_git_warning = !cli.skip_git_repo_check && !is_inside_git_repo(&config);

    try_run_ratatui_app(cli, config, show_git_warning, log_rx);
    Ok(())
}

#[expect(
    clippy::print_stderr,
    reason = "Resort to stderr in exceptional situations."
)]
fn try_run_ratatui_app(
    cli: Cli,
    config: Config,
    show_git_warning: bool,
    log_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) {
    if let Err(report) = run_ratatui_app(cli, config, show_git_warning, log_rx) {
        eprintln!("Error: {report:?}");
    }
}

fn run_ratatui_app(
    cli: Cli,
    config: Config,
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
    let (mut terminal, mut mouse_capture) = tui::init(&config)?;
    terminal.clear()?;

    let Cli { prompt, images, .. } = cli;
    let mut app = App::new(config.clone(), prompt, show_git_warning, images);

    // Bridge log receiver into the AppEvent channel so latest log lines update the UI.
    {
        let app_event_tx = app.event_sender();
        tokio::spawn(async move {
            while let Some(line) = log_rx.recv().await {
                app_event_tx.send(crate::app_event::AppEvent::LatestLog(line));
            }
        });
    }

    let app_result = app.run(&mut terminal, &mut mouse_capture);

    restore();
    app_result
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
