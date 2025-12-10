// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use additional_dirs::add_dir_warning_message;
use app::App;
pub use app::AppExitInfo;
use codex_app_server_protocol::AuthMode;
use codex_common::oss::ensure_oss_provider_ready;
use codex_common::oss::get_default_model_for_oss_provider;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::RolloutRecorder;
use codex_core::auth::enforce_login_restrictions;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::resolve_oss_provider;
use codex_core::find_conversation_path_by_id_str;
use codex_core::get_platform_sandbox;
use codex_core::protocol::AskForApproval;
use codex_protocol::config_types::SandboxMode;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::error;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;

mod additional_dirs;
mod app;
mod app_backtrack;
mod app_event;
mod app_event_sender;
mod ascii_animation;
mod bottom_pane;
mod chatwidget;
mod cli;
mod clipboard_paste;
mod color;
pub mod custom_terminal;
mod diff_render;
mod exec_cell;
mod exec_command;
mod file_search;
mod frames;
mod get_git_diff;
mod history_cell;
pub mod insert_history;
mod key_hint;
pub mod live_wrap;
mod markdown;
mod markdown_render;
mod markdown_stream;
mod model_migration;
pub mod onboarding;
mod oss_selection;
mod pager_overlay;
pub mod public_widgets;
mod render;
mod resume_picker;
mod selection_list;
mod session_log;
mod shimmer;
mod skill_error_prompt;
mod slash_command;
mod status;
mod status_indicator_widget;
mod streaming;
mod style;
mod terminal_palette;
mod text_formatting;
mod tooltips;
mod tui;
mod ui_consts;
pub mod update_action;
mod update_prompt;
mod updates;
mod version;

mod wrapping;

#[cfg(test)]
pub mod test_backend;

use crate::onboarding::TrustDirectorySelection;
use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::tui::Tui;
pub use cli::Cli;
pub use markdown_render::render_markdown_text;
pub use public_widgets::composer_input::ComposerAction;
pub use public_widgets::composer_input::ComposerInput;
use std::io::Write as _;

// (tests access modules directly within the crate)

