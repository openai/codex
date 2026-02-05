// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use additional_dirs::add_dir_warning_message;
use app::App;
pub use app::AppExitInfo;
pub use app::ExitReason;
use codex_app_server_protocol::AuthMode;
use codex_cloud_requirements::cloud_requirements_loader;
use codex_common::oss::ensure_oss_provider_ready;
use codex_common::oss::get_default_model_for_oss_provider;
use codex_core::AuthManager;
use codex_core::CodexAuth;
use codex_core::INTERACTIVE_SESSION_SOURCES;
use codex_core::RolloutRecorder;
use codex_core::ThreadSortKey;
use codex_core::auth::enforce_login_restrictions;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config::ConfigOverrides;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::resolve_oss_provider;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigLoadError;
use codex_core::config_loader::format_config_error_with_source;
use codex_core::default_client::set_default_client_residency_requirement;
use codex_core::find_thread_path_by_id_str;
use codex_core::find_thread_path_by_name_str;
use codex_core::path_utils;
use codex_core::protocol::AskForApproval;
use codex_core::read_session_meta_line;
use codex_core::terminal::Multiplexer;
use codex_core::windows_sandbox::WindowsSandboxLevelExt;
use codex_protocol::config_types::AltScreenMode;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::RolloutLine;
use codex_state::log_db;
use codex_utils_absolute_path::AbsolutePathBuf;
use cwd_prompt::CwdPromptAction;
use cwd_prompt::CwdSelection;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use tracing::error;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;
use uuid::Uuid;

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
mod collab;
mod collaboration_modes;
mod color;
pub mod custom_terminal;
mod cwd_prompt;
mod debug_config;
mod diff_render;
mod exec_cell;
mod exec_command;
mod external_editor;
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
mod notifications;
pub mod onboarding;
mod oss_selection;
mod pager_overlay;
pub mod public_widgets;
mod render;
mod resume_picker;
mod selection_list;
mod session_log;
mod shimmer;
mod skills_helpers;
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

