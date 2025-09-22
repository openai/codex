// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use app::App;
pub use app::AppExitInfo;
use codex_core::AuthManager;
use codex_core::BUILT_IN_OSS_MODEL_PROVIDER_ID;
use codex_core::CodexAuth;
use codex_core::RolloutRecorder;
use codex_core::config::Config;
use codex_core::config::ConfigOverrides;
use codex_core::config::ConfigToml;
use codex_core::config::GPT_5_CODEX_MEDIUM_MODEL;
use codex_core::config::find_codex_home;
use codex_core::config::load_config_as_toml_with_cli_overrides;
use codex_core::config::persist_model_selection;
use codex_core::find_conversation_path_by_id_str;
use codex_core::protocol::AskForApproval;
use codex_core::protocol::SandboxPolicy;
use codex_ollama::DEFAULT_OSS_MODEL;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::mcp_protocol::AuthMode;
use std::fs::OpenOptions;
use std::path::PathBuf;
use tracing::error;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::prelude::*;

mod app;
mod app_backtrack;
mod app_event;
mod app_event_sender;
mod ascii_animation;
mod bottom_pane;
mod chatwidget;
mod citation_regex;
mod cli;
mod clipboard_paste;
pub mod custom_terminal;
mod diff_render;
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
mod new_model_popup;
pub mod onboarding;
mod pager_overlay;
mod render;
mod resume_picker;
mod session_log;
mod shimmer;
mod slash_command;
mod status_indicator_widget;
mod streaming;
mod text_formatting;
mod tui;
mod ui_consts;
mod user_approval_widget;
mod version;
mod wrapping;

#[cfg(not(debug_assertions))]
mod updates;

use crate::new_model_popup::ModelUpgradeDecision;
use crate::new_model_popup::run_model_upgrade_popup;
use crate::onboarding::TrustDirectorySelection;
use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::tui::Tui;
pub use cli::Cli;
use codex_core::internal_storage::InternalStorage;

// (tests access modules directly within the crate)

pub async fn run_main(
    cli: Cli,
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

    // When using `--oss`, let the bootstrapper pick the model (defaulting to
    // gpt-oss:20b) and ensure it is present locally. Also, force the built‑in
    // `oss` model provider.
    let model = if let Some(model) = &cli.model {
        Some(model.clone())
    } else if cli.oss {
        Some(DEFAULT_OSS_MODEL.to_owned())
    } else {
        None // No model specified, will use the default.
    };

    let model_provider_override = if cli.oss {
        Some(BUILT_IN_OSS_MODEL_PROVIDER_ID.to_owned())
    } else {
        None
    };

    // canonicalize the cwd
    let cwd = cli.cwd.clone().map(|p| p.canonicalize().unwrap_or(p));

    let overrides = ConfigOverrides {
        model,
        review_model: None,
        approval_policy,
        sandbox_mode,
        cwd,
        model_provider: model_provider_override,
        config_profile: cli.config_profile.clone(),
        codex_linux_sandbox_exe,
        base_instructions: None,
        include_plan_tool: Some(true),
        include_apply_patch_tool: None,
        include_view_image_tool: None,
        show_raw_agent_reasoning: cli.oss.then_some(true),
        tools_web_search_request: cli.web_search.then_some(true),
    };
    let raw_overrides = cli.config_overrides.raw_overrides.clone();
    let overrides_cli = codex_common::CliConfigOverrides { raw_overrides };
    let cli_kv_overrides = match overrides_cli.parse_overrides() {
        Ok(v) => v,
        #[allow(clippy::print_stderr)]
        Err(e) => {
            eprintln!("Error parsing -c overrides: {e}");
            std::process::exit(1);
        }
    };

    let mut config = {
        // Load configuration and support CLI overrides.

        #[allow(clippy::print_stderr)]
        match Config::load_with_cli_overrides(cli_kv_overrides.clone(), overrides) {
            Ok(config) => config,
            Err(err) => {
                eprintln!("Error loading configuration: {err}");
                std::process::exit(1);
            }
        }
    };

    // we load config.toml here to determine project state.
    #[allow(clippy::print_stderr)]
    let config_toml = {
        let codex_home = match find_codex_home() {
            Ok(codex_home) => codex_home,
            Err(err) => {
                eprintln!("Error finding codex home: {err}");
                std::process::exit(1);
            }
        };

        match load_config_as_toml_with_cli_overrides(&codex_home, cli_kv_overrides) {
            Ok(config_toml) => config_toml,
            Err(err) => {
                eprintln!("Error loading config.toml: {err}");
                std::process::exit(1);
            }
        }
    };

    let cli_profile_override = cli.config_profile.clone();
    let active_profile = cli_profile_override
        .clone()
        .or_else(|| config_toml.profile.clone());

    let should_show_trust_screen = determine_repo_trust_state(
        &mut config,
        &config_toml,
        approval_policy,
        sandbox_mode,
        cli_profile_override,
    )?;

    let internal_storage = InternalStorage::load(&config.codex_home);

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
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_filter(env_filter());

    if cli.oss {
        codex_ollama::ensure_oss_ready(&config)
            .await
            .map_err(|e| std::io::Error::other(format!("OSS setup failed: {e}")))?;
    }

    let _ = tracing_subscriber::registry().with(file_layer).try_init();

    run_ratatui_app(
        cli,
        config,
        internal_storage,
        active_profile,
        should_show_trust_screen,
    )
    .await
    .map_err(|err| std::io::Error::other(err.to_string()))
}