pub async fn run_main(
    mut cli: Cli,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> std::io::Result<AppExitInfo> {
    let (sandbox_mode, approval_policy) = if cli.full_auto {
        (
            Some(SandboxMode::WorkspaceWrite),
            Some(AskForApproval::OnRequest),
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

    // Map the legacy --search flag to the new feature toggle.
    if cli.web_search {
        cli.config_overrides
            .raw_overrides
            .push("features.web_search_request=true".to_string());
    }

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    // `oss` model provider.
    let overrides_cli = codex_common::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = match overrides_cli.parse_overrides() {
        // Parse `-c` overrides from the CLI.
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let codex_home = match find_codex_home() {
        Ok(codex_home) => codex_home.to_path_buf(),
        Err(err) => {
            eprintln!("Error finding codex home: {err}");
            std::process::exit(1);
        }
    };

    #[allow(clippy::print_stderr)]
    let config_toml =
        match load_config_as_toml_with_cli_overrides(&codex_home, cli_kv_overrides.clone()).await {
            Ok(config_toml) => config_toml,
            Err(err) => {
                eprintln!("Error loading config.toml: {err}");
                std::process::exit(1);
            }
        };

    let model_provider_override = if cli.oss {
        let resolved = resolve_oss_provider(
            cli.oss_provider.as_deref(),
            &config_toml,
            cli.config_profile.clone(),
        );

        if let Some(provider) = resolved {
            Some(provider)
        } else {
            // No provider configured, prompt the user
            let provider = oss_selection::select_oss_provider(&codex_home).await?;
            if provider == "__CANCELLED__" {
                return Err(std::io::Error::other(
                    "OSS provider selection was cancelled by user",
                ));
            }
            Some(provider)
        }
    } else {
        None
    };

    // When using `--oss`, let the bootstrapper pick the model based on selected provider
    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        // Use the provider from model_provider_override
        model_provider_override
            .as_ref()
            .and_then(|provider_id| get_default_model_for_oss_provider(provider_id))
            .map(std::borrow::ToOwned::to_owned)
    } else {
        None // No model specified, will use the default.
    };

    // canonicalize the cwd
    let cwd = cli.cwd.clone().map(|p| p.canonicalize().unwrap_or(p));
    let additional_dirs = cli.add_dir.clone();

    let overrides = ConfigOverrides {
        model,
        review_model: None,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override.clone(),
        config_profile: cli.config_profile.clone(),
        codex_linux_sandbox_exe,
        base_instructions: None,
        developer_instructions: None,
        compact_prompt: None,
        include_apply_patch_tool: None,
        show_raw_agent_reasoning: cli.oss.then_some(true),
        tools_web_search_request: None,
        additional_writable_roots: additional_dirs,
    };

    let config = load_config_or_exit(cli_kv_overrides.clone(), overrides.clone()).await;

    if let Some(warning) = add_dir_warning_message(&cli.add_dir, &config.sandbox_policy) {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("Error adding directories: {warning}");
            std::process::exit(1);
        }
    }

    #[allow(clippy::print_stderr)]
    if let Err(err) = enforce_login_restrictions(&config).await {
        eprintln!("{err}");
        std::process::exit(1);
    }

    let active_profile = config.active_profile.clone();
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
        EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            EnvFilter::new("codex_core=info,codex_tui=info,codex_rmcp_client=info")
        })
    };

    let file_layer = tracing_subscriber::fmt::layer()
        .with_writer(non_blocking)
        .with_target(false)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_filter(env_filter());

    let feedback = codex_feedback::CodexFeedback::new();
    let targets = Targets::new().with_default(tracing::Level::TRACE);

    let feedback_layer = tracing_subscriber::fmt::layer()
        .with_writer(feedback.make_writer())
        .with_ansi(false)
        .with_target(false)
        .with_filter(targets);

    if cli.oss && model_provider_override.is_some() {
        // We're in the oss section, so provider_id should be Some
        // Let's handle None case gracefully though just in case
        let provider_id = match model_provider_override.as_ref() {
            Some(id) => id,
            None => {
                error!("OSS provider unexpectedly not set when oss flag is used");
                return Err(std::io::Error::other(
                    "OSS provider not set but oss flag was used",
                ));
            }
        };
        ensure_oss_provider_ready(provider_id, &config).await?;
    }

    let otel = codex_core::otel_init::build_provider(&config, env!("CARGO_PKG_VERSION"));

    #[allow(clippy::print_stderr)]
    let otel = match otel {
        Ok(otel) => otel,
        Err(e) => {
            eprintln!("Could not create otel exporter: {e}");
            std::process::exit(1);
        }
    };

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(feedback_layer)
        .with(otel_tracing_layer)
        .with(otel_logger_layer)
        .try_init();

    run_ratatui_app(
        cli,
        config,
        overrides,
        cli_kv_overrides,
        active_profile,
        feedback,
    )
    .await
    .map_err(|err| std::io::Error::other(err.to_string()))
}

