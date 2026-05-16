// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use crate::legacy_core::check_execpolicy_for_warnings;
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::load_config_as_toml_with_cli_and_load_options;
use crate::legacy_core::config::resolve_oss_provider;
use crate::legacy_core::config::resolve_profile_v2_config_path;
use crate::legacy_core::format_exec_policy_error_with_source;
use crate::legacy_core::windows_sandbox::WindowsSandboxLevelExt;
use crate::session_resume::ResolveCwdOutcome;
use crate::session_resume::resolve_cwd_for_resume_or_fork;
pub use crate::startup_error::LocalStateDbStartupError;
use additional_dirs::add_dir_warning_message;
use app::App;
pub use app::AppExitInfo;
pub use app::ExitReason;
use app_server_session::AppServerSession;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
pub use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::Account as AppServerAccount;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::AuthMode as AppServerAuthMode;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey as AppServerThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_cloud_requirements::cloud_requirements_loader_for_storage;
use codex_config::CloudRequirementsLoader;
use codex_config::ConfigLoadError;
use codex_config::LoaderOverrides;
use codex_config::format_config_error_with_source;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_login::AuthConfig;
use codex_login::default_client::originator;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::enforce_login_restrictions;
use codex_protocol::ThreadId;
use codex_protocol::config_types::AltScreenMode;
use codex_protocol::config_types::SandboxMode;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_rollout::StateDbHandle;
use codex_rollout::state_db;
use codex_state::log_db;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::canonicalize_existing_preserving_symlinks;
use codex_utils_home_dir::find_codex_home;
use codex_utils_oss::ensure_oss_provider_ready;
use codex_utils_oss::get_default_model_for_oss_provider;
use color_eyre::eyre::WrapErr;
use cwd_prompt::CwdPromptAction;
use std::fs::OpenOptions;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
pub use token_usage::TokenUsage;
use tracing::Level;
use tracing::error;
use tracing::warn;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::filter::Targets;
use tracing_subscriber::prelude::*;
use url::Url;
use uuid::Uuid;

pub(crate) use codex_app_server_client::legacy_core;

mod additional_dirs;
mod app;
mod app_backtrack;
mod app_command;
mod app_event;
mod app_event_sender;
mod app_server_approval_conversions;
mod app_server_session;
mod approval_events;
mod ascii_animation;
#[cfg(not(target_os = "linux"))]
mod audio_device;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod audio_device {
    use crate::app_event::RealtimeAudioDeviceKind;

    pub(crate) fn list_realtime_audio_device_names(
        kind: RealtimeAudioDeviceKind,
    ) -> Result<Vec<String>, String> {
        Err(format!(
            "Failed to load realtime {} devices: voice input is unavailable in this build",
            kind.noun()
        ))
    }
}
mod bottom_pane;
mod branch_summary;
mod chatwidget;
mod cli;
mod clipboard_copy;
mod clipboard_paste;
mod collaboration_modes;
mod color;
mod config_update;
pub(crate) mod custom_terminal;
mod pets;
pub use custom_terminal::Terminal;
mod auto_review_denials;
mod cwd_prompt;
mod debug_config;
mod diff_model;
mod diff_render;
mod exec_cell;
mod exec_command;
mod external_agent_config_migration;
mod external_agent_config_migration_startup;
mod external_editor;
mod file_search;
mod frames;
mod get_git_diff;
mod git_action_directives;
mod goal_display;
mod history_cell;
mod hooks_rpc;
mod ide_context;
pub(crate) mod insert_history;
pub use insert_history::insert_history_lines;
mod key_hint;
mod keymap;
mod keymap_setup;
mod line_truncation;
pub(crate) mod live_wrap;
pub use live_wrap::RowBuilder;
mod local_chatgpt_auth;
mod markdown;
mod markdown_render;
mod markdown_stream;
mod mention_codec;
mod model_catalog;
mod model_migration;
mod motion;
mod multi_agents;
mod notifications;
#[cfg(any(not(debug_assertions), test))]
mod npm_registry;
pub(crate) mod onboarding;
mod oss_selection;
mod pager_overlay;
mod permission_compat;
pub(crate) mod public_widgets;
mod render;
mod resize_reflow_cap;
mod resume_picker;
mod selection_list;
mod session_log;
mod session_resume;
mod session_state;
mod shimmer;
mod skills_helpers;
mod slash_command;
mod startup_error;
mod startup_hooks_review;
mod status;
mod status_indicator_widget;
mod streaming;
mod style;
mod terminal_palette;
mod terminal_probe;
mod terminal_title;
mod text_formatting;
mod theme_picker;
mod token_usage;
mod tooltips;
mod transcript_reflow;
mod tui;
mod ui_consts;
pub(crate) mod update_action;
pub use update_action::UpdateAction;
#[cfg(not(debug_assertions))]
pub use update_action::get_update_action;
mod update_prompt;
#[cfg(any(not(debug_assertions), test))]
mod update_versions;
mod updates;
mod version;
#[cfg(not(target_os = "linux"))]
mod voice;
mod width;
mod workspace_command;
#[cfg(target_os = "linux")]
#[allow(dead_code)]
mod voice {
    use crate::app_event_sender::AppEventSender;
    use crate::legacy_core::config::Config;
    use codex_app_server_protocol::ThreadRealtimeAudioChunk;
    use std::sync::Arc;
    use std::sync::atomic::AtomicBool;
    use std::sync::atomic::AtomicU16;

    pub struct VoiceCapture;

    pub(crate) struct RecordingMeterState;

    pub(crate) struct RealtimeAudioPlayer;

    impl VoiceCapture {
        pub fn start_realtime(_config: &Config, _tx: AppEventSender) -> Result<Self, String> {
            Err("voice input is unavailable in this build".to_string())
        }

        pub fn stop(self) {}

        pub fn stopped_flag(&self) -> Arc<AtomicBool> {
            Arc::new(AtomicBool::new(true))
        }

        pub fn last_peak_arc(&self) -> Arc<AtomicU16> {
            Arc::new(AtomicU16::new(0))
        }
    }

    impl RecordingMeterState {
        pub(crate) fn new() -> Self {
            Self
        }

        pub(crate) fn next_text(&mut self, _peak: u16) -> String {
            "⠤⠤⠤⠤".to_string()
        }
    }

    impl RealtimeAudioPlayer {
        pub(crate) fn start(_config: &Config) -> Result<Self, String> {
            Err("voice output is unavailable in this build".to_string())
        }

        pub(crate) fn enqueue_frame(
            &self,
            _frame: &ThreadRealtimeAudioChunk,
        ) -> Result<(), String> {
            Err("voice output is unavailable in this build".to_string())
        }

        pub(crate) fn clear(&self) {}
    }
}

mod wrapping;

mod table_detect;
#[cfg(test)]
pub(crate) mod test_backend;
#[cfg(test)]
pub(crate) mod test_support;

use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::startup_hooks_review::StartupHooksReviewOutcome;
use crate::startup_hooks_review::maybe_run_startup_hooks_review;
use crate::tui::Tui;
pub use cli::Cli;
use codex_arg0::Arg0DispatchPaths;
pub use markdown_render::render_markdown_text;
pub use public_widgets::composer_input::ComposerAction;
pub use public_widgets::composer_input::ComposerInput;
// (tests access modules directly within the crate)

#[cfg(unix)]
const AUTO_CONNECT_DAEMON_CONNECT_TIMEOUT: std::time::Duration =
    std::time::Duration::from_millis(50);