async fn run_ratatui_app(
    cli: Cli,
    config: Config,
    mut internal_storage: InternalStorage,
    active_profile: Option<String>,
    should_show_trust_screen: bool,
) -> color_eyre::Result<AppExitInfo> {
    let mut config = config;
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

    // Show update banner in terminal history (instead of stderr) so it is visible
    // within the TUI scrollback. Building spans keeps styling consistent.
    #[cfg(not(debug_assertions))]
    if let Some(latest_version) = updates::get_upgrade_version(&config) {
        use ratatui::style::Stylize as _;
        use ratatui::text::Line;

        let current_version = env!("CARGO_PKG_VERSION");
        let exe = std::env::current_exe()?;
        let managed_by_npm = std::env::var_os("CODEX_MANAGED_BY_NPM").is_some();

        let mut lines: Vec<Line<'static>> = Vec::new();
        lines.push(Line::from(vec![
            "✨⬆️ Update available!".bold().cyan(),
            " ".into(),
            format!("{current_version} -> {latest_version}.").into(),
        ]));

        if managed_by_npm {
            let npm_cmd = "npm install -g @openai/codex@latest";
            lines.push(Line::from(vec![
                "Run ".into(),
                npm_cmd.cyan(),
                " to update.".into(),
            ]));
        } else if cfg!(target_os = "macos")
            && (exe.starts_with("/opt/homebrew") || exe.starts_with("/usr/local"))
        {
            let brew_cmd = "brew upgrade codex";
            lines.push(Line::from(vec![
                "Run ".into(),
                brew_cmd.cyan(),
                " to update.".into(),
            ]));
        } else {
            lines.push(Line::from(vec![
                "See ".into(),
                "https://github.com/openai/codex/releases/latest".cyan(),
                " for the latest releases and installation options.".into(),
            ]));
        }

        lines.push("".into());
        tui.insert_history_lines(lines);
    }

    // Initialize high-fidelity session event logging if enabled.
    session_log::maybe_init(&config);

    let auth_manager = AuthManager::shared(config.codex_home.clone());
    let login_status = get_login_status(&config);
    let should_show_onboarding =
        should_show_onboarding(login_status, &config, should_show_trust_screen);
    if should_show_onboarding {
        let directory_trust_decision = run_onboarding_app(
            OnboardingScreenArgs {
                show_login_screen: should_show_login_screen(login_status, &config),
                show_trust_screen: should_show_trust_screen,
                login_status,
                auth_manager: auth_manager.clone(),
                config: config.clone(),
            },
            &mut tui,
        )
        .await?;
        if let Some(TrustDirectorySelection::Trust) = directory_trust_decision {
            config.approval_policy = AskForApproval::OnRequest;
            config.sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        }
    }

    // Determine resume behavior: explicit id, then resume last, then picker.
    let resume_selection = if let Some(id_str) = cli.resume_session_id.as_deref() {
        match find_conversation_path_by_id_str(&config.codex_home, id_str).await? {
            Some(path) => resume_picker::ResumeSelection::Resume(path),
            None => {
                error!("Error finding conversation path: {id_str}");
                resume_picker::ResumeSelection::StartFresh
            }
        }
    } else if cli.resume_last {
        match RolloutRecorder::list_conversations(&config.codex_home, 1, None).await {
            Ok(page) => page
                .items
                .first()
                .map(|it| resume_picker::ResumeSelection::Resume(it.path.clone()))
                .unwrap_or(resume_picker::ResumeSelection::StartFresh),
            Err(_) => resume_picker::ResumeSelection::StartFresh,
        }
    } else if cli.resume_picker {
        match resume_picker::run_resume_picker(&mut tui, &config.codex_home).await? {
            resume_picker::ResumeSelection::Exit => {
                restore();
                session_log::log_session_end();
                return Ok(AppExitInfo {
                    token_usage: codex_core::protocol::TokenUsage::default(),
                    conversation_id: None,
                });
            }
            other => other,
        }
    } else {
        resume_picker::ResumeSelection::StartFresh
    };

    if should_show_model_rollout_prompt(
        &cli,
        &config,
        active_profile.as_deref(),
        internal_storage.gpt_5_codex_model_prompt_seen,
    ) {
        internal_storage.gpt_5_codex_model_prompt_seen = true;
        if let Err(e) = internal_storage.persist().await {
            error!("Failed to persist internal storage: {e:?}");
        }

        let upgrade_decision = run_model_upgrade_popup(&mut tui).await?;
        let switch_to_new_model = upgrade_decision == ModelUpgradeDecision::Switch;

        if switch_to_new_model {
            config.model = GPT_5_CODEX_MEDIUM_MODEL.to_owned();
            config.model_reasoning_effort = None;
            if let Err(e) = persist_model_selection(
                &config.codex_home,
                active_profile.as_deref(),
                &config.model,
                config.model_reasoning_effort,
            )
            .await
            {
                error!("Failed to persist model selection: {e:?}");
            }
        }
    }

    let Cli { prompt, images, .. } = cli;

    let app_result = App::run(
        &mut tui,
        auth_manager,
        config,
        active_profile,
        prompt,
        images,
        resume_selection,
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
        match CodexAuth::from_codex_home(&codex_home) {
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

/// Determine if user has configured a sandbox / approval policy,
/// or if the current cwd project is trusted, and updates the config
/// accordingly.
fn determine_repo_trust_state(
    config: &mut Config,
    config_toml: &ConfigToml,
    approval_policy_overide: Option<AskForApproval>,
    sandbox_mode_override: Option<SandboxMode>,
    config_profile_override: Option<String>,
) -> std::io::Result<bool> {
    let config_profile = config_toml.get_config_profile(config_profile_override)?;

    if approval_policy_overide.is_some() || sandbox_mode_override.is_some() {
        // if the user has overridden either approval policy or sandbox mode,
        // skip the trust flow
        Ok(false)
    } else if config_profile.approval_policy.is_some() {
        // if the user has specified settings in a config profile, skip the trust flow
        // todo: profile sandbox mode?
        Ok(false)
    } else if config_toml.approval_policy.is_some() || config_toml.sandbox_mode.is_some() {
        // if the user has specified either approval policy or sandbox mode in config.toml
        // skip the trust flow
        Ok(false)
    } else if config_toml.is_cwd_trusted(&config.cwd) {
        // if the current cwd project is trusted and no config has been set
        // skip the trust flow and set the approval policy and sandbox mode
        config.approval_policy = AskForApproval::OnRequest;
        config.sandbox_policy = SandboxPolicy::new_workspace_write_policy();
        Ok(false)
    } else {
        // if none of the above conditions are met, show the trust screen
        Ok(true)
    }
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

fn should_show_model_rollout_prompt(
    cli: &Cli,
    config: &Config,
    active_profile: Option<&str>,
    gpt_5_codex_model_prompt_seen: bool,
) -> bool {
    let login_status = get_login_status(config);

    active_profile.is_none()
        && cli.model.is_none()
        && !gpt_5_codex_model_prompt_seen
        && config.model_provider.requires_openai_auth
        && matches!(login_status, LoginStatus::AuthMode(AuthMode::ChatGPT))
        && !cli.oss
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::Parser;
    use codex_core::auth::AuthDotJson;
    use codex_core::auth::get_auth_file;
    use codex_core::auth::login_with_api_key;
    use codex_core::auth::write_auth_json;
    use codex_core::token_data::IdTokenInfo;
    use codex_core::token_data::TokenData;
    use std::sync::atomic::AtomicUsize;
    use std::sync::atomic::Ordering;

    fn get_next_codex_home() -> PathBuf {
        static NEXT_CODEX_HOME_ID: AtomicUsize = AtomicUsize::new(0);
        let mut codex_home = std::env::temp_dir();
        let unique_suffix = format!(
            "codex_tui_test_{}_{}",
            std::process::id(),
            NEXT_CODEX_HOME_ID.fetch_add(1, Ordering::Relaxed)
        );
        codex_home.push(unique_suffix);
        codex_home
    }

    fn make_config() -> Config {
        // Create a unique CODEX_HOME per test to isolate auth.json writes.
        let codex_home = get_next_codex_home();
        std::fs::create_dir_all(&codex_home).expect("create unique CODEX_HOME");

        Config::load_from_base_config_with_overrides(
            ConfigToml::default(),
            ConfigOverrides::default(),
            codex_home,
        )
        .expect("load default config")
    }

    /// Test helper to write an `auth.json` with the requested auth mode into the
    /// provided CODEX_HOME directory. This ensures `get_login_status()` reads the
    /// intended mode deterministically.
    fn set_auth_method(codex_home: &std::path::Path, mode: AuthMode) {
        match mode {
            AuthMode::ApiKey => {
                login_with_api_key(codex_home, "sk-test-key").expect("write api key auth.json");
            }
            AuthMode::ChatGPT => {
                // Minimal valid JWT payload: header.payload.signature (all base64url, no padding)
                const FAKE_JWT: &str = "eyJhbGciOiJub25lIiwidHlwIjoiSldUIn0.e30.c2ln"; // {"alg":"none","typ":"JWT"}.{}."sig"
                let mut id_info = IdTokenInfo::default();
                id_info.raw_jwt = FAKE_JWT.to_string();
                let auth = AuthDotJson {
                    openai_api_key: None,
                    tokens: Some(TokenData {
                        id_token: id_info,
                        access_token: "access-token".to_string(),
                        refresh_token: "refresh-token".to_string(),
                        account_id: None,
                    }),
                    last_refresh: None,
                };
                let file = get_auth_file(codex_home);
                write_auth_json(&file, &auth).expect("write chatgpt auth.json");
            }
        }
    }

    #[test]
    fn shows_login_when_not_authenticated() {
        let cfg = make_config();
        assert!(should_show_login_screen(
            LoginStatus::NotAuthenticated,
            &cfg
        ));
    }

    #[test]
    fn shows_model_rollout_prompt_for_default_model() {
        let cli = Cli::parse_from(["codex"]);
        let cfg = make_config();
        set_auth_method(&cfg.codex_home, AuthMode::ChatGPT);
        assert!(should_show_model_rollout_prompt(&cli, &cfg, None, false,));
    }

    #[test]
    fn hides_model_rollout_prompt_when_api_auth_mode() {
        let cli = Cli::parse_from(["codex"]);
        let cfg = make_config();
        set_auth_method(&cfg.codex_home, AuthMode::ApiKey);
        assert!(!should_show_model_rollout_prompt(&cli, &cfg, None, false,));
    }

    #[test]
    fn hides_model_rollout_prompt_when_marked_seen() {
        let cli = Cli::parse_from(["codex"]);
        let cfg = make_config();
        set_auth_method(&cfg.codex_home, AuthMode::ChatGPT);
        assert!(!should_show_model_rollout_prompt(&cli, &cfg, None, true,));
    }

    #[test]
    fn hides_model_rollout_prompt_when_cli_overrides_model() {
        let cli = Cli::parse_from(["codex", "--model", "gpt-4.1"]);
        let cfg = make_config();
        set_auth_method(&cfg.codex_home, AuthMode::ChatGPT);
        assert!(!should_show_model_rollout_prompt(&cli, &cfg, None, false,));
    }

    #[test]
    fn hides_model_rollout_prompt_when_profile_active() {
        let cli = Cli::parse_from(["codex"]);
        let cfg = make_config();
        set_auth_method(&cfg.codex_home, AuthMode::ChatGPT);
        assert!(!should_show_model_rollout_prompt(
            &cli,
            &cfg,
            Some("gpt5"),
            false,
        ));
    }
}