async fn run_ratatui_app(
    cli: Cli,
    initial_config: Config,
    overrides: ConfigOverrides,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    active_profile: Option<String>,
    feedback: codex_feedback::CodexFeedback,
) -> color_eyre::Result<AppExitInfo> {
    color_eyre::install()?;

    // Forward panic reports through tracing so they appear in the UI status
    // line, but do not swallow the default/color-eyre panic handler.
    // Chain to the previous hook so users still get a rich panic report
    // (including backtraces) after we restore the terminal.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        tracing::error!("panic: {info}");
        prev_hook(info);
    }));
    let mut terminal = tui::init()?;
    terminal.clear()?;

    let mut tui = Tui::new(terminal);

    #[cfg(not(debug_assertions))]
    {
        use crate::update_prompt::UpdatePromptOutcome;

        let skip_update_prompt = cli.prompt.as_ref().is_some_and(|prompt| !prompt.is_empty());
        if !skip_update_prompt {
            match update_prompt::run_update_prompt_if_needed(&mut tui, &initial_config).await? {
                UpdatePromptOutcome::Continue => {}
                UpdatePromptOutcome::RunUpdate(action) => {
                    crate::tui::restore()?;
                    return Ok(AppExitInfo {
                        token_usage: codex_core::protocol::TokenUsage::default(),
                        conversation_id: None,
                        update_action: Some(action),
                    });
                }
            }
        }
    }

    // Initialize high-fidelity session event logging if enabled.
    session_log::maybe_init(&initial_config);

    let auth_manager = AuthManager::shared(
        initial_config.codex_home.clone(),
        false,
        initial_config.cli_auth_credentials_store_mode,
    );
    let login_status = get_login_status(&initial_config);
    let should_show_trust_screen = should_show_trust_screen(&initial_config);
    let should_show_onboarding =
        should_show_onboarding(login_status, &initial_config, should_show_trust_screen);

    let config = if should_show_onboarding {
        let onboarding_result = run_onboarding_app(
            OnboardingScreenArgs {
                show_login_screen: should_show_login_screen(login_status, &initial_config),
                show_trust_screen: should_show_trust_screen,
                login_status,
                auth_manager: auth_manager.clone(),
                config: initial_config.clone(),
            },
            &mut tui,
        )
        .await?;
        if onboarding_result.should_exit {
            restore();
            session_log::log_session_end();
            let _ = tui.terminal.clear();
            return Ok(AppExitInfo {
                token_usage: codex_core::protocol::TokenUsage::default(),
                conversation_id: None,
                update_action: None,
            });
        }
        // if the user acknowledged windows or made an explicit decision ato trust the directory, reload the config accordingly
        if onboarding_result
            .directory_trust_decision
            .map(|d| d == TrustDirectorySelection::Trust)
            .unwrap_or(false)
        {
            load_config_or_exit(cli_kv_overrides, overrides).await
        } else {
            initial_config
        }
    } else {
        initial_config
    };

    // Determine resume behavior: explicit id, then resume last, then picker.
    let resume_selection = if let Some(id_str) = cli.resume_session_id.as_deref() {
        match find_conversation_path_by_id_str(&config.codex_home, id_str).await? {
            Some(path) => resume_picker::ResumeSelection::Resume(path),
            None => {
                error!("Error finding conversation path: {id_str}");
                restore();
                session_log::log_session_end();
                let _ = tui.terminal.clear();
                if let Err(err) = writeln!(
                    std::io::stdout(),
                    "No saved session found with ID {id_str}. Run `codex resume` without an ID to choose from existing sessions."
                ) {
                    error!("Failed to write resume error message: {err}");
                }
                return Ok(AppExitInfo {
                    token_usage: codex_core::protocol::TokenUsage::default(),
                    conversation_id: None,
                    update_action: None,
                });
            }
        }
    } else if cli.resume_last {
        let provider_filter = vec![config.model_provider_id.clone()];
        match RolloutRecorder::list_conversations(
            &config.codex_home,
            1,
            None,
            INTERACTIVE_SESSION_SOURCES,
            Some(provider_filter.as_slice()),
            &config.model_provider_id,
        )
        .await
        {
            Ok(page) => page
                .items
                .first()
                .map(|it| resume_picker::ResumeSelection::Resume(it.path.clone()))
                .unwrap_or(resume_picker::ResumeSelection::StartFresh),
            Err(_) => resume_picker::ResumeSelection::StartFresh,
        }
    } else if cli.resume_picker {
        match resume_picker::run_resume_picker(
            &mut tui,
            &config.codex_home,
            &config.model_provider_id,
            cli.resume_show_all,
        )
        .await?
        {
            resume_picker::ResumeSelection::Exit => {
                restore();
                session_log::log_session_end();
                return Ok(AppExitInfo {
                    token_usage: codex_core::protocol::TokenUsage::default(),
                    conversation_id: None,
                    update_action: None,
                });
            }
            other => other,
        }
    } else {
        resume_picker::ResumeSelection::StartFresh
    };

    let Cli { prompt, images, .. } = cli;

    let app_result = App::run(
        &mut tui,
        auth_manager,
        config,
        active_profile,
        prompt,
        images,
        resume_selection,
        feedback,
        should_show_trust_screen, // Proxy to: is it a first run in this directory?
    )
    .await;

    restore();
    // Mark the end of the recorded session.
    session_log::log_session_end();
    // ignore error when collecting usage – report underlying error instead
    app_result
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    AuthMode(AuthMode),
    NotAuthenticated,
}