#[allow(clippy::too_many_arguments)]
async fn start_embedded_app_server(
    arg0_paths: Arg0DispatchPaths,
    config: Config,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    cloud_requirements: CloudRequirementsLoader,
    feedback: codex_feedback::CodexFeedback,
    log_db: Option<log_db::LogDbLayer>,
    state_db: Option<StateDbHandle>,
    environment_manager: Arc<EnvironmentManager>,
) -> color_eyre::Result<InProcessAppServerClient> {
    start_embedded_app_server_with(
        arg0_paths,
        config,
        cli_kv_overrides,
        loader_overrides,
        strict_config,
        cloud_requirements,
        feedback,
        log_db,
        state_db,
        environment_manager,
        InProcessAppServerClient::start,
    )
    .await
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AppServerTarget {
    Embedded,
    Remote { endpoint: RemoteAppServerEndpoint },
}

async fn init_state_db_for_app_server_target(
    config: &Config,
    app_server_target: &AppServerTarget,
) -> std::io::Result<Option<StateDbHandle>> {
    match app_server_target {
        AppServerTarget::Embedded => state_db::try_init(config).await.map(Some).map_err(|err| {
            std::io::Error::other(LocalStateDbStartupError::new(
                codex_state::state_db_path(config.sqlite_home.as_path()),
                err.to_string(),
            ))
        }),
        AppServerTarget::Remote { .. } => Ok(state_db::get_state_db(config).await),
    }
}

fn remote_addr_has_explicit_port(addr: &str, parsed: &Url) -> bool {
    let Some(host) = parsed.host_str() else {
        return false;
    };
    if parsed.port().is_some() {
        return true;
    }

    let Some((_, rest)) = addr.split_once("://") else {
        return false;
    };
    let authority_end = rest.find(['/', '?', '#']).unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let host_and_port = authority
        .rsplit_once('@')
        .map_or(authority, |(_, host_and_port)| host_and_port);
    let explicit_default_port = match parsed.scheme() {
        "ws" => 80,
        "wss" => 443,
        _ => return false,
    };
    let expected_host = if host.contains(':') {
        format!("[{host}]")
    } else {
        host.to_string()
    };
    host_and_port == format!("{expected_host}:{explicit_default_port}")
}

fn websocket_url_supports_auth_token(parsed: &Url) -> bool {
    match (parsed.scheme(), parsed.host()) {
        ("wss", Some(_)) => true,
        ("ws", Some(url::Host::Domain(domain))) => domain.eq_ignore_ascii_case("localhost"),
        ("ws", Some(url::Host::Ipv4(addr))) => addr.is_loopback(),
        ("ws", Some(url::Host::Ipv6(addr))) => addr.is_loopback(),
        _ => false,
    }
}

pub fn resolve_remote_addr(addr: &str) -> color_eyre::Result<RemoteAppServerEndpoint> {
    if let Some(socket_path) = addr.strip_prefix("unix://") {
        let socket_path = if socket_path.is_empty() {
            let codex_home = find_codex_home().wrap_err("failed to resolve CODEX_HOME")?;
            codex_app_server_client::app_server_control_socket_path(&codex_home)
                .map_err(color_eyre::Report::new)?
        } else {
            AbsolutePathBuf::relative_to_current_dir(socket_path)
                .map_err(color_eyre::Report::new)?
        };
        return Ok(RemoteAppServerEndpoint::UnixSocket { socket_path });
    }

    let parsed = match Url::parse(addr) {
        Ok(parsed) => parsed,
        Err(_) => {
            color_eyre::eyre::bail!(
                "invalid remote address `{addr}`; expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
            );
        }
    };
    if matches!(parsed.scheme(), "ws" | "wss")
        && parsed.host_str().is_some()
        && remote_addr_has_explicit_port(addr, &parsed)
        && parsed.path() == "/"
        && parsed.query().is_none()
        && parsed.fragment().is_none()
    {
        return Ok(RemoteAppServerEndpoint::WebSocket {
            websocket_url: parsed.to_string(),
            auth_token: None,
        });
    }

    color_eyre::eyre::bail!(
        "invalid remote address `{addr}`; expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
    );
}

pub fn remote_addr_supports_auth_token(endpoint: &RemoteAppServerEndpoint) -> bool {
    match endpoint {
        RemoteAppServerEndpoint::WebSocket { websocket_url, .. } => {
            Url::parse(websocket_url).is_ok_and(|parsed| websocket_url_supports_auth_token(&parsed))
        }
        RemoteAppServerEndpoint::UnixSocket { .. } => false,
    }
}

async fn connect_remote_app_server(
    endpoint: RemoteAppServerEndpoint,
) -> color_eyre::Result<AppServerClient> {
    let app_server = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        endpoint,
        client_name: "codex-tui".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .wrap_err("failed to connect to remote app server")?;
    Ok(AppServerClient::Remote(app_server))
}

#[cfg(unix)]
async fn maybe_probe_default_daemon_socket(codex_home: &Path) -> Option<AbsolutePathBuf> {
    let socket_path = codex_app_server_client::app_server_control_socket_path(codex_home).ok()?;
    if !socket_path.as_path().try_exists().unwrap_or(false) {
        return None;
    }

    match tokio::time::timeout(
        AUTO_CONNECT_DAEMON_CONNECT_TIMEOUT,
        tokio::net::UnixStream::connect(socket_path.as_path()),
    )
    .await
    {
        Ok(Ok(_stream)) => Some(socket_path),
        Ok(Err(err)) => {
            tracing::debug!(%err, socket_path = %socket_path.display(), "skipping default app-server daemon socket");
            None
        }
        Err(_) => {
            tracing::debug!(
                socket_path = %socket_path.display(),
                timeout_ms = AUTO_CONNECT_DAEMON_CONNECT_TIMEOUT.as_millis(),
                "timed out probing default app-server daemon socket"
            );
            None
        }
    }
}

#[cfg(not(unix))]
async fn maybe_probe_default_daemon_socket(_codex_home: &Path) -> Option<AbsolutePathBuf> {
    None
}

#[allow(clippy::too_many_arguments)]
async fn start_app_server(
    target: &AppServerTarget,
    arg0_paths: Arg0DispatchPaths,
    config: Config,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    cloud_requirements: CloudRequirementsLoader,
    feedback: codex_feedback::CodexFeedback,
    log_db: Option<log_db::LogDbLayer>,
    state_db: Option<StateDbHandle>,
    environment_manager: Arc<EnvironmentManager>,
) -> color_eyre::Result<AppServerClient> {
    match target {
        AppServerTarget::Embedded => start_embedded_app_server(
            arg0_paths,
            config,
            cli_kv_overrides,
            loader_overrides,
            strict_config,
            cloud_requirements,
            feedback,
            log_db,
            state_db,
            environment_manager,
        )
        .await
        .map(AppServerClient::InProcess),
        AppServerTarget::Remote { endpoint } => connect_remote_app_server(endpoint.clone()).await,
    }
}

pub(crate) async fn start_app_server_for_picker(
    config: &Config,
    target: &AppServerTarget,
    state_db: Option<StateDbHandle>,
    environment_manager: Arc<EnvironmentManager>,
) -> color_eyre::Result<AppServerSession> {
    let app_server = start_app_server(
        target,
        Arg0DispatchPaths::default(),
        config.clone(),
        Vec::new(),
        LoaderOverrides::default(),
        /*strict_config*/ false,
        CloudRequirementsLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        state_db,
        environment_manager,
    )
    .await?;
    Ok(AppServerSession::new(app_server))
}

#[cfg(test)]
pub(crate) async fn start_embedded_app_server_for_picker(
    config: &Config,
) -> color_eyre::Result<AppServerSession> {
    let state_db = init_state_db_for_app_server_target(config, &AppServerTarget::Embedded).await?;
    start_app_server_for_picker(
        config,
        &AppServerTarget::Embedded,
        state_db,
        Arc::new(EnvironmentManager::default_for_tests()),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn start_embedded_app_server_with<F, Fut>(
    arg0_paths: Arg0DispatchPaths,
    config: Config,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    cloud_requirements: CloudRequirementsLoader,
    feedback: codex_feedback::CodexFeedback,
    log_db: Option<log_db::LogDbLayer>,
    state_db: Option<StateDbHandle>,
    environment_manager: Arc<EnvironmentManager>,
    start_client: F,
) -> color_eyre::Result<InProcessAppServerClient>
where
    F: FnOnce(InProcessClientStartArgs) -> Fut,
    Fut: Future<Output = std::io::Result<InProcessAppServerClient>>,
{
    let config_warnings = config
        .startup_warnings
        .iter()
        .map(|warning| ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        })
        .collect();
    let client = start_client(InProcessClientStartArgs {
        arg0_paths,
        config: Arc::new(config),
        cli_overrides: cli_kv_overrides,
        loader_overrides,
        strict_config,
        cloud_requirements,
        feedback,
        log_db,
        state_db,
        environment_manager,
        config_warnings,
        session_source: serde_json::from_value(serde_json::json!("cli"))
            .unwrap_or_else(|err| panic!("cli session source should deserialize: {err}")),
        enable_codex_api_key_env: false,
        client_name: "codex-tui".to_string(),
        client_version: env!("CARGO_PKG_VERSION").to_string(),
        experimental_api: true,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: DEFAULT_IN_PROCESS_CHANNEL_CAPACITY,
    })
    .await
    .wrap_err("failed to start embedded app server")?;
    Ok(client)
}

async fn shutdown_app_server_if_present(app_server: Option<AppServerSession>) {
    if let Some(app_server) = app_server
        && let Err(err) = app_server.shutdown().await
    {
        warn!(%err, "Failed to shut down temporary embedded app server");
    }
}

fn session_target_from_app_server_thread(
    thread: AppServerThread,
) -> Option<resume_picker::SessionTarget> {
    match ThreadId::from_string(&thread.id) {
        Ok(thread_id) => Some(resume_picker::SessionTarget {
            path: thread.path,
            thread_id,
        }),
        Err(err) => {
            warn!(
                thread_id = thread.id,
                %err,
                "Ignoring app-server thread with invalid thread id during TUI session lookup"
            );
            None
        }
    }
}

async fn lookup_session_target_by_name_with_app_server(
    app_server: &mut AppServerSession,
    name: &str,
) -> color_eyre::Result<Option<resume_picker::SessionTarget>> {
    let mut cursor = None;
    loop {
        let response = app_server
            .thread_list(ThreadListParams {
                cursor: cursor.clone(),
                limit: Some(100),
                sort_key: Some(AppServerThreadSortKey::UpdatedAt),
                sort_direction: None,
                model_providers: None,
                source_kinds: Some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode]),
                archived: Some(false),
                cwd: None,
                use_state_db_only: false,
                search_term: Some(name.to_string()),
            })
            .await?;
        if let Some(thread) = response
            .data
            .into_iter()
            .find(|thread| thread.name.as_deref() == Some(name))
        {
            return Ok(session_target_from_app_server_thread(thread));
        }
        if response.next_cursor.is_none() {
            return Ok(None);
        }
        cursor = response.next_cursor;
    }
}