use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::tui::Tui;
pub use cli::Cli;
pub use markdown_render::render_markdown_text;
pub use public_widgets::composer_input::ComposerAction;
pub use public_widgets::composer_input::ComposerInput;
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

    // Map the legacy --search flag to the canonical web_search mode.
    if cli.web_search {
        cli.config_overrides
            .raw_overrides
            .push("web_search=\"live\"".to_string());
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

    let cwd = cli.cwd.clone();
    let config_cwd = match cwd.as_deref() {
        Some(path) => AbsolutePathBuf::from_absolute_path(path.canonicalize()?)?,
        None => AbsolutePathBuf::current_dir()?,
    };

    #[allow(clippy::print_stderr)]
    let config_toml = match load_config_as_toml_with_cli_overrides(
        &codex_home,
        &config_cwd,
        cli_kv_overrides.clone(),
    )
    .await
    {
        Ok(config_toml) => config_toml,
        Err(err) => {
            let config_error = err
                .get_ref()
                .and_then(|err| err.downcast_ref::<ConfigLoadError>())
                .map(ConfigLoadError::config_error);
            if let Some(config_error) = config_error {
                eprintln!(
                    "Error loading config.toml:\n{}",
                    format_config_error_with_source(config_error)
                );
            } else {
                eprintln!("Error loading config.toml: {err}");
            }
            std::process::exit(1);
        }
    };

    if let Err(err) =
        codex_core::personality_migration::maybe_migrate_personality(&codex_home, &config_toml)
            .await
    {
        tracing::warn!(error = %err, "failed to run personality migration");
    }

    let cloud_auth_manager = AuthManager::shared(
        codex_home.to_path_buf(),
        false,
        config_toml.cli_auth_credentials_store.unwrap_or_default(),
    );
    let chatgpt_base_url = config_toml
        .chatgpt_base_url
        .clone()
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string());
    let cloud_requirements = cloud_requirements_loader(cloud_auth_manager, chatgpt_base_url);

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

    let additional_dirs = cli.add_dir.clone();

    let overrides = ConfigOverrides {
        model,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override.clone(),
        config_profile: cli.config_profile.clone(),
        codex_linux_sandbox_exe,
        show_raw_agent_reasoning: cli.oss.then_some(true),
        additional_writable_roots: additional_dirs,
        ..Default::default()
    };

    let config = load_config_or_exit(
        cli_kv_overrides.clone(),
        overrides.clone(),
        cloud_requirements.clone(),
    )
    .await;
    set_default_client_residency_requirement(config.enforce_residency.value());

    if let Some(warning) = add_dir_warning_message(&cli.add_dir, config.sandbox_policy.get()) {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("Error adding directories: {warning}");
            std::process::exit(1);
        }
    }

    #[allow(clippy::print_stderr)]
    if let Err(err) = enforce_login_restrictions(&config) {
        eprintln!("{err}");
        std::process::exit(1);
    }

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
        // `with_target(true)` is the default, but we previously disabled it for file output.
        // Keep it enabled so we can selectively enable targets via `RUST_LOG=...` and then
        // grep for a specific module/target while troubleshooting.
        .with_target(true)
        .with_ansi(false)
        .with_span_events(
            tracing_subscriber::fmt::format::FmtSpan::NEW
                | tracing_subscriber::fmt::format::FmtSpan::CLOSE,
        )
        .with_filter(env_filter());

    let feedback = codex_feedback::CodexFeedback::new();
    let feedback_layer = feedback.logger_layer();
    let feedback_metadata_layer = feedback.metadata_layer();

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

    let otel = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        codex_core::otel_init::build_provider(&config, env!("CARGO_PKG_VERSION"), None, true)
    })) {
        Ok(Ok(otel)) => otel,
        Ok(Err(e)) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("Could not create otel exporter: {e}");
            }
            None
        }
        Err(_) => {
            #[allow(clippy::print_stderr)]
            {
                eprintln!("Could not create otel exporter: panicked during initialization");
            }
            None
        }
    };

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let log_db_layer = codex_core::state_db::get_state_db(&config, None)
        .await
        .map(|db| log_db::start(db).with_filter(env_filter()));

    let _ = tracing_subscriber::registry()
        .with(file_layer)
        .with(feedback_layer)
        .with(feedback_metadata_layer)
        .with(log_db_layer)
        .with(otel_logger_layer)
        .with(otel_tracing_layer)
        .try_init();

    run_ratatui_app(
        cli,
        config,
        overrides,
        cli_kv_overrides,
        cloud_requirements,
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
    mut cloud_requirements: CloudRequirementsLoader,
    feedback: codex_feedback::CodexFeedback,
) -> color_eyre::Result<AppExitInfo> {
    color_eyre::install()?;

    tooltips::announcement::prewarm();

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
                        thread_id: None,
                        thread_name: None,
                        update_action: Some(action),
                        exit_reason: ExitReason::UserRequested,
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
    let should_show_trust_screen_flag = should_show_trust_screen(&initial_config);
    let should_show_onboarding =
        should_show_onboarding(login_status, &initial_config, should_show_trust_screen_flag);

    let config = if should_show_onboarding {
        let show_login_screen = should_show_login_screen(login_status, &initial_config);
        let onboarding_result = run_onboarding_app(
            OnboardingScreenArgs {
                show_login_screen,
                show_trust_screen: should_show_trust_screen_flag,
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
                thread_id: None,
                thread_name: None,
                update_action: None,
                exit_reason: ExitReason::UserRequested,
            });
        }
        // If this onboarding run included the login step, always refresh cloud requirements and
        // rebuild config. This avoids missing newly available cloud requirements due to login
        // status detection edge cases.
        if show_login_screen {
            cloud_requirements = cloud_requirements_loader(
                auth_manager.clone(),
                initial_config.chatgpt_base_url.clone(),
            );
        }

        // If the user made an explicit trust decision, or we showed the login flow, reload config
        // so current process state reflects persisted trust/auth changes.
        if onboarding_result.directory_trust_decision.is_some() || show_login_screen {
            load_config_or_exit(
                cli_kv_overrides.clone(),
                overrides.clone(),
                cloud_requirements.clone(),
            )
            .await
        } else {
            initial_config
        }
    } else {
        initial_config
    };
    let mut missing_session_exit = |id_str: &str, action: &str| {
        error!("Error finding conversation path: {id_str}");
        restore();
        session_log::log_session_end();
        let _ = tui.terminal.clear();
        Ok(AppExitInfo {
            token_usage: codex_core::protocol::TokenUsage::default(),
            thread_id: None,
            thread_name: None,
            update_action: None,
            exit_reason: ExitReason::Fatal(format!(
                "No saved session found with ID {id_str}. Run `codex {action}` without an ID to choose from existing sessions."
            )),
        })
    };

    let use_fork = cli.fork_picker || cli.fork_last || cli.fork_session_id.is_some();
    let session_selection = if use_fork {
        if let Some(id_str) = cli.fork_session_id.as_deref() {
            let is_uuid = Uuid::parse_str(id_str).is_ok();
            let path = if is_uuid {
                find_thread_path_by_id_str(&config.codex_home, id_str).await?
            } else {
                find_thread_path_by_name_str(&config.codex_home, id_str).await?
            };
            match path {
                Some(path) => resume_picker::SessionSelection::Fork(path),
                None => return missing_session_exit(id_str, "fork"),
            }
        } else if cli.fork_last {
            let provider_filter = vec![config.model_provider_id.clone()];
            match RolloutRecorder::list_threads(
                &config.codex_home,
                1,
                None,
                ThreadSortKey::UpdatedAt,
                INTERACTIVE_SESSION_SOURCES,
                Some(provider_filter.as_slice()),
                &config.model_provider_id,
            )
            .await
            {
                Ok(page) => page
                    .items
                    .first()
                    .map(|it| resume_picker::SessionSelection::Fork(it.path.clone()))
                    .unwrap_or(resume_picker::SessionSelection::StartFresh),
                Err(_) => resume_picker::SessionSelection::StartFresh,
            }
        } else if cli.fork_picker {
            match resume_picker::run_fork_picker(
                &mut tui,
                &config.codex_home,
                &config.model_provider_id,
                cli.fork_show_all,
            )
            .await?
            {
                resume_picker::SessionSelection::Exit => {
                    restore();
                    session_log::log_session_end();
                    return Ok(AppExitInfo {
                        token_usage: codex_core::protocol::TokenUsage::default(),
                        thread_id: None,
                        thread_name: None,
                        update_action: None,
                        exit_reason: ExitReason::UserRequested,
                    });
                }
                other => other,
            }
        } else {
            resume_picker::SessionSelection::StartFresh
        }
    } else if let Some(id_str) = cli.resume_session_id.as_deref() {
        let is_uuid = Uuid::parse_str(id_str).is_ok();
        let path = if is_uuid {
            find_thread_path_by_id_str(&config.codex_home, id_str).await?
        } else {
            find_thread_path_by_name_str(&config.codex_home, id_str).await?
        };
        match path {
            Some(path) => resume_picker::SessionSelection::Resume(path),
            None => return missing_session_exit(id_str, "resume"),
        }
    } else if cli.resume_last {
        let provider_filter = vec![config.model_provider_id.clone()];
        let filter_cwd = if cli.resume_show_all {
            None
        } else {
            Some(config.cwd.as_path())
        };
        match RolloutRecorder::find_latest_thread_path(
            &config.codex_home,
            1,
            None,
            ThreadSortKey::UpdatedAt,
            INTERACTIVE_SESSION_SOURCES,
            Some(provider_filter.as_slice()),
            &config.model_provider_id,
            filter_cwd,
        )
        .await
        {
            Ok(Some(path)) => resume_picker::SessionSelection::Resume(path),
            _ => resume_picker::SessionSelection::StartFresh,
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
            resume_picker::SessionSelection::Exit => {
                restore();
                session_log::log_session_end();
                return Ok(AppExitInfo {
                    token_usage: codex_core::protocol::TokenUsage::default(),
                    thread_id: None,
                    thread_name: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
            other => other,
        }
    } else {
        resume_picker::SessionSelection::StartFresh
    };

    let current_cwd = config.cwd.clone();
    let allow_prompt = cli.cwd.is_none();
    let action_and_path_if_resume_or_fork = match &session_selection {
        resume_picker::SessionSelection::Resume(path) => Some((CwdPromptAction::Resume, path)),
        resume_picker::SessionSelection::Fork(path) => Some((CwdPromptAction::Fork, path)),
        _ => None,
    };
    let fallback_cwd = match action_and_path_if_resume_or_fork {
        Some((action, path)) => {
            resolve_cwd_for_resume_or_fork(&mut tui, &current_cwd, path, action, allow_prompt)
                .await?
        }
        None => None,
    };

    let config = match &session_selection {
        resume_picker::SessionSelection::Resume(_) | resume_picker::SessionSelection::Fork(_) => {
            load_config_or_exit_with_fallback_cwd(
                cli_kv_overrides.clone(),
                overrides.clone(),
                cloud_requirements.clone(),
                fallback_cwd,
            )
            .await
        }
        _ => config,
    };
    set_default_client_residency_requirement(config.enforce_residency.value());
    let active_profile = config.active_profile.clone();
    let should_show_trust_screen = should_show_trust_screen(&config);

    let Cli {
        prompt,
        images,
        no_alt_screen,
        ..
    } = cli;

    let use_alt_screen = determine_alt_screen_mode(no_alt_screen, config.tui_alternate_screen);
    tui.set_alt_screen_enabled(use_alt_screen);

    let app_result = App::run(
        &mut tui,
        auth_manager,
        config,
        cli_kv_overrides.clone(),
        overrides.clone(),
        active_profile,
        prompt,
        images,
        session_selection,
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

pub(crate) async fn read_session_cwd(path: &Path) -> Option<PathBuf> {
    // Prefer the latest TurnContext cwd so resume/fork reflects the most recent
    // session directory (for the changed-cwd prompt). The alternative would be
    // mutating the SessionMeta line when the session cwd changes, but the rollout
    // is an append-only JSONL log and rewriting the head would be error-prone.
    // When rollouts move to SQLite, we can drop this scan.
    if let Some(cwd) = parse_latest_turn_context_cwd(path).await {
        return Some(cwd);
    }
    match read_session_meta_line(path).await {
        Ok(meta_line) => Some(meta_line.meta.cwd),
        Err(err) => {
            let rollout_path = path.display().to_string();
            tracing::warn!(
                %rollout_path,
                %err,
                "Failed to read session metadata from rollout"
            );
            None
        }
    }
}

async fn parse_latest_turn_context_cwd(path: &Path) -> Option<PathBuf> {
    let text = tokio::fs::read_to_string(path).await.ok()?;
    for line in text.lines().rev() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(rollout_line) = serde_json::from_str::<RolloutLine>(trimmed) else {
            continue;
        };
        if let RolloutItem::TurnContext(item) = rollout_line.item {
            return Some(item.cwd);
        }
    }
    None
}

pub(crate) fn cwds_differ(current_cwd: &Path, session_cwd: &Path) -> bool {
    match (
        path_utils::normalize_for_path_comparison(current_cwd),
        path_utils::normalize_for_path_comparison(session_cwd),
    ) {
        (Ok(current), Ok(session)) => current != session,
        _ => current_cwd != session_cwd,
    }
}

pub(crate) async fn resolve_cwd_for_resume_or_fork(
    tui: &mut Tui,
    current_cwd: &Path,
    path: &Path,
    action: CwdPromptAction,
    allow_prompt: bool,
) -> color_eyre::Result<Option<PathBuf>> {
    let Some(history_cwd) = read_session_cwd(path).await else {
        return Ok(None);
    };
    if allow_prompt && cwds_differ(current_cwd, &history_cwd) {
        let selection =
            cwd_prompt::run_cwd_selection_prompt(tui, action, current_cwd, &history_cwd).await?;
        return Ok(Some(match selection {
            CwdSelection::Current => current_cwd.to_path_buf(),
            CwdSelection::Session => history_cwd,
        }));
    }
    Ok(Some(history_cwd))
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

/// Determine whether to use the terminal's alternate screen buffer.
///
/// The alternate screen buffer provides a cleaner fullscreen experience without polluting
/// the terminal's scrollback history. However, it conflicts with terminal multiplexers like
/// Zellij that strictly follow the xterm spec, which disallows scrollback in alternate screen
/// buffers. Zellij intentionally disables scrollback in alternate screen mode (see
/// https://github.com/zellij-org/zellij/pull/1032) and offers no configuration option to
/// change this behavior.
///
/// This function implements a pragmatic workaround:
/// - If `--no-alt-screen` is explicitly passed, always disable alternate screen
/// - Otherwise, respect the `tui.alternate_screen` config setting:
///   - `always`: Use alternate screen everywhere (original behavior)
///   - `never`: Inline mode only, preserves scrollback
///   - `auto` (default): Auto-detect the terminal multiplexer and disable alternate screen
///     only in Zellij, enabling it everywhere else
fn determine_alt_screen_mode(no_alt_screen: bool, tui_alternate_screen: AltScreenMode) -> bool {
    if no_alt_screen {
        false
    } else {
        match tui_alternate_screen {
            AltScreenMode::Always => true,
            AltScreenMode::Never => false,
            AltScreenMode::Auto => {
                let terminal_info = codex_core::terminal::terminal_info();
                !matches!(terminal_info.multiplexer, Some(Multiplexer::Zellij { .. }))
            }
        }
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
            Ok(Some(auth)) => LoginStatus::AuthMode(auth.api_auth_mode()),
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
    cloud_requirements: CloudRequirementsLoader,
) -> Config {
    load_config_or_exit_with_fallback_cwd(cli_kv_overrides, overrides, cloud_requirements, None)
        .await
}

async fn load_config_or_exit_with_fallback_cwd(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
    cloud_requirements: CloudRequirementsLoader,
    fallback_cwd: Option<PathBuf>,
) -> Config {
    #[allow(clippy::print_stderr)]
    match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .cloud_requirements(cloud_requirements)
        .fallback_cwd(fallback_cwd)
        .build()
        .await
    {
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
    if cfg!(target_os = "windows")
        && WindowsSandboxLevel::from_config(config) == WindowsSandboxLevel::Disabled
    {
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
    use codex_core::config::ConfigBuilder;
    use codex_core::config::ConfigOverrides;
    use codex_core::config::ProjectConfig;
    use codex_core::protocol::AskForApproval;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::RolloutLine;
    use codex_protocol::protocol::SessionMeta;
    use codex_protocol::protocol::SessionMetaLine;
    use codex_protocol::protocol::TurnContextItem;
    use serial_test::serial;
    use tempfile::TempDir;

    async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
        ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await
    }

    #[tokio::test]
    #[serial]
    async fn windows_skips_trust_prompt_without_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        config.did_user_set_custom_approval_policy_or_sandbox_mode = false;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_enabled(false);

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
    #[tokio::test]
    #[serial]
    async fn windows_shows_trust_prompt_with_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        config.did_user_set_custom_approval_policy_or_sandbox_mode = false;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_enabled(true);

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
    #[tokio::test]
    async fn untrusted_project_skips_trust_prompt() -> std::io::Result<()> {
        use codex_protocol::config_types::TrustLevel;
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
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

    fn build_turn_context(config: &Config, cwd: PathBuf) -> TurnContextItem {
        let model = config
            .model
            .clone()
            .unwrap_or_else(|| "gpt-5.1".to_string());
        TurnContextItem {
            cwd,
            approval_policy: config.approval_policy.value(),
            sandbox_policy: config.sandbox_policy.get().clone(),
            model,
            personality: None,
            collaboration_mode: None,
            effort: config.model_reasoning_effort,
            summary: config.model_reasoning_summary,
            user_instructions: None,
            developer_instructions: None,
            final_output_json_schema: None,
            truncation_policy: None,
        }
    }

    #[tokio::test]
    async fn read_session_cwd_prefers_latest_turn_context() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let first = temp_dir.path().join("first");
        let second = temp_dir.path().join("second");
        std::fs::create_dir_all(&first)?;
        std::fs::create_dir_all(&second)?;

        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let lines = vec![
            RolloutLine {
                timestamp: "t0".to_string(),
                item: RolloutItem::TurnContext(build_turn_context(&config, first)),
            },
            RolloutLine {
                timestamp: "t1".to_string(),
                item: RolloutItem::TurnContext(build_turn_context(&config, second.clone())),
            },
        ];
        let mut text = String::new();
        for line in lines {
            text.push_str(&serde_json::to_string(&line).expect("serialize rollout"));
            text.push('\n');
        }
        std::fs::write(&rollout_path, text)?;

        let cwd = read_session_cwd(&rollout_path).await.expect("expected cwd");
        assert_eq!(cwd, second);
        Ok(())
    }

    #[tokio::test]
    async fn should_prompt_when_meta_matches_current_but_latest_turn_differs() -> std::io::Result<()>
    {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let current = temp_dir.path().join("current");
        let latest = temp_dir.path().join("latest");
        std::fs::create_dir_all(&current)?;
        std::fs::create_dir_all(&latest)?;

        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let session_meta = SessionMeta {
            cwd: current.clone(),
            ..SessionMeta::default()
        };
        let lines = vec![
            RolloutLine {
                timestamp: "t0".to_string(),
                item: RolloutItem::SessionMeta(SessionMetaLine {
                    meta: session_meta,
                    git: None,
                }),
            },
            RolloutLine {
                timestamp: "t1".to_string(),
                item: RolloutItem::TurnContext(build_turn_context(&config, latest.clone())),
            },
        ];
        let mut text = String::new();
        for line in lines {
            text.push_str(&serde_json::to_string(&line).expect("serialize rollout"));
            text.push('\n');
        }
        std::fs::write(&rollout_path, text)?;

        let session_cwd = read_session_cwd(&rollout_path).await.expect("expected cwd");
        assert_eq!(session_cwd, latest);
        assert!(cwds_differ(&current, &session_cwd));
        Ok(())
    }

    #[tokio::test]
    async fn config_rebuild_changes_trust_defaults_with_cwd() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let codex_home = temp_dir.path().to_path_buf();
        let trusted = temp_dir.path().join("trusted");
        let untrusted = temp_dir.path().join("untrusted");
        std::fs::create_dir_all(&trusted)?;
        std::fs::create_dir_all(&untrusted)?;

        // TOML keys need escaped backslashes on Windows paths.
        let trusted_display = trusted.display().to_string().replace('\\', "\\\\");
        let untrusted_display = untrusted.display().to_string().replace('\\', "\\\\");
        let config_toml = format!(
            r#"[projects."{trusted_display}"]
trust_level = "trusted"

[projects."{untrusted_display}"]
trust_level = "untrusted"
"#
        );
        std::fs::write(temp_dir.path().join("config.toml"), config_toml)?;

        let trusted_overrides = ConfigOverrides {
            cwd: Some(trusted.clone()),
            ..Default::default()
        };
        let trusted_config = ConfigBuilder::default()
            .codex_home(codex_home.clone())
            .harness_overrides(trusted_overrides.clone())
            .build()
            .await?;
        assert_eq!(
            trusted_config.approval_policy.value(),
            AskForApproval::OnRequest
        );

        let untrusted_overrides = ConfigOverrides {
            cwd: Some(untrusted),
            ..trusted_overrides
        };
        let untrusted_config = ConfigBuilder::default()
            .codex_home(codex_home)
            .harness_overrides(untrusted_overrides)
            .build()
            .await?;
        assert_eq!(
            untrusted_config.approval_policy.value(),
            AskForApproval::UnlessTrusted
        );
        Ok(())
    }

    #[tokio::test]
    async fn read_session_cwd_falls_back_to_session_meta() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let _config = build_config(&temp_dir).await?;
        let session_cwd = temp_dir.path().join("session");
        std::fs::create_dir_all(&session_cwd)?;

        let rollout_path = temp_dir.path().join("rollout.jsonl");
        let session_meta = SessionMeta {
            cwd: session_cwd.clone(),
            ..SessionMeta::default()
        };
        let meta_line = RolloutLine {
            timestamp: "t0".to_string(),
            item: RolloutItem::SessionMeta(SessionMetaLine {
                meta: session_meta,
                git: None,
            }),
        };
        let text = format!(
            "{}\n",
            serde_json::to_string(&meta_line).expect("serialize meta")
        );
        std::fs::write(&rollout_path, text)?;

        let cwd = read_session_cwd(&rollout_path).await.expect("expected cwd");
        assert_eq!(cwd, session_cwd);
        Ok(())
    }
}