fn get_login_status(config: &Config) -> LoginStatus {
    if config.model_provider.requires_openai_auth {
        // Reading the OpenAI API key is an async operation because it may need
        // to refresh the token. Block on it.
        let codex_home = config.codex_home.clone();
        match CodexAuth::from_auth_storage(&codex_home, config.cli_auth_credentials_store_mode) {
            Ok(Some(auth)) => LoginStatus::AuthMode(auth.mode),
            Ok(None) => LoginStatus::NotAuthenticated,
            Err(err) => {
                error!("Failed to read auth.json: {err}");
                LoginStatus::NotAuthenticated
            }
        }
    } else {
        LoginStatus::NotAuthenticated
    }
}

async fn load_config_or_exit(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
) -> Config {
    #[allow(clippy::print_stderr)]
    match Config::load_with_cli_overrides(cli_kv_overrides, overrides).await {
        Ok(config) => config,
        Err(err) => {
            eprintln!("Error loading configuration: {err}");
            std::process::exit(1);
        }
    }
}

/// Determine if user has configured a sandbox / approval policy,
/// or if the current cwd project is already trusted. If not, we need to
/// show the trust screen.
fn should_show_trust_screen(config: &Config) -> bool {
    if cfg!(target_os = "windows") && get_platform_sandbox().is_none() {
        // If the experimental sandbox is not enabled, Native Windows cannot enforce sandboxed write access; skip the trust prompt entirely.
        return false;
    }
    if config.did_user_set_custom_approval_policy_or_sandbox_mode {
        // Respect explicit approval/sandbox overrides made by the user.
        return false;
    }
    // otherwise, show only if no trust decision has been made
    config.active_project.trust_level.is_none()
}

fn should_show_onboarding(
    login_status: LoginStatus,
    config: &Config,
    show_trust_screen: bool,
) -> bool {
    if show_trust_screen {
        return true;
    }

    should_show_login_screen(login_status, config)
}

fn should_show_login_screen(login_status: LoginStatus, config: &Config) -> bool {
    // Only show the login screen for providers that actually require OpenAI auth
    // (OpenAI or equivalents). For OSS/other providers, skip login entirely.
    if !config.model_provider.requires_openai_auth {
        return false;
    }

    login_status == LoginStatus::NotAuthenticated
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ConfigToml;
    use codex_core::config::ProjectConfig;
    use serial_test::serial;
    use tempfile::TempDir;

    #[test]
    #[serial]
    fn windows_skips_trust_prompt_without_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            temp_dir.path().to_path_buf(),
        )?;
        config.did_user_set_custom_approval_policy_or_sandbox_mode = false;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_globally(false);

        let should_show = should_show_trust_screen(&config);
        if cfg!(target_os = "windows") {
            assert!(
                !should_show,
                "Windows trust prompt should always be skipped on native Windows"
            );
        } else {
            assert!(
                should_show,
                "Non-Windows should still show trust prompt when project is untrusted"
            );
        }
        Ok(())
    }
    #[test]
    #[serial]
    fn windows_shows_trust_prompt_with_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            temp_dir.path().to_path_buf(),
        )?;
        config.did_user_set_custom_approval_policy_or_sandbox_mode = false;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_globally(true);

        let should_show = should_show_trust_screen(&config);
        if cfg!(target_os = "windows") {
            assert!(
                should_show,
                "Windows trust prompt should be shown on native Windows with sandbox enabled"
            );
        } else {
            assert!(
                should_show,
                "Non-Windows should still show trust prompt when project is untrusted"
            );
        }
        Ok(())
    }
    #[test]
    fn untrusted_project_skips_trust_prompt() -> std::io::Result<()> {
        use codex_protocol::config_types::TrustLevel;
        let temp_dir = TempDir::new()?;
        let mut config = Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            temp_dir.path().to_path_buf(),
        )?;
        config.did_user_set_custom_approval_policy_or_sandbox_mode = false;
        config.active_project = ProjectConfig {
            trust_level: Some(TrustLevel::Untrusted),
        };

        let should_show = should_show_trust_screen(&config);
        assert!(
            !should_show,
            "Trust prompt should not be shown for projects explicitly marked as untrusted"
        );
        Ok(())
    }
}