async fn lookup_session_target_with_app_server(
    app_server: &mut AppServerSession,
    id_or_name: &str,
) -> color_eyre::Result<Option<resume_picker::SessionTarget>> {
    if Uuid::parse_str(id_or_name).is_ok() {
        let thread_id = match ThreadId::from_string(id_or_name) {
            Ok(thread_id) => thread_id,
            Err(err) => {
                warn!(
                    session = id_or_name,
                    %err,
                    "Failed to parse session id during TUI lookup"
                );
                return Ok(None);
            }
        };
        return match app_server
            .thread_read(thread_id, /*include_turns*/ false)
            .await
        {
            Ok(thread) => Ok(session_target_from_app_server_thread(thread)),
            Err(err) => {
                warn!(
                    session = id_or_name,
                    %err,
                    "thread/read failed during TUI session lookup"
                );
                Ok(None)
            }
        };
    }

    lookup_session_target_by_name_with_app_server(app_server, id_or_name).await
}

async fn lookup_latest_session_target_with_app_server(
    app_server: &mut AppServerSession,
    config: &Config,
    cwd_filter: Option<&Path>,
    include_non_interactive: bool,
) -> color_eyre::Result<Option<resume_picker::SessionTarget>> {
    let response = app_server
        .thread_list(latest_session_lookup_params(
            app_server.is_remote(),
            config,
            cwd_filter,
            include_non_interactive,
        ))
        .await?;
    Ok(response
        .data
        .into_iter()
        .find_map(session_target_from_app_server_thread))
}

fn latest_session_lookup_params(
    is_remote: bool,
    config: &Config,
    cwd_filter: Option<&Path>,
    include_non_interactive: bool,
) -> ThreadListParams {
    ThreadListParams {
        cursor: None,
        limit: Some(1),
        sort_key: Some(AppServerThreadSortKey::UpdatedAt),
        sort_direction: None,
        model_providers: if is_remote {
            None
        } else {
            Some(vec![config.model_provider_id.clone()])
        },
        source_kinds: (!include_non_interactive)
            .then_some(vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode]),
        archived: Some(false),
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().to_string())),
        use_state_db_only: false,
        search_term: None,
    }
}

fn config_cwd_for_app_server_target(
    cwd: Option<&Path>,
    app_server_target: &AppServerTarget,
    environment_manager: &EnvironmentManager,
) -> std::io::Result<Option<AbsolutePathBuf>> {
    if matches!(app_server_target, AppServerTarget::Remote { .. })
        || environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote())
    {
        return Ok(None);
    }

    let cwd = match cwd {
        Some(path) => {
            AbsolutePathBuf::from_absolute_path(canonicalize_existing_preserving_symlinks(path)?)
        }
        None => AbsolutePathBuf::current_dir(),
    }?;
    Ok(Some(cwd))
}

fn should_load_configured_environments(
    loader_overrides: &LoaderOverrides,
    app_server_target: &AppServerTarget,
) -> bool {
    !loader_overrides.ignore_user_config
        && !matches!(app_server_target, AppServerTarget::Remote { .. })
}

fn latest_session_cwd_filter<'a>(
    remote_mode: bool,
    remote_cwd_override: Option<&'a Path>,
    config: &'a Config,
    show_all: bool,
) -> Option<&'a Path> {
    if show_all {
        return None;
    }

    if remote_mode {
        remote_cwd_override
    } else {
        Some(config.cwd.as_path())
    }
}

