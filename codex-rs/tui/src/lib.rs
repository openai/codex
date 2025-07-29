// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
use app::App;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config_types::SandboxMode;
use codex_core::openai_api_key::OPENAI_API_KEY_ENV_VAR;
use codex_core::openai_api_key::get_openai_api_key;
use codex_core::openai_api_key::set_openai_api_key;
use codex_core::protocol::AskForApproval;
use codex_core::util::is_inside_git_repo;
use codex_login::try_read_openai_api_key;
use log_layer::TuiLogLayer;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod app;
mod app_event;
mod app_event_sender;
mod bottom_pane;
mod cell_widget;
mod chatwidget;
mod citation_regex;
mod cli;
mod conversation_history_widget;
mod exec_command;
mod file_search;
mod get_git_diff;
mod git_warning_screen;
mod history_cell;
mod insert_history;
mod log_layer;
mod markdown;
mod scroll_event_helper;
mod slash_command;
mod status_indicator_widget;
mod text_block;
mod text_formatting;
mod tui;
mod user_approval_widget;

pub use cli::Cli;

pub async fn run_main(
    cli: Cli,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<codex_core::protocol::TokenUsage> {
    let (sandbox_mode, approval_policy) = if cli.full_auto {
        (
            Some(SandboxMode::WorkspaceWrite),
            Some(AskForApproval::OnFailure),
        )
    } else if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never),
        )
    } else {
        (
            cli.sandbox_mode.map(Into::<SandboxMode>::into),
            cli.approval_policy.map(Into::into),
        )
    };

    let config = {
        // Load configuration and support CLI overrides.
        let overrides = ConfigOverrides {
            model: cli.model.clone(),
            approval_policy,
            sandbox_mode,
            cwd: cli.cwd.clone().map(|p| p.canonicalize().unwrap_or(p)),
            model_provider: None,
            config_profile: cli.config_profile.clone(),
            codex_linux_sandbox_exe,
            base_instructions: None,
        };
        // Parse `-c` overrides from the CLI.
        let cli_kv_overrides = match cli.config_overrides.parse_overrides() {
            Ok(v) => v,
            #[allow(clippy::print_stderr)]
            Err(e) => {
                eprintln!("Error parsing -c overrides: {e}");
                std::process::exit(1);
            }
        };

        #[allow(clippy::print_stderr)]
        match Config::load_with_cli_overrides(cli_kv_overrides, overrides) {
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

    let show_login_screen = should_show_login_screen(&config).await;
    if show_login_screen {
        std::io::stdout().write_all(
            b"Oh dear, we don't seem to have an API key.\nTerribly sorry, but may I open a browser window for you to log in? [Yn] ",
        )?;
        std::io::stdout().flush()?;
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        let trimmed = input.trim();
        if !(trimmed.is_empty() || trimmed.eq_ignore_ascii_case("y")) {
            std::io::stdout().write_all(b"Right-o, fair enough. See you next time!\n")?;
            std::process::exit(1);
        }
        // Spawn a task to run the login command.
        // Block until the login command is finished.
        let new_key = codex_login::login_with_chatgpt(&config.codex_home, false).await?;
        set_openai_api_key(new_key);
        std::io::stdout().write_all(b"Excellent, looks like that worked. Let's get started!\n")?;
    }

    // Determine whether we need to display the "not a git repo" warning
    // modal. The flag is shown when the current working directory is *not*
    // inside a Git repository **and** the user did *not* pass the
    // `--allow-no-git-exec` flag.
    let show_git_warning = !cli.skip_git_repo_check && !is_inside_git_repo(&config);

    run_ratatui_app(cli, config, show_git_warning, log_rx)
        .map_err(|err| std::io::Error::other(err.to_string()))
}

fn run_ratatui_app(
    cli: Cli,
    config: Config,
    show_git_warning: bool,
    mut log_rx: tokio::sync::mpsc::UnboundedReceiver<String>,
) -> color_eyre::Result<codex_core::protocol::TokenUsage> {
    color_eyre::install()?;

    // Forward panic reports through tracing so they appear in the UI status
    // line instead of interleaving raw panic output with the interface.
    std::panic::set_hook(Box::new(|info| {
        tracing::error!("panic: {info}");
    }));
    let mut terminal = tui::init(&config)?;
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

    let app_result = app.run(&mut terminal);
    let usage = app.token_usage();

    restore();
    // ignore error when collecting usage – report underlying error instead
    app_result.map(|_| usage)
}

#[expect(
    clippy::print_stderr,
    reason = "TUI should no longer be displayed, so we can write to stderr."
)]
fn restore() {
    if let Err(err) = tui::restore() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
}

async fn should_show_login_screen(config: &Config) -> bool {
    if is_in_need_of_openai_api_key(config) {
        // Reading the OpenAI API key is an async operation because it may need
        // to refresh the token. Block on it.
        let codex_home = config.codex_home.clone();
        if let Ok(openai_api_key) = try_read_openai_api_key(&codex_home).await {
            set_openai_api_key(openai_api_key);
            false
        } else {
            true
        }
    } else {
        false
    }
}

fn is_in_need_of_openai_api_key(config: &Config) -> bool {
    let is_using_openai_key = config
        .model_provider
        .env_key
        .as_ref()
        .map(|s| s == OPENAI_API_KEY_ENV_VAR)
        .unwrap_or(false);
    is_using_openai_key && get_openai_api_key().is_none()
}