pub async fn run_main(
    mut cli: Cli,
    arg0_paths: Arg0DispatchPaths,
    loader_overrides: LoaderOverrides,
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
) -> std::io::Result<AppExitInfo> {
    let strict_config = cli.strict_config;
    let (sandbox_mode, approval_policy) = if cli.dangerously_bypass_approvals_and_sandbox {
        (
            Some(SandboxMode::DangerFullAccess),
            Some(AskForApproval::Never.to_core()),
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
    let overrides_cli = codex_utils_cli::CliConfigOverrides { raw_overrides };
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

    let remote_endpoint = match explicit_remote_endpoint {
        Some(endpoint) => Some(endpoint),
        None => maybe_probe_default_daemon_socket(&codex_home)
            .await
            .map(|socket_path| RemoteAppServerEndpoint::UnixSocket { socket_path }),
    };
    let app_server_target = remote_endpoint
        .clone()
        .map_or(AppServerTarget::Embedded, |endpoint| {
            AppServerTarget::Remote { endpoint }
        });
    let remote_cwd_override = cli
        .cwd
        .clone()
        .filter(|_| matches!(app_server_target, AppServerTarget::Remote { .. }));

    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    let environment_manager =
        if should_load_configured_environments(&loader_overrides, &app_server_target) {
            EnvironmentManager::from_codex_home(codex_home.clone(), local_runtime_paths).await
        } else {
            EnvironmentManager::from_env(local_runtime_paths).await
        }
        .map(Arc::new)
        .map_err(std::io::Error::other)?;
    let cwd = cli.cwd.clone();
    let config_cwd =
        config_cwd_for_app_server_target(cwd.as_deref(), &app_server_target, &environment_manager)?;
    let mut loader_overrides = loader_overrides;
    if let Some(profile_v2) = cli.config_profile_v2.as_ref() {
        let user_config_path = resolve_profile_v2_config_path(&codex_home, profile_v2);
        loader_overrides.user_config_path = Some(user_config_path);
        loader_overrides.user_config_profile = Some(profile_v2.clone());
    }

    #[allow(clippy::print_stderr)]
    let config_toml = match load_config_as_toml_with_cli_and_load_options(
        &codex_home,
        config_cwd.as_ref(),
        cli_kv_overrides.clone(),
        codex_config::ConfigLoadOptions {
            loader_overrides: loader_overrides.clone(),
            strict_config,
        },
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

    let chatgpt_base_url = config_toml
        .chatgpt_base_url
        .clone()
        .unwrap_or_else(|| "https://chatgpt.com/backend-api/".to_string());
    let cloud_requirements = cloud_requirements_loader_for_storage(
        codex_home.to_path_buf(),
        /*enable_codex_api_key_env*/ false,
        config_toml.cli_auth_credentials_store.unwrap_or_default(),
        chatgpt_base_url,
    )
    .await;

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
        cwd: if matches!(app_server_target, AppServerTarget::Remote { .. }) {
            None
        } else {
            cwd
        },
        model_provider: model_provider_override.clone(),
        config_profile: cli.config_profile.clone(),
        codex_self_exe: arg0_paths.codex_self_exe.clone(),
        codex_linux_sandbox_exe: arg0_paths.codex_linux_sandbox_exe.clone(),
        main_execve_wrapper_exe: arg0_paths.main_execve_wrapper_exe.clone(),
        show_raw_agent_reasoning: cli.oss.then_some(true),
        bypass_hook_trust: cli.bypass_hook_trust.then_some(true),
        additional_writable_roots: additional_dirs,
        ..Default::default()
    };

    let mut config = load_config_or_exit(
        cli_kv_overrides.clone(),
        overrides.clone(),
        loader_overrides.clone(),
        cloud_requirements.clone(),
        strict_config,
    )
    .await;

    let otel_originator = originator().value;
    let otel = match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        crate::legacy_core::otel_init::build_provider(
            &config,
            env!("CARGO_PKG_VERSION"),
            /*service_name_override*/ None,
            /*default_analytics_enabled*/ true,
        )
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
    crate::legacy_core::otel_init::record_process_start(otel.as_ref(), otel_originator.as_str());
    crate::legacy_core::otel_init::install_sqlite_telemetry(
        otel.as_ref(),
        otel_originator.as_str(),
    );
    let state_db = init_state_db_for_app_server_target(&config, &app_server_target).await?;

    let effective_toml = config.config_layer_stack.effective_config();
    match effective_toml.try_into() {
        Ok(config_toml) => {
            match crate::legacy_core::personality_migration::maybe_migrate_personality(
                &config.codex_home,
                &config_toml,
                state_db.clone(),
            )
            .await
            {
                Ok(
                    crate::legacy_core::personality_migration::PersonalityMigrationStatus::Applied,
                ) => {
                    config = load_config_or_exit(
                        cli_kv_overrides.clone(),
                        overrides.clone(),
                        loader_overrides.clone(),
                        cloud_requirements.clone(),
                        strict_config,
                    )
                    .await;
                }
                Ok(
                    crate::legacy_core::personality_migration::PersonalityMigrationStatus::SkippedMarker
                    | crate::legacy_core::personality_migration::PersonalityMigrationStatus::SkippedExplicitPersonality
                    | crate::legacy_core::personality_migration::PersonalityMigrationStatus::SkippedNoSessions,
                ) => {}
                Err(err) => {
                    tracing::warn!(error = %err, "failed to run personality migration");
                }
            }
        }
        Err(err) => {
            tracing::warn!(error = %err, "failed to deserialize config for personality migration");
        }
    }

    #[allow(clippy::print_stderr)]
    match check_execpolicy_for_warnings(&config.config_layer_stack).await {
        Ok(None) => {}
        Ok(Some(err)) | Err(err) => {
            eprintln!(
                "Error loading rules:\n{}",
                format_exec_policy_error_with_source(&err)
            );
            std::process::exit(1);
        }
    }

    set_default_client_residency_requirement(config.enforce_residency.value());

    if let Some(warning) = add_dir_warning_message(
        &cli.add_dir,
        &config.permissions.effective_permission_profile(),
        config.cwd.as_path(),
    ) {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("Error adding directories: {warning}");
            std::process::exit(1);
        }
    }

    if matches!(app_server_target, AppServerTarget::Embedded) {
        #[allow(clippy::print_stderr)]
        if let Err(err) = enforce_login_restrictions(&AuthConfig {
            codex_home: config.codex_home.to_path_buf(),
            auth_credentials_store_mode: config.cli_auth_credentials_store_mode,
            forced_login_method: config.forced_login_method,
            forced_chatgpt_workspace_id: config.forced_chatgpt_workspace_id.clone(),
            chatgpt_base_url: Some(config.chatgpt_base_url.clone()),
        })
        .await
        {
            eprintln!("{err}");
            std::process::exit(1);
        }
    }

    let log_dir = config.log_dir.clone();
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

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let log_db = state_db.clone().map(log_db::start);
    let log_db_layer = log_db
        .clone()
        .map(|layer| layer.with_filter(Targets::new().with_default(Level::TRACE)));

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
        arg0_paths,
        loader_overrides,
        strict_config,
        app_server_target,
        remote_cwd_override,
        config,
        overrides,
        cli_kv_overrides,
        cloud_requirements,
        feedback,
        log_db,
        state_db,
        remote_endpoint,
        environment_manager,
    )
    .await
    .map_err(|err| std::io::Error::other(err.to_string()))
}

#[allow(clippy::too_many_arguments)]
async fn run_ratatui_app(
    cli: Cli,
    arg0_paths: Arg0DispatchPaths,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    app_server_target: AppServerTarget,
    remote_cwd_override: Option<PathBuf>,
    initial_config: Config,
    overrides: ConfigOverrides,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    mut cloud_requirements: CloudRequirementsLoader,
    feedback: codex_feedback::CodexFeedback,
    log_db: Option<log_db::LogDbLayer>,
    state_db: Option<StateDbHandle>,
    remote_endpoint: Option<RemoteAppServerEndpoint>,
    environment_manager: Arc<EnvironmentManager>,
) -> color_eyre::Result<AppExitInfo> {
    let remote_mode = matches!(&app_server_target, AppServerTarget::Remote { .. });
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
    let mut terminal_restore_guard = TerminalRestoreGuard::new();

    #[cfg(not(debug_assertions))]
    {
        use crate::update_prompt::UpdatePromptOutcome;

        let skip_update_prompt = cli.prompt.as_ref().is_some_and(|prompt| !prompt.is_empty());
        if !skip_update_prompt {
            match update_prompt::run_update_prompt_if_needed(&mut tui, &initial_config).await? {
                UpdatePromptOutcome::Continue => {}
                UpdatePromptOutcome::RunUpdate(action) => {
                    terminal_restore_guard.restore()?;
                    return Ok(AppExitInfo {
                        token_usage: crate::token_usage::TokenUsage::default(),
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

    let app_server_session = match start_app_server(
        &app_server_target,
        arg0_paths.clone(),
        initial_config.clone(),
        cli_kv_overrides.clone(),
        loader_overrides.clone(),
        strict_config,
        cloud_requirements.clone(),
        feedback.clone(),
        log_db.clone(),
        state_db.clone(),
        environment_manager.clone(),
    )
    .await
    {
        Ok(app_server) => AppServerSession::new(app_server),
        Err(err) => {
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            return Err(err);
        }
    }
    .with_remote_cwd_override(remote_cwd_override.clone());
    let mut app_server = Some(app_server_session);

    let should_show_trust_screen_flag = !remote_mode && should_show_trust_screen(&initial_config);
    let mut trust_decision_was_made = false;
    let login_status = if initial_config.model_provider.requires_openai_auth {
        let Some(app_server) = app_server.as_mut() else {
            unreachable!("app server should exist when auth is required");
        };
        get_login_status(app_server, &initial_config).await?
    } else {
        LoginStatus::NotAuthenticated
    };
    let should_show_onboarding =
        should_show_onboarding(login_status, &initial_config, should_show_trust_screen_flag);

    let config = if should_show_onboarding {
        let show_login_screen = should_show_login_screen(login_status, &initial_config);
        let onboarding_result = run_onboarding_app(
            OnboardingScreenArgs {
                show_login_screen,
                show_trust_screen: should_show_trust_screen_flag,
                login_status,
                app_server_request_handle: app_server
                    .as_ref()
                    .map(AppServerSession::request_handle),
                config: initial_config.clone(),
            },
            if show_login_screen {
                app_server.as_mut()
            } else {
                None
            },
            &mut tui,
        )
        .await?;
        if onboarding_result.should_exit {
            shutdown_app_server_if_present(app_server.take()).await;
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            let _ = tui.terminal.clear();
            return Ok(AppExitInfo {
                token_usage: crate::token_usage::TokenUsage::default(),
                thread_id: None,
                thread_name: None,
                update_action: None,
                exit_reason: ExitReason::UserRequested,
            });
        }
        trust_decision_was_made = onboarding_result.directory_trust_decision.is_some();
        // If this onboarding run included the login step, always refresh cloud requirements and
        // rebuild config. This avoids missing newly available cloud requirements due to login
        // status detection edge cases.
        if show_login_screen && !remote_mode {
            cloud_requirements = cloud_requirements_loader_for_storage(
                initial_config.codex_home.to_path_buf(),
                /*enable_codex_api_key_env*/ false,
                initial_config.cli_auth_credentials_store_mode,
                initial_config.chatgpt_base_url.clone(),
            )
            .await;
        }

        // If the user made an explicit trust decision, or we showed the login flow, reload config
        // so current process state reflects persisted trust/auth changes.
        if onboarding_result.directory_trust_decision.is_some()
            || (show_login_screen && !remote_mode)
        {
            load_config_or_exit(
                cli_kv_overrides.clone(),
                overrides.clone(),
                loader_overrides.clone(),
                cloud_requirements.clone(),
                strict_config,
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
        terminal_restore_guard.restore_silently();
        session_log::log_session_end();
        let _ = tui.terminal.clear();
        Ok(AppExitInfo {
            token_usage: crate::token_usage::TokenUsage::default(),
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
            let Some(startup_app_server) = app_server.as_mut() else {
                unreachable!("app server should be initialized for --fork <id>");
            };
            match lookup_session_target_with_app_server(startup_app_server, id_str).await? {
                Some(target_session) => resume_picker::SessionSelection::Fork(target_session),
                None => {
                    shutdown_app_server_if_present(app_server.take()).await;
                    return missing_session_exit(id_str, "fork");
                }
            }
        } else if cli.fork_last {
            let filter_cwd = latest_session_cwd_filter(
                remote_mode,
                remote_cwd_override.as_deref(),
                &config,
                cli.fork_show_all,
            );
            let Some(app_server) = app_server.as_mut() else {
                unreachable!("app server should be initialized for --fork --last");
            };
            match lookup_latest_session_target_with_app_server(
                app_server, &config, filter_cwd, /*include_non_interactive*/ false,
            )
            .await?
            {
                Some(target_session) => resume_picker::SessionSelection::Fork(target_session),
                None => resume_picker::SessionSelection::StartFresh,
            }
        } else if cli.fork_picker {
            let Some(app_server) = app_server.take() else {
                unreachable!("app server should be initialized for --fork picker");
            };
            match resume_picker::run_fork_picker_with_app_server(
                &mut tui,
                &config,
                cli.fork_show_all,
                app_server,
            )
            .await?
            {
                resume_picker::SessionSelection::Exit => {
                    terminal_restore_guard.restore_silently();
                    session_log::log_session_end();
                    return Ok(AppExitInfo {
                        token_usage: crate::token_usage::TokenUsage::default(),
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
        let Some(startup_app_server) = app_server.as_mut() else {
            unreachable!("app server should be initialized for --resume <id>");
        };
        match lookup_session_target_with_app_server(startup_app_server, id_str).await? {
            Some(target_session) => resume_picker::SessionSelection::Resume(target_session),
            None => {
                shutdown_app_server_if_present(app_server.take()).await;
                return missing_session_exit(id_str, "resume");
            }
        }
    } else if cli.resume_last {
        let filter_cwd = latest_session_cwd_filter(
            remote_mode,
            remote_cwd_override.as_deref(),
            &config,
            cli.resume_show_all,
        );
        let Some(app_server) = app_server.as_mut() else {
            unreachable!("app server should be initialized for --resume --last");
        };
        match lookup_latest_session_target_with_app_server(
            app_server,
            &config,
            filter_cwd,
            cli.resume_include_non_interactive,
        )
        .await?
        {
            Some(target_session) => resume_picker::SessionSelection::Resume(target_session),
            None => resume_picker::SessionSelection::StartFresh,
        }
    } else if cli.resume_picker {
        let Some(app_server) = app_server.take() else {
            unreachable!("app server should be initialized for --resume picker");
        };
        match resume_picker::run_resume_picker_with_app_server(
            &mut tui,
            &config,
            cli.resume_show_all,
            cli.resume_include_non_interactive,
            app_server,
        )
        .await?
        {
            resume_picker::SessionSelection::Exit => {
                terminal_restore_guard.restore_silently();
                session_log::log_session_end();
                return Ok(AppExitInfo {
                    token_usage: crate::token_usage::TokenUsage::default(),
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
    let allow_prompt = !remote_mode && cli.cwd.is_none();
    let action_and_target_session_if_resume_or_fork = match &session_selection {
        resume_picker::SessionSelection::Resume(target_session) => {
            Some((CwdPromptAction::Resume, target_session))
        }
        resume_picker::SessionSelection::Fork(target_session) => {
            Some((CwdPromptAction::Fork, target_session))
        }
        _ => None,
    };
    let fallback_cwd = match action_and_target_session_if_resume_or_fork {
        Some((action, target_session)) => {
            if remote_mode {
                Some(current_cwd.to_path_buf())
            } else {
                match resolve_cwd_for_resume_or_fork(
                    &mut tui,
                    state_db.as_deref(),
                    &current_cwd,
                    target_session.thread_id,
                    target_session.path.as_deref(),
                    action,
                    allow_prompt,
                )
                .await?
                {
                    ResolveCwdOutcome::Continue(cwd) => cwd,
                    ResolveCwdOutcome::Exit => {
                        terminal_restore_guard.restore_silently();
                        session_log::log_session_end();
                        return Ok(AppExitInfo {
                            token_usage: crate::token_usage::TokenUsage::default(),
                            thread_id: None,
                            thread_name: None,
                            update_action: None,
                            exit_reason: ExitReason::UserRequested,
                        });
                    }
                }
            }
        }
        None => None,
    };

    let picker_cancelled_without_selection = matches!(
        session_selection,
        resume_picker::SessionSelection::StartFresh
    ) && (cli.resume_picker || cli.fork_picker);

    let mut config = match &session_selection {
        resume_picker::SessionSelection::Resume(_) | resume_picker::SessionSelection::Fork(_) => {
            load_config_or_exit_with_fallback_cwd(
                cli_kv_overrides.clone(),
                overrides.clone(),
                loader_overrides.clone(),
                cloud_requirements.clone(),
                strict_config,
                fallback_cwd,
            )
            .await
        }
        resume_picker::SessionSelection::StartFresh if picker_cancelled_without_selection => {
            load_config_or_exit(
                cli_kv_overrides.clone(),
                overrides.clone(),
                loader_overrides.clone(),
                cloud_requirements.clone(),
                strict_config,
            )
            .await
        }
        _ => config,
    };

    // Configure syntax highlighting theme from the final config — onboarding
    // and resume/fork can both reload config with a different tui_theme, so
    // this must happen after the last possible reload.
    if let Some(w) = crate::render::highlight::set_theme_override(
        config.tui_theme.clone(),
        find_codex_home().ok().map(AbsolutePathBuf::into_path_buf),
    ) {
        config.startup_warnings.push(w);
    }

    set_default_client_residency_requirement(config.enforce_residency.value());
    let active_profile = config.active_profile.clone();
    let should_show_trust_screen = should_show_trust_screen(&config);
    let should_prompt_windows_sandbox_nux_at_startup = cfg!(target_os = "windows")
        && trust_decision_was_made
        && WindowsSandboxLevel::from_config(&config) == WindowsSandboxLevel::Disabled;

    let Cli {
        prompt,
        shared,
        no_alt_screen,
        ..
    } = cli;
    let images = shared.into_inner().images;

    let use_alt_screen = determine_alt_screen_mode(no_alt_screen, config.tui_alternate_screen);
    tui.set_alt_screen_enabled(use_alt_screen);
    let mut app_server = match app_server {
        Some(app_server) => app_server,
        None => match start_app_server(
            &app_server_target,
            arg0_paths,
            config.clone(),
            cli_kv_overrides.clone(),
            loader_overrides.clone(),
            strict_config,
            cloud_requirements.clone(),
            feedback.clone(),
            log_db.clone(),
            state_db.clone(),
            environment_manager.clone(),
        )
        .await
        {
            Ok(app_server) => AppServerSession::new(app_server)
                .with_remote_cwd_override(remote_cwd_override.clone()),
            Err(err) => {
                terminal_restore_guard.restore_silently();
                session_log::log_session_end();
                return Err(err);
            }
        },
    };

    let startup_hooks_browser =
        match maybe_run_startup_hooks_review(&mut app_server, &mut tui, &config).await? {
            StartupHooksReviewOutcome::Continue => None,
            StartupHooksReviewOutcome::OpenHooksBrowser(data) => Some(data),
        };

    let app_result = App::run(
        &mut tui,
        app_server,
        config,
        cli_kv_overrides.clone(),
        overrides.clone(),
        loader_overrides.clone(),
        active_profile,
        prompt,
        images,
        session_selection,
        feedback,
        should_show_trust_screen, // Proxy to: is it a first run in this directory?
        should_show_trust_screen_flag, // Preserve the startup-time trust NUX signal before onboarding
        should_prompt_windows_sandbox_nux_at_startup,
        remote_endpoint,
        state_db,
        environment_manager,
        startup_hooks_browser,
    )
    .await;

    terminal_restore_guard.restore_silently();
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
    if let Err(err) = tui::restore_after_exit() {
        eprintln!(
            "failed to restore terminal. Run `reset` or restart your terminal to recover: {err}"
        );
    }
}

struct TerminalRestoreGuard {
    active: bool,
}

impl TerminalRestoreGuard {
    fn new() -> Self {
        Self { active: true }
    }

    #[cfg_attr(debug_assertions, allow(dead_code))]
    fn restore(&mut self) -> color_eyre::Result<()> {
        if self.active {
            crate::tui::restore_after_exit()?;
            self.active = false;
        }
        Ok(())
    }

    fn restore_silently(&mut self) {
        if self.active {
            restore();
            self.active = false;
        }
    }
}

impl Drop for TerminalRestoreGuard {
    fn drop(&mut self) {
        self.restore_silently();
    }
}

/// Determine whether to use the terminal's alternate screen buffer.
///
/// - If `--no-alt-screen` is explicitly passed, always disable alternate screen
/// - Otherwise, respect the `tui.alternate_screen` config setting:
///   - `always`: Use alternate screen
///   - `never`: Inline mode only, preserves scrollback
///   - `auto` (default): Use alternate screen
fn determine_alt_screen_mode(no_alt_screen: bool, tui_alternate_screen: AltScreenMode) -> bool {
    if no_alt_screen {
        return false;
    }

    tui_alternate_screen != AltScreenMode::Never
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoginStatus {
    AuthMode(AppServerAuthMode),
    NotAuthenticated,
}

/// Determines the user's authentication mode using a lightweight account read
/// rather than a full `bootstrap`, avoiding the model-list fetch and
/// rate-limit round-trip that `bootstrap` would trigger.
async fn get_login_status(
    app_server: &mut AppServerSession,
    config: &Config,
) -> color_eyre::Result<LoginStatus> {
    if !config.model_provider.requires_openai_auth {
        return Ok(LoginStatus::NotAuthenticated);
    }

    let account = app_server.read_account().await?;
    Ok(match account.account {
        Some(AppServerAccount::ApiKey {}) => LoginStatus::AuthMode(AppServerAuthMode::ApiKey),
        Some(AppServerAccount::Chatgpt { .. }) => LoginStatus::AuthMode(AppServerAuthMode::Chatgpt),
        Some(AppServerAccount::AmazonBedrock {}) => LoginStatus::NotAuthenticated,
        None => LoginStatus::NotAuthenticated,
    })
}

async fn load_config_or_exit(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
    loader_overrides: LoaderOverrides,
    cloud_requirements: CloudRequirementsLoader,
    strict_config: bool,
) -> Config {
    load_config_or_exit_with_fallback_cwd(
        cli_kv_overrides,
        overrides,
        loader_overrides,
        cloud_requirements,
        strict_config,
        /*fallback_cwd*/ None,
    )
    .await
}

async fn load_config_or_exit_with_fallback_cwd(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
    loader_overrides: LoaderOverrides,
    cloud_requirements: CloudRequirementsLoader,
    strict_config: bool,
    fallback_cwd: Option<PathBuf>,
) -> Config {
    #[allow(clippy::print_stderr)]
    match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .loader_overrides(loader_overrides)
        .strict_config(strict_config)
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

/// Determine if the user has decided whether to trust the current directory.
fn should_show_trust_screen(config: &Config) -> bool {
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
    use crate::legacy_core::config::ConfigBuilder;
    use crate::legacy_core::config::ConfigOverrides;
    use codex_app_server_protocol::AskForApproval;
    use codex_app_server_protocol::ClientRequest;
    use codex_app_server_protocol::RequestId;
    use codex_app_server_protocol::ThreadStartParams;
    use codex_app_server_protocol::ThreadStartResponse;
    use codex_config::config_toml::ProjectConfig;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use tempfile::TempDir;

    async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
        ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await
    }

    async fn start_test_embedded_app_server(
        config: Config,
    ) -> color_eyre::Result<InProcessAppServerClient> {
        let state_db =
            init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await?;
        start_embedded_app_server(
            Arg0DispatchPaths::default(),
            config,
            Vec::new(),
            LoaderOverrides::default(),
            /*strict_config*/ false,
            CloudRequirementsLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            state_db,
            Arc::new(EnvironmentManager::default_for_tests()),
        )
        .await
    }

    #[test]
    fn alternate_screen_auto_uses_alt_screen() {
        assert!(determine_alt_screen_mode(
            /*no_alt_screen*/ false,
            AltScreenMode::Auto,
        ));
        assert!(determine_alt_screen_mode(
            /*no_alt_screen*/ false,
            AltScreenMode::Always,
        ));
        assert!(!determine_alt_screen_mode(
            /*no_alt_screen*/ false,
            AltScreenMode::Never,
        ));
        assert!(!determine_alt_screen_mode(
            /*no_alt_screen*/ true,
            AltScreenMode::Auto,
        ));
    }

    #[test]
    fn session_target_display_label_falls_back_to_thread_id() {
        let thread_id = ThreadId::new();
        let target = crate::resume_picker::SessionTarget {
            path: None,
            thread_id,
        };

        assert_eq!(target.display_label(), format!("thread {thread_id}"));
    }

    #[test]
    fn resolve_remote_addr_accepts_websocket_url() {
        assert_eq!(
            resolve_remote_addr("ws://127.0.0.1:4500").expect("ws URL should normalize"),
            RemoteAppServerEndpoint::WebSocket {
                websocket_url: "ws://127.0.0.1:4500/".to_string(),
                auth_token: None,
            }
        );
    }

    #[test]
    fn resolve_remote_addr_accepts_secure_websocket_url() {
        assert_eq!(
            resolve_remote_addr("wss://example.com:443").expect("wss URL should normalize"),
            RemoteAppServerEndpoint::WebSocket {
                websocket_url: "wss://example.com/".to_string(),
                auth_token: None,
            }
        );
    }

    #[test]
    fn resolve_remote_addr_accepts_default_socket() -> color_eyre::Result<()> {
        let codex_home = find_codex_home().wrap_err("failed to resolve CODEX_HOME")?;
        assert_eq!(
            resolve_remote_addr("unix://")?,
            RemoteAppServerEndpoint::UnixSocket {
                socket_path: codex_app_server_client::app_server_control_socket_path(&codex_home)?,
            }
        );
        Ok(())
    }

    #[test]
    fn resolve_remote_addr_accepts_relative_socket_path() -> color_eyre::Result<()> {
        assert_eq!(
            resolve_remote_addr("unix://codex.sock")?,
            RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
            }
        );
        Ok(())
    }

    #[test]
    fn resolve_remote_addr_accepts_absolute_socket_path() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let socket_path = temp_dir.path().join("codex.sock");
        assert_eq!(
            resolve_remote_addr(&format!("unix://{}", socket_path.display()))?,
            RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::from_absolute_path(&socket_path)?,
            }
        );
        Ok(())
    }

    #[test]
    fn resolve_remote_addr_rejects_invalid_remote_addresses() {
        for addr in [
            "ws://127.0.0.1",
            "wss://example.com",
            "127.0.0.1:4500",
            "https://127.0.0.1:4500",
        ] {
            let err = resolve_remote_addr(addr).expect_err("invalid remote addresses should fail");
            assert!(err.to_string().contains(
                "expected `ws://host:port`, `wss://host:port`, `unix://`, or `unix://PATH`"
            ));
        }
    }

    #[tokio::test]
    async fn default_daemon_auto_connect_skips_missing_socket() -> color_eyre::Result<()> {
        let codex_home = TempDir::new()?;
        assert!(
            maybe_probe_default_daemon_socket(codex_home.path())
                .await
                .is_none()
        );
        Ok(())
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn default_daemon_auto_connect_probes_socket_only() -> color_eyre::Result<()> {
        let codex_home = TempDir::new()?;
        let socket_path =
            codex_app_server_client::app_server_control_socket_path(codex_home.path())?;
        std::fs::create_dir_all(socket_path.as_path().parent().expect("socket parent"))?;
        let _listener = tokio::net::UnixListener::bind(socket_path.as_path())?;

        assert_eq!(
            maybe_probe_default_daemon_socket(codex_home.path()).await,
            Some(socket_path)
        );
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_keep_local_filters_for_embedded_sessions()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let cwd = temp_dir.path().join("project");

        let params = latest_session_lookup_params(
            /*is_remote*/ false,
            &config,
            Some(cwd.as_path()),
            /*include_non_interactive*/ false,
        );

        assert_eq!(params.model_providers, Some(vec![config.model_provider_id]));
        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(cwd.to_string_lossy().to_string()))
        );
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_omit_local_filters_for_remote_sessions()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;

        let params = latest_session_lookup_params(
            /*is_remote*/ true, &config, /*cwd_filter*/ None,
            /*include_non_interactive*/ false,
        );

        assert_eq!(params.model_providers, None);
        assert_eq!(params.cwd, None);
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_keep_explicit_cwd_filter_for_remote_sessions()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let cwd = Path::new("repo/on/server");

        let params = latest_session_lookup_params(
            /*is_remote*/ true,
            &config,
            Some(cwd),
            /*include_non_interactive*/ false,
        );

        assert_eq!(params.model_providers, None);
        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(String::from("repo/on/server")))
        );
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_cwd_filter_respects_scope_options() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let remote_cwd = Path::new("repo/on/server");

        let local_filter = latest_session_cwd_filter(
            /*remote_mode*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ false,
        );
        let show_all_filter = latest_session_cwd_filter(
            /*remote_mode*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ true,
        );
        let remote_filter = latest_session_cwd_filter(
            /*remote_mode*/ true,
            Some(remote_cwd),
            &config,
            /*show_all*/ false,
        );

        assert_eq!(local_filter, Some(config.cwd.as_path()));
        assert_eq!(show_all_filter, None);
        assert_eq!(remote_filter, Some(remote_cwd));
        Ok(())
    }

    #[tokio::test]
    async fn fork_last_filters_latest_session_by_cwd_unless_show_all() -> color_eyre::Result<()> {
        fn write_session_rollout(
            codex_home: &Path,
            filename_ts: &str,
            meta_rfc3339: &str,
            preview: &str,
            model_provider: &str,
            cwd: &Path,
        ) -> color_eyre::Result<ThreadId> {
            let uuid = Uuid::new_v4();
            let uuid_str = uuid.to_string();
            let thread_id = ThreadId::from_string(&uuid_str)?;
            let year = &filename_ts[0..4];
            let month = &filename_ts[5..7];
            let day = &filename_ts[8..10];
            let rollout_path = codex_home
                .join("sessions")
                .join(year)
                .join(month)
                .join(day)
                .join(format!("rollout-{filename_ts}-{uuid_str}.jsonl"));
            let parent = rollout_path.parent().ok_or_else(|| {
                color_eyre::eyre::eyre!("rollout path is missing a parent directory")
            })?;
            std::fs::create_dir_all(parent)?;

            let session_meta = codex_protocol::protocol::SessionMeta {
                id: thread_id,
                timestamp: meta_rfc3339.to_string(),
                cwd: cwd.to_path_buf(),
                originator: "codex".to_string(),
                cli_version: "0.0.0".to_string(),
                source: codex_protocol::protocol::SessionSource::Cli,
                model_provider: Some(model_provider.to_string()),
                ..Default::default()
            };
            let session_meta = serde_json::to_value(codex_protocol::protocol::SessionMetaLine {
                meta: session_meta,
                git: None,
            })?;
            let lines = [
                serde_json::json!({
                    "timestamp": meta_rfc3339,
                    "type": "session_meta",
                    "payload": session_meta,
                })
                .to_string(),
                serde_json::json!({
                    "timestamp": meta_rfc3339,
                    "type": "response_item",
                    "payload": {
                        "type": "message",
                        "role": "user",
                        "content": [{"type": "input_text", "text": preview}],
                    },
                })
                .to_string(),
                serde_json::json!({
                    "timestamp": meta_rfc3339,
                    "type": "event_msg",
                    "payload": {
                        "type": "user_message",
                        "message": preview,
                        "kind": "plain",
                    },
                })
                .to_string(),
            ];
            std::fs::write(&rollout_path, lines.join("\n") + "\n")?;
            let updated_at =
                chrono::DateTime::parse_from_rfc3339(meta_rfc3339)?.with_timezone(&chrono::Utc);
            let times = std::fs::FileTimes::new().set_modified(updated_at.into());
            OpenOptions::new()
                .append(true)
                .open(rollout_path)?
                .set_times(times)?;

            Ok(thread_id)
        }

        let temp_dir = TempDir::new()?;
        let project_cwd = temp_dir.path().join("project");
        let other_cwd = temp_dir.path().join("other-project");
        std::fs::create_dir_all(&project_cwd)?;
        std::fs::create_dir_all(&other_cwd)?;

        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                cwd: Some(project_cwd.clone()),
                ..Default::default()
            })
            .build()
            .await?;
        let model_provider = config.model_provider_id.as_str();
        let project_thread_id = write_session_rollout(
            temp_dir.path(),
            "2025-01-02T10-00-00",
            "2025-01-02T10:00:00Z",
            "older project session",
            model_provider,
            &project_cwd,
        )?;
        let other_thread_id = write_session_rollout(
            temp_dir.path(),
            "2025-01-02T12-00-00",
            "2025-01-02T12:00:00Z",
            "newer other project session",
            model_provider,
            &other_cwd,
        )?;

        let mut app_server =
            AppServerSession::new(codex_app_server_client::AppServerClient::InProcess(
                start_test_embedded_app_server(config.clone()).await?,
            ));
        let filter_cwd = latest_session_cwd_filter(
            /*remote_mode*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ false,
        );
        let scoped_target = lookup_latest_session_target_with_app_server(
            &mut app_server,
            &config,
            filter_cwd,
            /*include_non_interactive*/ false,
        )
        .await?
        .expect("expected project-scoped fork --last target");
        let show_all_filter_cwd = latest_session_cwd_filter(
            /*remote_mode*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ true,
        );
        let show_all_target = lookup_latest_session_target_with_app_server(
            &mut app_server,
            &config,
            show_all_filter_cwd,
            /*include_non_interactive*/ false,
        )
        .await?
        .expect("expected global fork --last target");
        app_server.shutdown().await?;

        assert_eq!(scoped_target.thread_id, project_thread_id);
        assert_eq!(show_all_target.thread_id, other_thread_id);
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_omits_cwd_for_remote_sessions() -> std::io::Result<()>
    {
        let remote_only_cwd = if cfg!(windows) {
            Path::new(r"C:\definitely\not\local\to\this\test")
        } else {
            Path::new("/definitely/not/local/to/this/test")
        };
        let target = AppServerTarget::Remote {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
            },
        };
        let environment_manager = EnvironmentManager::default_for_tests();

        let config_cwd =
            config_cwd_for_app_server_target(Some(remote_only_cwd), &target, &environment_manager)?;

        assert_eq!(config_cwd, None);
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_canonicalizes_embedded_cli_cwd() -> std::io::Result<()>
    {
        let temp_dir = TempDir::new()?;
        let target = AppServerTarget::Embedded;
        let environment_manager = EnvironmentManager::default_for_tests();

        let config_cwd =
            config_cwd_for_app_server_target(Some(temp_dir.path()), &target, &environment_manager)?;

        assert_eq!(
            config_cwd,
            Some(AbsolutePathBuf::from_absolute_path(dunce::canonicalize(
                temp_dir.path()
            )?)?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_errors_for_missing_embedded_cli_cwd()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let missing = temp_dir.path().join("missing");
        let target = AppServerTarget::Embedded;
        let environment_manager = EnvironmentManager::default_for_tests();

        let err = config_cwd_for_app_server_target(Some(&missing), &target, &environment_manager)
            .expect_err("missing embedded cwd should fail");

        assert_eq!(err.kind(), std::io::ErrorKind::NotFound);
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_omits_cwd_for_remote_exec_server()
    -> std::io::Result<()> {
        let remote_only_cwd = if cfg!(windows) {
            Path::new(r"C:\definitely\not\local\to\this\test")
        } else {
            Path::new("/definitely/not/local/to/this/test")
        };
        let target = AppServerTarget::Embedded;
        let environment_manager = EnvironmentManager::create_for_tests(
            Some("ws://127.0.0.1:8765".to_string()),
            ExecServerRuntimePaths::new(
                std::env::current_exe().expect("current exe"),
                /*codex_linux_sandbox_exe*/ None,
            )?,
        )
        .await;

        let config_cwd =
            config_cwd_for_app_server_target(Some(remote_only_cwd), &target, &environment_manager)?;

        assert_eq!(config_cwd, None);
        Ok(())
    }

    #[tokio::test]
    #[serial]
    async fn windows_shows_trust_prompt_without_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_enabled(/*value*/ false);

        let should_show = should_show_trust_screen(&config);
        assert!(
            should_show,
            "Trust prompt should be shown when project trust is undecided"
        );
        Ok(())
    }

    #[tokio::test]
    async fn embedded_app_server_supports_thread_start_rpc() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let app_server = start_test_embedded_app_server(config).await?;
        let response: ThreadStartResponse = app_server
            .request_typed(ClientRequest::ThreadStart {
                request_id: RequestId::Integer(1),
                params: ThreadStartParams {
                    ephemeral: Some(true),
                    ..ThreadStartParams::default()
                },
            })
            .await
            .expect("thread/start should succeed");
        assert!(!response.thread.id.is_empty());

        app_server.shutdown().await?;
        Ok(())
    }

    #[tokio::test]
    async fn lookup_session_target_by_name_uses_backend_title_search() -> color_eyre::Result<()> {
        Box::pin(async {
            let temp_dir = TempDir::new()?;
            let config = build_config(&temp_dir).await?;
            let thread_id = ThreadId::new();
            let rollout_path = temp_dir
                .path()
                .join("sessions/2025/02/01")
                .join(format!("rollout-2025-02-01T10-00-00-{thread_id}.jsonl"));
            let rollout_dir = rollout_path.parent().expect("rollout parent");
            std::fs::create_dir_all(rollout_dir)?;
            std::fs::write(&rollout_path, "")?;

            let state_runtime = codex_state::StateRuntime::init(
                config.codex_home.to_path_buf(),
                config.model_provider_id.clone(),
            )
            .await
            .map_err(std::io::Error::other)?;
            state_runtime
                .mark_backfill_complete(/*last_watermark*/ None)
                .await
                .map_err(std::io::Error::other)?;

            let session_cwd = temp_dir.path().join("project");
            std::fs::create_dir_all(&session_cwd)?;
            let created_at = chrono::DateTime::parse_from_rfc3339("2025-02-01T10:00:00Z")
                .expect("timestamp should parse")
                .with_timezone(&chrono::Utc);
            let mut builder = codex_state::ThreadMetadataBuilder::new(
                thread_id,
                rollout_path.clone(),
                created_at,
                serde_json::from_value(serde_json::json!("cli"))
                    .expect("cli session source should deserialize"),
            );
            builder.cwd = session_cwd;
            let mut metadata = builder.build(config.model_provider_id.as_str());
            metadata.title = "saved-session".to_string();
            metadata.first_user_message = Some("preview text".to_string());
            state_runtime
                .upsert_thread(&metadata)
                .await
                .map_err(std::io::Error::other)?;

            let mut app_server =
                AppServerSession::new(codex_app_server_client::AppServerClient::InProcess(
                    start_test_embedded_app_server(config).await?,
                ));
            let target =
                lookup_session_target_by_name_with_app_server(&mut app_server, "saved-session")
                    .await?;
            let target = target.expect("name lookup should find the saved thread");
            assert_eq!(target.path, Some(rollout_path));
            assert_eq!(target.thread_id, thread_id);

            app_server.shutdown().await?;
            Ok(())
        })
        .await
    }

    #[tokio::test]
    async fn embedded_app_server_start_failure_is_returned() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let result = start_embedded_app_server_with(
            Arg0DispatchPaths::default(),
            config,
            Vec::new(),
            LoaderOverrides::default(),
            /*strict_config*/ false,
            CloudRequirementsLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            /*state_db*/ None,
            Arc::new(EnvironmentManager::default_for_tests()),
            |_args| async { Err(std::io::Error::other("boom")) },
        )
        .await;
        let err = match result {
            Ok(_) => panic!("startup failure should be returned"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("failed to start embedded app server"),
            "error should preserve the embedded app server startup context"
        );
        Ok(())
    }

    #[tokio::test]
    async fn embedded_state_db_failure_is_typed_for_cli_recovery() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        let occupied_sqlite_home = temp_dir.path().join("sqlite-home");
        std::fs::write(&occupied_sqlite_home, "occupied")?;
        config.sqlite_home = occupied_sqlite_home.clone();

        let err =
            match init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await {
                Ok(_) => panic!("embedded startup should surface state db init failures"),
                Err(err) => err,
            };
        let startup_error = err
            .get_ref()
            .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
            .expect("state db startup failure should retain its typed context");

        assert_eq!(
            startup_error.state_db_path(),
            codex_state::state_db_path(occupied_sqlite_home.as_path()).as_path()
        );
        assert!(
            startup_error
                .detail()
                .contains("failed to initialize state runtime"),
            "startup error should preserve the underlying state db failure"
        );
        Ok(())
    }
    #[tokio::test]
    #[serial]
    async fn windows_shows_trust_prompt_with_sandbox() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        config.active_project = ProjectConfig { trust_level: None };
        config.set_windows_sandbox_enabled(/*value*/ true);

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
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .codex_home(codex_home.clone())
            .harness_overrides(trusted_overrides.clone())
            .build()
            .await?;
        assert_eq!(
            AskForApproval::from(trusted_config.permissions.approval_policy.value()),
            AskForApproval::OnRequest
        );

        let untrusted_overrides = ConfigOverrides {
            cwd: Some(untrusted),
            ..trusted_overrides
        };
        let untrusted_config = ConfigBuilder::default()
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .codex_home(codex_home)
            .harness_overrides(untrusted_overrides)
            .build()
            .await?;
        assert_eq!(
            AskForApproval::from(untrusted_config.permissions.approval_policy.value()),
            AskForApproval::UnlessTrusted
        );
        Ok(())
    }

    /// Regression: theme must be configured from the *final* config.
    ///
    /// `run_ratatui_app` can reload config during onboarding and again
    /// during session resume/fork.  The syntax theme override (stored in
    /// a `OnceLock`) must use the final config's `tui_theme`, not the
    /// initial one — otherwise users resuming a thread in a project with
    /// a different theme get the wrong highlighting.
    ///
    /// We verify the invariant indirectly: `validate_theme_name` (the
    /// pure validation core of `set_theme_override`) must be called with
    /// the *final* config's theme, and its warning must land in the
    /// final config's `startup_warnings`.
    #[tokio::test]
    async fn theme_warning_uses_final_config() -> std::io::Result<()> {
        use crate::render::highlight::validate_theme_name;

        let temp_dir = TempDir::new()?;

        // initial_config has a valid theme — no warning.
        let initial_config = build_config(&temp_dir).await?;
        assert!(initial_config.tui_theme.is_none());

        // Simulate resume/fork reload: the final config has an invalid theme.
        let mut config = build_config(&temp_dir).await?;
        config.tui_theme = Some("bogus-theme".into());

        // Theme override must use the final config (not initial_config).
        // This mirrors the real call site in run_ratatui_app.
        if let Some(w) = validate_theme_name(config.tui_theme.as_deref(), Some(temp_dir.path())) {
            config.startup_warnings.push(w);
        }

        assert_eq!(
            config.startup_warnings.len(),
            1,
            "warning from final config's invalid theme should be present"
        );
        assert!(
            config.startup_warnings[0].contains("bogus-theme"),
            "warning should reference the final config's theme name"
        );
        Ok(())
    }
}
