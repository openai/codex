// Forbid accidental stdout/stderr writes in the *library* portion of the TUI.
// The standalone `codex-tui` binary prints a short help message before the
// alternate‑screen mode starts; that file opts‑out locally via `allow`.
#![deny(clippy::print_stdout, clippy::print_stderr)]
#![deny(clippy::disallowed_methods)]
use crate::legacy_core::config::Config;
use crate::legacy_core::config::ConfigBuilder;
use crate::legacy_core::config::ConfigOverrides;
use crate::legacy_core::config::ConfigTomlLoadResult;
use crate::legacy_core::config::bootstrap_auth_config;
use crate::legacy_core::config::load_config_toml_with_layer_stack;
#[cfg(test)]
use crate::legacy_core::config::resolve_bootstrap_http_client_factory;
use crate::legacy_core::config::resolve_oss_provider;
use crate::legacy_core::config::resolve_profile_v2_config_path;
use crate::session_resume::ResolveCwdOutcome;
use crate::session_resume::ResumeCwdContext;
use crate::session_resume::effective_resume_cwd_mode;
use crate::session_resume::resolve_cwd_for_resume_or_fork;
pub use crate::startup_error::LocalStateDbStartupError;
use additional_dirs::add_dir_warning_message;
use app::App;
pub use app::AppExitInfo;
pub use app::ExitReason;
use app_server_session::AppServerSession;
use app_server_session::ThreadParamsMode;
use codex_app_server_client::AppServerClient;
use codex_app_server_client::DEFAULT_IN_PROCESS_CHANNEL_CAPACITY;
use codex_app_server_client::InProcessAppServerClient;
use codex_app_server_client::InProcessClientStartArgs;
use codex_app_server_client::RemoteAppServerClient;
use codex_app_server_client::RemoteAppServerConnectArgs;
pub use codex_app_server_client::RemoteAppServerEndpoint;
use codex_app_server_protocol::Account as AppServerAccount;
use codex_app_server_protocol::AskForApproval;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::GetAccountResponse;
use codex_app_server_protocol::Thread as AppServerThread;
use codex_app_server_protocol::ThreadListCwdFilter;
use codex_app_server_protocol::ThreadListParams;
use codex_app_server_protocol::ThreadSortKey as AppServerThreadSortKey;
use codex_app_server_protocol::ThreadSourceKind;
use codex_cloud_config::cloud_config_bundle_loader_for_storage;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigLoadError;
use codex_config::LoaderOverrides;
use codex_config::format_config_error_with_source;
use codex_config::types::ResumeCwdMode;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_login::AuthConfig;
use codex_login::default_client::originator;
use codex_login::default_client::set_default_client_residency_requirement;
use codex_login::enforce_login_restrictions;
use codex_login::is_workload_identity_selected;
use codex_protocol::ThreadId;
use codex_protocol::auth::AuthMode;
use codex_protocol::config_types::AltScreenMode;
use codex_protocol::config_types::SandboxMode;
#[cfg(target_os = "windows")]
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
pub use session_archive_commands::DeleteConfirmation;
pub use session_archive_commands::SessionArchiveAction;
pub use session_archive_commands::SessionArchiveCommandOptions;
pub use session_archive_commands::run_session_archive_command;
pub use session_queue_commands::run_session_queue_command;
use std::fs::OpenOptions;
use std::io::IsTerminal;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
pub use token_usage::TokenUsage;
use tracing::error;
use tracing::warn;
use tracing_appender::non_blocking;
use tracing_subscriber::EnvFilter;
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
mod app_info;
mod app_server_approval_conversions;
mod app_server_session;
mod approval_events;
mod ascii_animation;
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
mod external_editor;
mod file_search;
mod frames;
mod get_git_diff;
mod git_action_directives;
mod goal_display;
mod goal_files;
mod history_cell;
mod hooks_rpc;
mod ide_context;
mod inline_visualization;
pub(crate) mod insert_history;
pub use insert_history::insert_history_lines;
mod key_hint;
mod keymap;
mod keymap_setup;
mod line_truncation;
pub(crate) mod live_wrap;
pub use live_wrap::RowBuilder;
mod local_chatgpt_auth;
mod managed_new_thread_defaults;
mod markdown;
mod markdown_render;
mod markdown_stream;
mod markdown_text_merge;
mod mention_codec;
mod model_catalog;
mod model_migration;
mod motion;
mod multi_agents;
mod named_session_lookup;
mod notifications;
#[cfg(any(not(debug_assertions), test))]
mod npm_registry;
pub(crate) mod onboarding;
mod oss_selection;
mod pager_overlay;
pub(crate) mod public_widgets;
mod render;
mod resize_reflow_cap;
mod resume_picker;
mod selection_list;
mod service_tier_resolution;
mod session_archive_commands;
mod session_log;
mod session_queue_commands;
mod session_resume;
mod session_start;
mod session_state;
mod skills_helpers;
mod slash_command;
mod startup_draft;
mod startup_error;
mod startup_hooks_review;
mod startup_orchestration;
mod startup_preflight;
mod status;
mod status_indicator_widget;
mod streaming;
mod style;
mod terminal_hyperlinks;
mod terminal_palette;
mod terminal_probe;
mod terminal_title;
mod terminal_visualization_instructions;
mod text_formatting;
mod theme_picker;
mod thread_transcript;
mod token_usage;
mod tooltips;
mod transcript_reflow;
mod tui;
mod ui_consts;
mod unarchive_prompt;
pub(crate) mod update_action;
pub use update_action::UpdateAction;
#[cfg(not(debug_assertions))]
pub use update_action::get_update_action;
mod update_prompt;
#[cfg(any(not(debug_assertions), test))]
mod update_versions;
mod updates;
#[cfg(any(not(debug_assertions), test))]
mod updates_cache;
mod version;
mod width;
#[cfg(any(target_os = "windows", test))]
mod windows_sandbox;
mod workspace_command;
mod workspace_messages;

mod wrapping;

mod table_detect;
#[cfg(test)]
pub(crate) mod test_backend;
#[cfg(test)]
pub(crate) mod test_support;

use crate::onboarding::onboarding_screen::OnboardingScreenArgs;
use crate::onboarding::onboarding_screen::run_onboarding_app;
use crate::startup_hooks_review::StartupHooksReviewOutcome;
use crate::startup_hooks_review::load_startup_hooks_review_entry;
use crate::startup_hooks_review::maybe_run_startup_hooks_review;
use crate::tui::Tui;
pub use cli::Cli;
use codex_arg0::Arg0DispatchPaths;
pub use markdown_render::render_markdown_text;
pub use public_widgets::composer_input::ComposerAction;
pub use public_widgets::composer_input::ComposerInput;
// (tests access modules directly within the crate)

const TUI_LOG_FILE_NAME: &str = "codex-tui.log";
const INTERACTIVE_OTEL_SHUTDOWN_TIMEOUT: Duration = Duration::from_millis(/*millis*/ 500);

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
    cloud_config_bundle: CloudConfigBundleLoader,
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
        cloud_config_bundle,
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
    LocalDaemon { endpoint: RemoteAppServerEndpoint },
    Remote { endpoint: RemoteAppServerEndpoint },
}

impl AppServerTarget {
    pub(crate) fn uses_remote_workspace(&self) -> bool {
        matches!(self, Self::Remote { .. })
    }

    fn auth_config_for_cloud_loader(&self, mut auth_config: AuthConfig) -> AuthConfig {
        if self.uses_remote_workspace() {
            // Remove local auth restrictions before loading credentials for a remote
            // workspace; the remote app server enforces its own authentication policy.
            auth_config.forced_login_method = None;
            auth_config.forced_chatgpt_workspace_id = None;
            auth_config.managed_auth_policy = Default::default();
        }
        auth_config
    }

    fn thread_params_mode(&self) -> ThreadParamsMode {
        if self.uses_remote_workspace() {
            ThreadParamsMode::Remote
        } else {
            ThreadParamsMode::Embedded
        }
    }
}

async fn init_state_db_for_app_server_target(
    config: &Config,
    app_server_target: &AppServerTarget,
) -> std::io::Result<Option<StateDbHandle>> {
    match app_server_target {
        AppServerTarget::Embedded => state_db::try_init(config).await.map(Some).map_err(|err| {
            let database_path = codex_state::runtime_db_path_for_corruption_error(&err)
                .unwrap_or_else(|| config.sqlite_config().state_db_path());
            std::io::Error::other(LocalStateDbStartupError::new(
                database_path,
                format!("{err:#}"),
            ))
        }),
        AppServerTarget::LocalDaemon { .. } | AppServerTarget::Remote { .. } => {
            Ok(state_db::get_state_db(config).await)
        }
    }
}

// TODO(jif) delete after 22/11/2026.
fn remove_legacy_tui_log_file(codex_home: &Path) {
    // Shared append-only TUI logs could grow without bound. Existing processes
    // may still hold the file open, so startup cleanup is best effort.
    let _ = std::fs::remove_file(codex_home.join("log").join(TUI_LOG_FILE_NAME));
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
        mcp_server_openai_form_elicitation: false,
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
    cloud_config_bundle: CloudConfigBundleLoader,
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
            cloud_config_bundle,
            feedback,
            log_db,
            state_db,
            environment_manager,
        )
        .await
        .map(AppServerClient::InProcess),
        AppServerTarget::LocalDaemon { endpoint } | AppServerTarget::Remote { endpoint } => {
            connect_remote_app_server(endpoint.clone()).await
        }
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
        CloudConfigBundleLoader::default(),
        codex_feedback::CodexFeedback::new(),
        /*log_db*/ None,
        state_db,
        environment_manager,
    )
    .await?;
    Ok(AppServerSession::new(app_server, target.thread_params_mode()).with_startup_config(config))
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
    cloud_config_bundle: CloudConfigBundleLoader,
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
        cloud_config_bundle,
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
        mcp_server_openai_form_elicitation: false,
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

/// Shut down the startup app server before restoring its terminal and ending its session.
async fn shutdown_startup_session(
    app_server: Option<AppServerSession>,
    terminal_restore_guard: &mut TerminalRestoreGuard,
) {
    shutdown_app_server_if_present(app_server).await;
    terminal_restore_guard.restore_silently();
    session_log::log_session_end();
}

fn session_target_from_app_server_thread(
    thread: AppServerThread,
) -> Option<resume_picker::SessionTarget> {
    match ThreadId::from_string(&thread.id) {
        Ok(thread_id) => Some(resume_picker::SessionTarget {
            path: thread.path,
            thread_id,
            history_mode: Some(thread.history_mode),
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

pub(crate) fn resume_source_kinds(include_non_interactive: bool) -> Vec<ThreadSourceKind> {
    let mut source_kinds = vec![ThreadSourceKind::Cli, ThreadSourceKind::VsCode];
    if include_non_interactive {
        // `thread/list` treats omitted and empty `sourceKinds` as interactive-only,
        // so include-non-interactive has to name the user-resumable non-interactive
        // sources explicitly until the API grows an unfiltered request.
        source_kinds.extend([ThreadSourceKind::Exec, ThreadSourceKind::AppServer]);
    }
    source_kinds
}

async fn lookup_session_target_with_app_server(
    app_server: &mut AppServerSession,
    config: &Config,
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

    named_session_lookup::lookup(app_server, config, id_or_name).await
}

async fn lookup_latest_session_target_with_app_server(
    app_server: &mut AppServerSession,
    config: &Config,
    cwd_filter: Option<&Path>,
    include_non_interactive: bool,
) -> color_eyre::Result<Option<resume_picker::SessionTarget>> {
    let uses_remote_workspace = app_server.uses_remote_workspace();
    for lookup_mode in [
        LatestSessionLookupMode::StateDbOnly,
        LatestSessionLookupMode::ScanAndRepair,
    ] {
        let response = app_server
            .thread_list(latest_session_lookup_params(
                uses_remote_workspace,
                config,
                cwd_filter,
                include_non_interactive,
                lookup_mode,
            ))
            .await?;
        let target = response
            .data
            .into_iter()
            .find_map(session_target_from_app_server_thread);
        if target.as_ref().is_some_and(|target| {
            uses_remote_workspace || target.path.as_deref().is_some_and(std::path::Path::exists)
        }) {
            return Ok(target);
        }
    }
    Ok(None)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LatestSessionLookupMode {
    StateDbOnly,
    ScanAndRepair,
}

fn latest_session_lookup_params(
    uses_remote_workspace: bool,
    config: &Config,
    cwd_filter: Option<&Path>,
    include_non_interactive: bool,
    lookup_mode: LatestSessionLookupMode,
) -> ThreadListParams {
    ThreadListParams {
        cursor: None,
        limit: Some(1),
        sort_key: Some(AppServerThreadSortKey::UpdatedAt),
        sort_direction: None,
        model_providers: if uses_remote_workspace {
            None
        } else {
            Some(vec![config.model_provider_id.clone()])
        },
        source_kinds: Some(resume_source_kinds(include_non_interactive)),
        archived: Some(false),
        section_id: None,
        project_id: None,
        parent_thread_id: None,
        ancestor_thread_id: None,
        cwd: cwd_filter.map(|cwd| ThreadListCwdFilter::One(cwd.to_string_lossy().to_string())),
        use_state_db_only: match lookup_mode {
            LatestSessionLookupMode::StateDbOnly => true,
            LatestSessionLookupMode::ScanAndRepair => false,
        },
        search_term: None,
    }
}

fn config_cwd_for_app_server_target(
    cwd: Option<&Path>,
    app_server_target: &AppServerTarget,
    default_environment_is_remote: bool,
) -> std::io::Result<Option<AbsolutePathBuf>> {
    if app_server_target.uses_remote_workspace() || default_environment_is_remote {
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

fn uses_remote_workspace_or_environment(
    app_server_target: &AppServerTarget,
    environment_manager: &EnvironmentManager,
) -> bool {
    app_server_target.uses_remote_workspace()
        || environment_manager
            .default_environment()
            .is_some_and(|environment| environment.is_remote())
}

async fn resolve_startup_resume_or_fork_cwd(
    tui: &mut Tui,
    config: &Config,
    state_db: Option<&codex_state::StateRuntime>,
    session_selection: &resume_picker::SessionSelection,
    cwd_override: Option<&Path>,
    uses_remote_workspace: bool,
    uses_remote_workspace_or_environment: bool,
) -> color_eyre::Result<ResolveCwdOutcome> {
    let Some((action, target_session)) = (match session_selection {
        resume_picker::SessionSelection::Resume(target_session) => {
            Some((CwdPromptAction::Resume, target_session))
        }
        resume_picker::SessionSelection::Fork(target_session) => {
            Some((CwdPromptAction::Fork, target_session))
        }
        _ => None,
    }) else {
        return Ok(ResolveCwdOutcome::Continue(None));
    };
    let resume_cwd_mode = effective_resume_cwd_mode(config.tui_resume_cwd, cwd_override);
    if uses_remote_workspace_or_environment
        && cwd_override.is_none()
        && matches!(resume_cwd_mode, Some(ResumeCwdMode::Current))
    {
        color_eyre::eyre::bail!(
            "`tui.resume_cwd = \"current\"` requires `--cd` when using a remote workspace"
        );
    }
    if uses_remote_workspace {
        return Ok(ResolveCwdOutcome::Continue(Some(config.cwd.to_path_buf())));
    }

    resolve_cwd_for_resume_or_fork(
        tui,
        config,
        state_db,
        target_session,
        action,
        ResumeCwdContext {
            current_cwd: config.cwd.as_path(),
            remembered_current_cwd: config.cwd.as_path(),
            allow_remember_current: !uses_remote_workspace_or_environment || cwd_override.is_some(),
            mode: resume_cwd_mode,
        },
    )
    .await
}

fn should_load_configured_environments(
    loader_overrides: &LoaderOverrides,
    app_server_target: &AppServerTarget,
) -> bool {
    !loader_overrides.ignore_user_config && !app_server_target.uses_remote_workspace()
}

fn latest_session_cwd_filter<'a>(
    uses_remote_workspace: bool,
    remote_cwd_override: Option<&'a Path>,
    config: &'a Config,
    show_all: bool,
) -> Option<&'a Path> {
    if show_all {
        return None;
    }

    if uses_remote_workspace {
        remote_cwd_override
    } else {
        Some(config.cwd.as_path())
    }
}

fn app_server_target_for_launch(
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
    default_daemon_socket: Option<AbsolutePathBuf>,
    can_reuse_implicit_local_daemon: bool,
    workload_identity_selected: bool,
) -> std::io::Result<AppServerTarget> {
    if workload_identity_selected {
        if explicit_remote_endpoint.is_some() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "workload identity must be configured on the remote app-server host",
            ));
        }
        return Ok(AppServerTarget::Embedded);
    }
    Ok(match explicit_remote_endpoint {
        Some(endpoint) => AppServerTarget::Remote { endpoint },
        None if can_reuse_implicit_local_daemon => {
            default_daemon_socket.map_or(AppServerTarget::Embedded, |socket_path| {
                AppServerTarget::LocalDaemon {
                    endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
                }
            })
        }
        None => AppServerTarget::Embedded,
    })
}

async fn cloud_config_bundle_for_app_server_target(
    app_server_target: &AppServerTarget,
    bootstrap_config: &ConfigTomlLoadResult,
    codex_home: &Path,
) -> std::io::Result<CloudConfigBundleLoader> {
    cloud_config_bundle_loader_for_storage(
        app_server_target
            .auth_config_for_cloud_loader(bootstrap_auth_config(codex_home, bootstrap_config)?),
        /*enable_codex_api_key_env*/ false,
    )
    .await
}

fn loader_overrides_are_default(loader_overrides: &LoaderOverrides) -> bool {
    let loader_overrides_are_default = loader_overrides.user_config_path.is_none()
        && loader_overrides.user_config_profile.is_none()
        && loader_overrides.managed_config_path.is_none()
        && loader_overrides.system_config_path.is_none()
        && loader_overrides.system_requirements_path.is_none()
        && !loader_overrides.ignore_managed_requirements
        && !loader_overrides.ignore_user_config
        && !loader_overrides.ignore_user_and_project_exec_policy_rules
        && loader_overrides
            .macos_managed_config_requirements_base64
            .is_none();
    #[cfg(target_os = "macos")]
    let loader_overrides_are_default =
        loader_overrides_are_default && loader_overrides.managed_preferences_base64.is_none();
    loader_overrides_are_default
}

fn can_reuse_implicit_local_daemon(
    cli_kv_overrides: &[(String, toml::Value)],
    loader_overrides: &LoaderOverrides,
    strict_config: bool,
    has_non_replayable_launch_overrides: bool,
) -> bool {
    // A reused daemon cannot adopt this invocation's full launch config state.
    cli_kv_overrides.is_empty()
        && loader_overrides_are_default(loader_overrides)
        && !strict_config
        && !has_non_replayable_launch_overrides
}

/// Restore terminal modes before a fatal startup exit bypasses destructor cleanup.
fn restore_terminal_before_fatal_exit() {
    if crossterm::terminal::is_raw_mode_enabled().unwrap_or(false) {
        let _ = tui::restore_after_exit();
    }
}

pub async fn run_main(
    cli: Cli,
    arg0_paths: Arg0DispatchPaths,
    loader_overrides: LoaderOverrides,
    explicit_remote_endpoint: Option<RemoteAppServerEndpoint>,
) -> std::io::Result<AppExitInfo> {
    match startup_orchestration::run_main_inner(
        cli,
        arg0_paths,
        loader_overrides,
        explicit_remote_endpoint,
    )
    .await
    {
        Err(err) if startup_draft::StartupCancelled::matches(&err) => Ok(AppExitInfo {
            token_usage: TokenUsage::default(),
            thread_id: None,
            resume_hint: None,
            update_action: None,
            exit_reason: ExitReason::UserRequested,
        }),
        result => result,
    }
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
    manually_selected_oss_provider: Option<String>,
    overrides: ConfigOverrides,
    cli_kv_overrides: Vec<(String, toml::Value)>,
    mut cloud_config_bundle: CloudConfigBundleLoader,
    feedback: codex_feedback::CodexFeedback,
    log_db: Option<log_db::LogDbLayer>,
    state_db: Option<StateDbHandle>,
    environment_manager: Arc<EnvironmentManager>,
    startup_draft: startup_draft::StartupDraft,
) -> color_eyre::Result<AppExitInfo> {
    let uses_remote_workspace = app_server_target.uses_remote_workspace();
    let workload_identity_selected = is_workload_identity_selected();
    color_eyre::install()?;

    tooltips::announcement::prewarm(initial_config.http_client_factory());

    // Forward panic reports through tracing so they appear in the UI status
    // line, but do not swallow the default/color-eyre panic handler.
    // Chain to the previous hook so users still get a rich panic report
    // (including backtraces) after we restore the terminal.
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = tui::restore_after_exit();
        tracing::error!("panic: {info}");
        prev_hook(info);
    }));
    let (mut tui, mut terminal_restore_guard, mut startup_draft) = startup_draft.into_parts();

    #[cfg(not(debug_assertions))]
    {
        use crate::update_prompt::UpdatePromptOutcome;

        let skip_update_prompt = cli.prompt.as_ref().is_some_and(|prompt| !prompt.is_empty());
        if !skip_update_prompt {
            startup_draft.flush_pending_events(&mut tui).await?;
            match update_prompt::run_update_prompt_if_needed(&mut tui, &initial_config).await? {
                UpdatePromptOutcome::Continue => {}
                UpdatePromptOutcome::RunUpdate(action) => {
                    terminal_restore_guard.restore()?;
                    return Ok(AppExitInfo {
                        token_usage: crate::token_usage::TokenUsage::default(),
                        thread_id: None,
                        resume_hint: None,
                        update_action: Some(action),
                        exit_reason: ExitReason::UserRequested,
                    });
                }
            }
        }
    }

    // Initialize high-fidelity session event logging if enabled.
    session_log::maybe_init(&initial_config);

    let startup_app_server = startup_draft
        .run_until(
            &mut tui,
            start_app_server(
                &app_server_target,
                arg0_paths.clone(),
                initial_config.clone(),
                cli_kv_overrides.clone(),
                loader_overrides.clone(),
                strict_config,
                cloud_config_bundle.clone(),
                feedback.clone(),
                log_db.clone(),
                state_db.clone(),
                environment_manager.clone(),
            ),
        )
        .await;
    let app_server_session = match startup_app_server {
        Ok(Ok(app_server)) => {
            AppServerSession::new(app_server, app_server_target.thread_params_mode())
                .with_startup_config(&initial_config)
        }
        Ok(Err(err)) => {
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            return Err(err);
        }
        Err(err) => {
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            return Err(err.into());
        }
    }
    .with_remote_cwd_override(remote_cwd_override.clone());
    if let Some(provider) = manually_selected_oss_provider.as_deref() {
        match startup_draft
            .run_until(
                &mut tui,
                config_update::write_config_batch(
                    app_server_session.request_handle(),
                    vec![config_update::build_oss_provider_edit(provider)],
                ),
            )
            .await
        {
            Ok(Ok(_)) => {}
            Ok(Err(err)) => {
                warn!(
                    %err,
                    provider,
                    "Failed to persist selected OSS provider preference"
                );
            }
            Err(err) => {
                shutdown_startup_session(Some(app_server_session), &mut terminal_restore_guard)
                    .await;
                return Err(err.into());
            }
        }
    }
    let remote_project_trust =
        if uses_remote_workspace && let Some(remote_cwd) = remote_cwd_override.as_deref() {
            match startup_draft
                .run_until(
                    &mut tui,
                    config_update::read_remote_project_trust(
                        app_server_session.request_handle(),
                        remote_cwd,
                    ),
                )
                .await
            {
                Ok(Ok(remote_project_trust)) => remote_project_trust,
                Ok(Err(err)) => {
                    shutdown_startup_session(Some(app_server_session), &mut terminal_restore_guard)
                        .await;
                    return Err(err);
                }
                Err(err) => {
                    shutdown_startup_session(Some(app_server_session), &mut terminal_restore_guard)
                        .await;
                    return Err(err.into());
                }
            }
        } else {
            None
        };
    let mut app_server = Some(app_server_session);
    let should_show_trust_screen_flag = remote_project_trust.is_some()
        || (!uses_remote_workspace && should_show_trust_screen(&initial_config));
    #[cfg(target_os = "windows")]
    let mut trust_decision_was_made = false;
    let startup_model_provider = initial_config.model_provider_id.clone();
    let (login_status, mut startup_account) = if workload_identity_selected {
        (LoginStatus::AuthMode(AuthMode::Chatgpt), None)
    } else if initial_config.model_provider.requires_openai_auth {
        let Some(active_app_server) = app_server.as_mut() else {
            unreachable!("app server should exist when auth is required");
        };
        let login_status = startup_draft
            .run_until(&mut tui, get_login_status(active_app_server))
            .await;
        match login_status {
            Ok(Ok((login_status, account))) => (login_status, Some(account)),
            Ok(Err(err)) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err);
            }
            Err(err) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err.into());
            }
        }
    } else {
        (LoginStatus::NotAuthenticated, None)
    };
    let should_show_onboarding =
        should_show_onboarding(login_status, &initial_config, should_show_trust_screen_flag);

    let config = if should_show_onboarding {
        if let Err(err) = startup_draft.flush_pending_events(&mut tui).await {
            shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
            return Err(err.into());
        }
        // Authentication can change while any interactive onboarding screen is open.
        startup_account = None;
        let show_login_screen = should_show_login_screen(login_status, &initial_config);
        let onboarding_result = run_onboarding_app(
            OnboardingScreenArgs {
                show_login_screen,
                show_trust_screen: should_show_trust_screen_flag,
                remote_project_trust,
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
        .await;
        let onboarding_result = match onboarding_result {
            Ok(onboarding_result) => onboarding_result,
            Err(err) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err);
            }
        };
        if onboarding_result.should_exit {
            shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
            let _ = tui.terminal.clear();
            return Ok(AppExitInfo {
                token_usage: crate::token_usage::TokenUsage::default(),
                thread_id: None,
                resume_hint: None,
                update_action: None,
                exit_reason: ExitReason::UserRequested,
            });
        }
        #[cfg(target_os = "windows")]
        {
            trust_decision_was_made =
                !uses_remote_workspace && onboarding_result.directory_trust_persisted;
        }
        let reloaded_config = startup_draft
            .run_until(&mut tui, async {
                // If this onboarding run included the login step, always refresh the cloud config
                // bundle and rebuild config. This avoids missing newly available cloud-managed
                // policy due to login status detection edge cases.
                if show_login_screen && !uses_remote_workspace && !workload_identity_selected {
                    cloud_config_bundle = cloud_config_bundle_loader_for_storage(
                        initial_config.auth_config(),
                        /*enable_codex_api_key_env*/ false,
                    )
                    .await?;
                }

                // Reload config when persisted trust or auth changes alter the current process.
                Ok::<_, std::io::Error>(
                    if !uses_remote_workspace
                        && (onboarding_result.directory_trust_persisted || show_login_screen)
                    {
                        load_config_or_exit(
                            cli_kv_overrides.clone(),
                            overrides.clone(),
                            loader_overrides.clone(),
                            cloud_config_bundle.clone(),
                            strict_config,
                        )
                        .await
                    } else {
                        initial_config
                    },
                )
            })
            .await;
        match reloaded_config {
            Ok(Ok(config)) => config,
            Ok(Err(err)) | Err(err) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err.into());
            }
        }
    } else {
        initial_config
    };
    startup_draft.apply_config(&config);
    if !(cli.resume_picker || cli.fork_picker || cli.agents_overview)
        && let Err(err) = startup_draft.show(&mut tui)
    {
        shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
        return Err(err.into());
    }

    let missing_session_exit =
        |id_str: &str,
         action: &str,
         tui: &mut Tui,
         terminal_restore_guard: &mut TerminalRestoreGuard| {
            error!("Error finding conversation path: {id_str}");
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            let _ = tui.terminal.clear();
            Ok(AppExitInfo {
                token_usage: crate::token_usage::TokenUsage::default(),
                thread_id: None,
                resume_hint: None,
                update_action: None,
                exit_reason: ExitReason::Fatal(format!(
                    "No saved session found with ID {id_str}. Run `codex {action}` without an ID to choose from existing sessions."
                )),
            })
        };

    let use_fork = cli.fork_picker || cli.fork_last || cli.fork_session_id.is_some();
    let session_selection = if cli.agents_overview {
        resume_picker::SessionSelection::AgentsOverview
    } else if use_fork {
        if let Some(id_str) = cli.fork_session_id.as_deref() {
            let Some(startup_app_server) = app_server.as_mut() else {
                unreachable!("app server should be initialized for --fork <id>");
            };
            let lookup = startup_draft
                .run_until(
                    &mut tui,
                    lookup_session_target_with_app_server(startup_app_server, &config, id_str),
                )
                .await;
            let target_session = match lookup {
                Ok(result) => result?,
                Err(err) => {
                    shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                    return Err(err.into());
                }
            };
            match target_session {
                Some(target_session) => resume_picker::SessionSelection::Fork(target_session),
                None => {
                    shutdown_app_server_if_present(app_server.take()).await;
                    return missing_session_exit(
                        id_str,
                        "fork",
                        &mut tui,
                        &mut terminal_restore_guard,
                    );
                }
            }
        } else if cli.fork_last {
            let filter_cwd = latest_session_cwd_filter(
                uses_remote_workspace,
                remote_cwd_override.as_deref(),
                &config,
                cli.fork_show_all,
            );
            let Some(startup_app_server) = app_server.as_mut() else {
                unreachable!("app server should be initialized for --fork --last");
            };
            let lookup = startup_draft
                .run_until(
                    &mut tui,
                    lookup_latest_session_target_with_app_server(
                        startup_app_server,
                        &config,
                        filter_cwd,
                        /*include_non_interactive*/ false,
                    ),
                )
                .await;
            let target_session = match lookup {
                Ok(result) => result?,
                Err(err) => {
                    shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                    return Err(err.into());
                }
            };
            match target_session {
                Some(target_session) => resume_picker::SessionSelection::Fork(target_session),
                None => resume_picker::SessionSelection::StartFresh,
            }
        } else if cli.fork_picker {
            if let Err(err) = startup_draft.flush_pending_events(&mut tui).await {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err.into());
            }
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
                        resume_hint: None,
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
        let lookup = startup_draft
            .run_until(
                &mut tui,
                lookup_session_target_with_app_server(startup_app_server, &config, id_str),
            )
            .await;
        let target_session = match lookup {
            Ok(result) => result?,
            Err(err) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err.into());
            }
        };
        match target_session {
            Some(target_session) => resume_picker::SessionSelection::Resume(target_session),
            None => {
                shutdown_app_server_if_present(app_server.take()).await;
                return missing_session_exit(
                    id_str,
                    "resume",
                    &mut tui,
                    &mut terminal_restore_guard,
                );
            }
        }
    } else if cli.resume_last {
        let filter_cwd = latest_session_cwd_filter(
            uses_remote_workspace,
            remote_cwd_override.as_deref(),
            &config,
            cli.resume_show_all,
        );
        let Some(startup_app_server) = app_server.as_mut() else {
            unreachable!("app server should be initialized for --resume --last");
        };
        let lookup = startup_draft
            .run_until(
                &mut tui,
                lookup_latest_session_target_with_app_server(
                    startup_app_server,
                    &config,
                    filter_cwd,
                    cli.resume_include_non_interactive,
                ),
            )
            .await;
        let target_session = match lookup {
            Ok(result) => result?,
            Err(err) => {
                shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
                return Err(err.into());
            }
        };
        match target_session {
            Some(target_session) => resume_picker::SessionSelection::Resume(target_session),
            None => resume_picker::SessionSelection::StartFresh,
        }
    } else if cli.resume_picker {
        if let Err(err) = startup_draft.flush_pending_events(&mut tui).await {
            shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
            return Err(err.into());
        }
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
                    resume_hint: None,
                    update_action: None,
                    exit_reason: ExitReason::UserRequested,
                });
            }
            other => other,
        }
    } else {
        resume_picker::SessionSelection::StartFresh
    };

    if let Err(err) = startup_draft.update_session_selection(&mut tui, &session_selection) {
        shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
        return Err(err.into());
    }

    if matches!(
        &session_selection,
        resume_picker::SessionSelection::Resume(_) | resume_picker::SessionSelection::Fork(_)
    ) && let Err(err) = startup_draft.flush_pending_events(&mut tui).await
    {
        shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
        return Err(err.into());
    }

    let current_cwd = config.cwd.clone();
    let fallback_cwd = match resolve_startup_resume_or_fork_cwd(
        &mut tui,
        &config,
        state_db.as_deref(),
        &session_selection,
        cli.cwd.as_deref(),
        uses_remote_workspace,
        uses_remote_workspace_or_environment(&app_server_target, &environment_manager),
    )
    .await
    {
        Ok(ResolveCwdOutcome::Continue(cwd)) => cwd,
        Ok(ResolveCwdOutcome::ContinueAfterPrompt(cwd)) => {
            // Another daemon client can change authentication while this prompt is open.
            startup_account = None;
            Some(cwd)
        }
        Ok(ResolveCwdOutcome::Exit) => {
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            return Ok(AppExitInfo {
                token_usage: crate::token_usage::TokenUsage::default(),
                thread_id: None,
                resume_hint: None,
                update_action: None,
                exit_reason: ExitReason::UserRequested,
            });
        }
        Err(err) => {
            terminal_restore_guard.restore_silently();
            session_log::log_session_end();
            return Err(err);
        }
    };

    if (cli.resume_picker || cli.fork_picker)
        && let Err(err) = startup_draft.show(&mut tui)
    {
        shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
        return Err(err.into());
    }

    let picker_cancelled_without_selection = matches!(
        session_selection,
        resume_picker::SessionSelection::StartFresh
    ) && (cli.resume_picker || cli.fork_picker);

    let reloaded_config = match &session_selection {
        resume_picker::SessionSelection::Resume(_) | resume_picker::SessionSelection::Fork(_) => {
            startup_draft
                .run_until(
                    &mut tui,
                    load_config_or_exit_with_fallback_cwd(
                        cli_kv_overrides.clone(),
                        overrides.clone(),
                        loader_overrides.clone(),
                        cloud_config_bundle.clone(),
                        strict_config,
                        fallback_cwd,
                    ),
                )
                .await
        }
        resume_picker::SessionSelection::StartFresh if picker_cancelled_without_selection => {
            startup_draft
                .run_until(
                    &mut tui,
                    load_config_or_exit(
                        cli_kv_overrides.clone(),
                        overrides.clone(),
                        loader_overrides.clone(),
                        cloud_config_bundle.clone(),
                        strict_config,
                    ),
                )
                .await
        }
        _ => Ok(config),
    };
    let mut config = match reloaded_config {
        Ok(config) => config,
        Err(err) => {
            shutdown_startup_session(app_server.take(), &mut terminal_restore_guard).await;
            return Err(err.into());
        }
    };
    startup_draft.apply_config(&config);

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
    let should_show_trust_screen = should_show_trust_screen(&config);
    #[cfg(target_os = "windows")]
    let windows_sandbox_level = crate::windows_sandbox::level_from_config(&config);
    #[cfg(target_os = "windows")]
    let required_elevated_sandbox_needs_setup = windows_sandbox_level
        == WindowsSandboxLevel::Elevated
        && config
            .config_layer_stack
            .requirements()
            .windows_sandbox_mode
            .source
            .is_some()
        && !crate::windows_sandbox::sandbox_setup_is_complete(config.codex_home.as_path());
    #[cfg(target_os = "windows")]
    let should_prompt_windows_sandbox_nux_at_startup = (trust_decision_was_made
        && windows_sandbox_level == WindowsSandboxLevel::Disabled)
        || required_elevated_sandbox_needs_setup;
    #[cfg(not(target_os = "windows"))]
    let should_prompt_windows_sandbox_nux_at_startup = false;

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
        None => match startup_draft
            .run_until(
                &mut tui,
                start_app_server(
                    &app_server_target,
                    arg0_paths,
                    config.clone(),
                    cli_kv_overrides.clone(),
                    loader_overrides.clone(),
                    strict_config,
                    cloud_config_bundle.clone(),
                    feedback.clone(),
                    log_db.clone(),
                    state_db.clone(),
                    environment_manager.clone(),
                ),
            )
            .await
        {
            Ok(Ok(app_server)) => {
                // A picker can replace the server; account reads belong to their original session.
                startup_account = None;
                AppServerSession::new(app_server, app_server_target.thread_params_mode())
                    .with_startup_config(&config)
                    .with_remote_cwd_override(remote_cwd_override.clone())
            }
            Ok(Err(err)) => {
                terminal_restore_guard.restore_silently();
                session_log::log_session_end();
                return Err(err);
            }
            Err(err) => {
                terminal_restore_guard.restore_silently();
                session_log::log_session_end();
                return Err(err.into());
            }
        },
    };

    // Persistent app-server resumes may attach to an already-running thread,
    // where resume config overrides are ignored.
    let is_persistent_resume = !matches!(&app_server_target, AppServerTarget::Embedded)
        && matches!(
            &session_selection,
            resume_picker::SessionSelection::Resume(_)
        );
    let bypass_hook_trust_for_startup_review = config.bypass_hook_trust && !is_persistent_resume;
    let hooks_request_handle = app_server.request_handle();
    let hooks_cwd = config.cwd.to_path_buf();
    if config.model_provider_id != startup_model_provider {
        startup_account = None;
    }
    let startup_prefetch_started_at = Instant::now();
    let startup_prefetch = startup_draft
        .run_until(&mut tui, async {
            tokio::join!(
                async {
                    match startup_account {
                        Some(account) => app_server.bootstrap_with_account(&config, account).await,
                        None => app_server.bootstrap(&config).await,
                    }
                },
                load_startup_hooks_review_entry(hooks_request_handle, hooks_cwd),
            )
        })
        .await;
    let (startup_bootstrap, startup_hooks_entry) = match startup_prefetch {
        Ok(startup_prefetch) => startup_prefetch,
        Err(err) => {
            shutdown_startup_session(Some(app_server), &mut terminal_restore_guard).await;
            return Err(err.into());
        }
    };
    if let Err(err) = startup_draft.flush_pending_events(&mut tui).await {
        shutdown_startup_session(Some(app_server), &mut terminal_restore_guard).await;
        return Err(err.into());
    }
    let startup_bootstrap = match startup_bootstrap {
        Ok(startup_bootstrap) => Some(startup_bootstrap),
        Err(err) => {
            shutdown_startup_session(Some(app_server), &mut terminal_restore_guard).await;
            return Err(err);
        }
    };
    let startup_elapsed_before_app = startup_prefetch_started_at.elapsed();
    let startup_hooks_review = maybe_run_startup_hooks_review(
        &mut app_server,
        &mut tui,
        &config,
        bypass_hook_trust_for_startup_review,
        startup_hooks_entry,
    )
    .await;
    let startup_hooks_browser = match startup_hooks_review {
        Err(err) => {
            shutdown_startup_session(Some(app_server), &mut terminal_restore_guard).await;
            return Err(err);
        }
        Ok(StartupHooksReviewOutcome::Continue) => None,
        Ok(StartupHooksReviewOutcome::OpenHooksBrowser(data)) => Some(data),
    };

    let app_result = App::run(
        &mut tui,
        app_server,
        config,
        current_cwd.to_path_buf(),
        cli_kv_overrides.clone(),
        overrides.clone(),
        loader_overrides.clone(),
        cloud_config_bundle,
        prompt,
        images,
        session_selection,
        feedback,
        should_show_trust_screen, // Proxy to: is it a first run in this directory?
        should_prompt_windows_sandbox_nux_at_startup,
        app_server_target,
        state_db,
        environment_manager,
        startup_elapsed_before_app,
        startup_bootstrap,
        startup_hooks_browser,
        startup_draft,
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
    AuthMode(AuthMode),
    NotAuthenticated,
}

/// Reads the account once to determine login status and preserve the response for bootstrap.
async fn get_login_status(
    app_server: &mut AppServerSession,
) -> color_eyre::Result<(LoginStatus, GetAccountResponse)> {
    let account = app_server.read_account().await?;
    let login_status = match &account.account {
        Some(AppServerAccount::ApiKey {}) => LoginStatus::AuthMode(AuthMode::ApiKey),
        Some(AppServerAccount::Chatgpt { .. }) => LoginStatus::AuthMode(AuthMode::Chatgpt),
        Some(AppServerAccount::AmazonBedrock { .. }) | None => LoginStatus::NotAuthenticated,
    };
    Ok((login_status, account))
}

async fn load_config_or_exit(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
    loader_overrides: LoaderOverrides,
    cloud_config_bundle: CloudConfigBundleLoader,
    strict_config: bool,
) -> Config {
    load_config_or_exit_with_fallback_cwd(
        cli_kv_overrides,
        overrides,
        loader_overrides,
        cloud_config_bundle,
        strict_config,
        /*fallback_cwd*/ None,
    )
    .await
}

async fn load_config_or_exit_with_fallback_cwd(
    cli_kv_overrides: Vec<(String, toml::Value)>,
    overrides: ConfigOverrides,
    loader_overrides: LoaderOverrides,
    cloud_config_bundle: CloudConfigBundleLoader,
    strict_config: bool,
    fallback_cwd: Option<PathBuf>,
) -> Config {
    #[allow(clippy::print_stderr)]
    match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides)
        .harness_overrides(overrides)
        .loader_overrides(loader_overrides)
        .strict_config(strict_config)
        .cloud_config_bundle(cloud_config_bundle)
        .fallback_cwd(fallback_cwd)
        .build()
        .await
    {
        Ok(config) => config,
        Err(err) => {
            restore_terminal_before_fatal_exit();
            eprintln!("Error loading configuration: {err}");
            std::process::exit(1);
        }
    }
}

#[allow(clippy::print_stderr)]
async fn load_bootstrap_config_or_exit(
    codex_home: &Path,
    cwd: Option<&AbsolutePathBuf>,
    cli_kv_overrides: Vec<(String, codex_config::TomlValue)>,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    cloud_config_bundle: CloudConfigBundleLoader,
) -> ConfigTomlLoadResult {
    match load_config_toml_with_layer_stack(
        codex_home,
        cwd,
        cli_kv_overrides,
        codex_config::ConfigLoadOptions {
            loader_overrides,
            strict_config,
            cloud_config_bundle,
        },
    )
    .await
    {
        Ok(config_toml) => config_toml,
        Err(err) => {
            restore_terminal_before_fatal_exit();
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
    use codex_utils_absolute_path::test_support::PathExt;
    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use tempfile::TempDir;

    async fn build_config(temp_dir: &TempDir) -> std::io::Result<Config> {
        ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .build()
            .await
    }

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
        let parent = rollout_path
            .parent()
            .ok_or_else(|| color_eyre::eyre::eyre!("rollout path is missing a parent directory"))?;
        std::fs::create_dir_all(parent)?;

        let session_meta = codex_protocol::protocol::SessionMeta {
            session_id: thread_id.into(),
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
        std::fs::OpenOptions::new()
            .append(true)
            .open(rollout_path)?
            .set_times(times)?;

        Ok(thread_id)
    }

    #[test]
    fn startup_removes_legacy_tui_log_file() -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let legacy_log_dir = temp_dir.path().join("log");
        std::fs::create_dir_all(&legacy_log_dir)?;
        let legacy_log = legacy_log_dir.join(TUI_LOG_FILE_NAME);
        std::fs::write(&legacy_log, "legacy log")?;

        remove_legacy_tui_log_file(temp_dir.path());

        assert!(!legacy_log.exists());
        Ok(())
    }

    #[tokio::test]
    async fn startup_services_use_final_cloud_managed_http_policy() -> color_eyre::Result<()> {
        for (configured_respect_system_proxy, managed_respect_system_proxy) in
            [(true, false), (false, true)]
        {
            let codex_home = TempDir::new()?;
            std::fs::write(
                codex_home.path().join("config.toml"),
                format!("[features]\nrespect_system_proxy = {configured_respect_system_proxy}\n"),
            )?;
            let prepared_environment_manager =
                EnvironmentManager::prepare_from_codex_home(codex_home.path()).await?;
            let loader_overrides = LoaderOverrides::without_managed_config_for_tests();
            let bootstrap_config = load_config_toml_with_layer_stack(
                codex_home.path(),
                /*cwd*/ None,
                Vec::new(),
                codex_config::ConfigLoadOptions {
                    loader_overrides: loader_overrides.clone(),
                    ..Default::default()
                },
            )
            .await?;
            let bootstrap_http_client_factory = resolve_bootstrap_http_client_factory(
                &bootstrap_config.config_toml,
                bootstrap_config
                    .config_layer_stack
                    .requirements()
                    .feature_requirements
                    .as_ref(),
            )?;
            let cloud_config_bundle =
                codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_requirement(
                    format!("[features]\nrespect_system_proxy = {managed_respect_system_proxy}\n"),
                );
            let config = ConfigBuilder::default()
                .codex_home(codex_home.path().to_path_buf())
                .loader_overrides(loader_overrides)
                .cloud_config_bundle(cloud_config_bundle)
                .build()
                .await?;
            let runtime_paths = ExecServerRuntimePaths::new(
                std::env::current_exe()?,
                /*codex_linux_sandbox_exe*/ None,
            )?;
            let environment_manager = prepared_environment_manager
                .build(Some(runtime_paths), config.http_client_factory())?;

            assert_ne!(
                bootstrap_http_client_factory.outbound_proxy_policy(),
                config.http_client_factory().outbound_proxy_policy()
            );
            assert_eq!(
                environment_manager
                    .http_client_factory()
                    .outbound_proxy_policy(),
                config.http_client_factory().outbound_proxy_policy()
            );
            assert_eq!(
                config
                    .auth_route_config()
                    .http_client_factory()
                    .outbound_proxy_policy(),
                config.http_client_factory().outbound_proxy_policy()
            );
            assert_eq!(config.respect_system_proxy, managed_respect_system_proxy);
        }

        Ok(())
    }

    pub(crate) async fn start_test_embedded_app_server(
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
            CloudConfigBundleLoader::default(),
            codex_feedback::CodexFeedback::new(),
            /*log_db*/ None,
            state_db,
            Arc::new(EnvironmentManager::default_for_tests()),
        )
        .await
    }

    #[tokio::test]
    async fn startup_resume_and_fork_use_configured_or_explicit_cwd() -> color_eyre::Result<()> {
        for (action, configured_mode, has_explicit_cwd, expected_directory) in [
            (CwdPromptAction::Resume, "current", false, "launch"),
            (CwdPromptAction::Resume, "session", false, "session"),
            (CwdPromptAction::Resume, "session", true, "explicit"),
            (CwdPromptAction::Fork, "current", false, "launch"),
            (CwdPromptAction::Fork, "session", false, "session"),
            (CwdPromptAction::Fork, "session", true, "explicit"),
        ] {
            let temp_dir = TempDir::new()?;
            let codex_home = temp_dir.path().join("codex-home");
            let launch_cwd = temp_dir.path().join("launch");
            let session_cwd = temp_dir.path().join("session");
            let explicit_cwd = temp_dir.path().join("explicit");
            std::fs::create_dir_all(&codex_home)?;
            std::fs::create_dir_all(&launch_cwd)?;
            std::fs::create_dir_all(&session_cwd)?;
            std::fs::create_dir_all(&explicit_cwd)?;
            std::fs::write(
                codex_home.join("config.toml"),
                format!("[tui]\nresume_cwd = \"{configured_mode}\"\n"),
            )?;
            let cwd_override = has_explicit_cwd.then_some(explicit_cwd.as_path());
            let config = ConfigBuilder::default()
                .codex_home(codex_home.clone())
                .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
                .harness_overrides(ConfigOverrides {
                    cwd: Some(cwd_override.unwrap_or(launch_cwd.as_path()).to_path_buf()),
                    ..Default::default()
                })
                .build()
                .await?;
            let filename_timestamp = "2025-01-05T12-00-00";
            let thread_id = write_session_rollout(
                &codex_home,
                filename_timestamp,
                "2025-01-05T12:00:00Z",
                "Saved user message",
                &config.model_provider_id,
                &session_cwd,
            )?;
            let rollout_path = codex_home
                .join("sessions/2025/01/05")
                .join(format!("rollout-{filename_timestamp}-{thread_id}.jsonl"));
            let state_db =
                init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await?;
            let target_session = resume_picker::SessionTarget {
                path: Some(rollout_path),
                thread_id,
                history_mode: None,
            };
            let session_selection = match action {
                CwdPromptAction::Resume => resume_picker::SessionSelection::Resume(target_session),
                CwdPromptAction::Fork => resume_picker::SessionSelection::Fork(target_session),
            };
            let mut tui = tui::test_support::make_test_tui()?;

            let fallback_cwd = match resolve_startup_resume_or_fork_cwd(
                &mut tui,
                &config,
                state_db.as_deref(),
                &session_selection,
                cwd_override,
                /*uses_remote_workspace*/ false,
                /*uses_remote_workspace_or_environment*/ false,
            )
            .await?
            {
                ResolveCwdOutcome::Continue(cwd) => cwd,
                ResolveCwdOutcome::ContinueAfterPrompt(_) => {
                    panic!("configured cwd should not prompt during startup")
                }
                ResolveCwdOutcome::Exit => panic!("configured cwd should not exit startup"),
            };
            let final_config = ConfigBuilder::default()
                .codex_home(codex_home)
                .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
                .harness_overrides(ConfigOverrides {
                    cwd: cwd_override.map(Path::to_path_buf),
                    ..Default::default()
                })
                .fallback_cwd(fallback_cwd)
                .build()
                .await?;
            let expected_cwd = temp_dir.path().join(expected_directory);
            assert!(!session_resume::cwds_differ(
                final_config.cwd.as_path(),
                &expected_cwd,
            ));
            let mut app_server = start_app_server_for_picker(
                &final_config,
                &AppServerTarget::Embedded,
                state_db,
                Arc::new(EnvironmentManager::default_for_tests()),
            )
            .await?;
            let started = match action {
                CwdPromptAction::Resume => {
                    app_server
                        .resume_thread(
                            final_config,
                            thread_id,
                            app_server_session::ResumeModelSettings::RestoreFromThread,
                        )
                        .await?
                }
                CwdPromptAction::Fork => app_server.fork_thread(final_config, thread_id).await?,
            };

            assert!(!session_resume::cwds_differ(
                started.session.cwd.as_path(),
                &expected_cwd,
            ));
            app_server.shutdown().await?;
        }

        Ok(())
    }

    #[tokio::test]
    async fn startup_remote_current_cwd_without_override_is_rejected() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        std::fs::write(
            temp_dir.path().join("config.toml"),
            "[tui]\nresume_cwd = \"current\"\n",
        )?;
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let mut tui = tui::test_support::make_test_tui()?;

        let error = resolve_startup_resume_or_fork_cwd(
            &mut tui,
            &config,
            /*state_db*/ None,
            &resume_picker::SessionSelection::Resume(resume_picker::SessionTarget {
                path: None,
                thread_id: ThreadId::new(),
                history_mode: None,
            }),
            /*cwd_override*/ None,
            /*uses_remote_workspace*/ false,
            /*uses_remote_workspace_or_environment*/ true,
        )
        .await
        .expect_err("remote current cwd should require an explicit override");

        assert_eq!(
            error.to_string(),
            "`tui.resume_cwd = \"current\"` requires `--cd` when using a remote workspace"
        );
        Ok(())
    }

    #[tokio::test]
    async fn startup_session_cwd_without_metadata_is_rejected() -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        std::fs::write(
            temp_dir.path().join("config.toml"),
            "[tui]\nresume_cwd = \"session\"\n",
        )?;
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .build()
            .await?;
        let mut tui = tui::test_support::make_test_tui()?;

        let error = resolve_startup_resume_or_fork_cwd(
            &mut tui,
            &config,
            /*state_db*/ None,
            &resume_picker::SessionSelection::Resume(resume_picker::SessionTarget {
                path: None,
                thread_id: ThreadId::new(),
                history_mode: None,
            }),
            /*cwd_override*/ None,
            /*uses_remote_workspace*/ false,
            /*uses_remote_workspace_or_environment*/ false,
        )
        .await
        .expect_err("session cwd should require saved metadata");

        assert_eq!(
            error.to_string(),
            "failed to determine the working directory recorded for the selected session"
        );
        Ok(())
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
            history_mode: None,
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

    #[test]
    fn app_server_target_for_launch_uses_local_daemon_for_default_socket() -> color_eyre::Result<()>
    {
        let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
        let target = app_server_target_for_launch(
            /*explicit_remote_endpoint*/ None,
            Some(socket_path.clone()),
            /*can_reuse_implicit_local_daemon*/ true,
            /*workload_identity_selected*/ false,
        )?;

        assert_eq!(
            target,
            AppServerTarget::LocalDaemon {
                endpoint: RemoteAppServerEndpoint::UnixSocket { socket_path },
            }
        );
        assert!(!target.uses_remote_workspace());
        assert_eq!(target.thread_params_mode(), ThreadParamsMode::Embedded);
        Ok(())
    }

    #[test]
    fn app_server_target_for_launch_prefers_explicit_remote_endpoint() -> color_eyre::Result<()> {
        let explicit_endpoint = RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("explicit.sock")?,
        };
        let target = app_server_target_for_launch(
            Some(explicit_endpoint.clone()),
            Some(AbsolutePathBuf::relative_to_current_dir("default.sock")?),
            /*can_reuse_implicit_local_daemon*/ false,
            /*workload_identity_selected*/ false,
        )?;

        assert_eq!(
            target,
            AppServerTarget::Remote {
                endpoint: explicit_endpoint,
            }
        );
        assert!(target.uses_remote_workspace());
        assert_eq!(target.thread_params_mode(), ThreadParamsMode::Remote);
        Ok(())
    }

    #[test]
    fn app_server_target_for_launch_skips_local_daemon_when_launch_config_is_not_replayable()
    -> color_eyre::Result<()> {
        let socket_path = AbsolutePathBuf::relative_to_current_dir("codex.sock")?;
        let target = app_server_target_for_launch(
            /*explicit_remote_endpoint*/ None,
            Some(socket_path),
            /*can_reuse_implicit_local_daemon*/ false,
            /*workload_identity_selected*/ false,
        )?;

        assert_eq!(target, AppServerTarget::Embedded);
        Ok(())
    }

    #[test]
    fn workload_identity_requires_an_embedded_app_server() -> color_eyre::Result<()> {
        let default_socket = AbsolutePathBuf::relative_to_current_dir("default.sock")?;
        assert_eq!(
            app_server_target_for_launch(
                /*explicit_remote_endpoint*/ None,
                Some(default_socket),
                /*can_reuse_implicit_local_daemon*/ true,
                /*workload_identity_selected*/ true,
            )?,
            AppServerTarget::Embedded
        );

        let explicit_endpoint = RemoteAppServerEndpoint::UnixSocket {
            socket_path: AbsolutePathBuf::relative_to_current_dir("explicit.sock")?,
        };
        let error = app_server_target_for_launch(
            Some(explicit_endpoint),
            /*default_daemon_socket*/ None,
            /*can_reuse_implicit_local_daemon*/ false,
            /*workload_identity_selected*/ true,
        )
        .expect_err("remote hosts must own workload identity");
        assert_eq!(
            error.to_string(),
            "workload identity must be configured on the remote app-server host"
        );
        Ok(())
    }

    #[test]
    fn can_reuse_implicit_local_daemon_requires_default_launch_config() -> color_eyre::Result<()> {
        let mut loader_overrides = LoaderOverrides::default();
        let cli_kv_overrides = vec![("web_search".to_string(), toml::Value::String("live".into()))];

        assert!(can_reuse_implicit_local_daemon(
            &[],
            &LoaderOverrides::default(),
            /*strict_config*/ false,
            /*has_non_replayable_launch_overrides*/ false,
        ));
        assert!(!can_reuse_implicit_local_daemon(
            &cli_kv_overrides,
            &LoaderOverrides::default(),
            /*strict_config*/ false,
            /*has_non_replayable_launch_overrides*/ false,
        ));
        loader_overrides.ignore_user_config = true;
        assert!(!can_reuse_implicit_local_daemon(
            &[],
            &loader_overrides,
            /*strict_config*/ false,
            /*has_non_replayable_launch_overrides*/ false,
        ));
        assert!(!can_reuse_implicit_local_daemon(
            &[],
            &LoaderOverrides::default(),
            /*strict_config*/ true,
            /*has_non_replayable_launch_overrides*/ false,
        ));
        assert!(!can_reuse_implicit_local_daemon(
            &[],
            &LoaderOverrides::default(),
            /*strict_config*/ false,
            /*has_non_replayable_launch_overrides*/ true,
        ));
        Ok(())
    }

    #[test]
    fn should_load_configured_environments_for_local_daemon() -> color_eyre::Result<()> {
        let target = AppServerTarget::LocalDaemon {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
            },
        };

        assert!(should_load_configured_environments(
            &LoaderOverrides::default(),
            &target,
        ));
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_keep_local_filters_for_embedded_sessions()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let cwd = temp_dir.path().join("project");

        let params = latest_session_lookup_params(
            /*uses_remote_workspace*/ false,
            &config,
            Some(cwd.as_path()),
            /*include_non_interactive*/ false,
            LatestSessionLookupMode::StateDbOnly,
        );

        assert_eq!(
            params.model_providers,
            Some(vec![config.model_provider_id.clone()])
        );
        assert_eq!(
            params.cwd,
            Some(ThreadListCwdFilter::One(cwd.to_string_lossy().to_string()))
        );
        assert!(params.use_state_db_only);

        let scan_params = latest_session_lookup_params(
            /*uses_remote_workspace*/ false,
            &config,
            Some(cwd.as_path()),
            /*include_non_interactive*/ false,
            LatestSessionLookupMode::ScanAndRepair,
        );
        assert!(!scan_params.use_state_db_only);
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_keep_local_filters_for_local_daemon_sessions()
    -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let cwd = temp_dir.path().join("project");
        let target = AppServerTarget::LocalDaemon {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
            },
        };

        let params = latest_session_lookup_params(
            target.uses_remote_workspace(),
            &config,
            Some(cwd.as_path()),
            /*include_non_interactive*/ false,
            LatestSessionLookupMode::StateDbOnly,
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
            /*uses_remote_workspace*/ true,
            &config,
            /*cwd_filter*/ None,
            /*include_non_interactive*/ false,
            LatestSessionLookupMode::StateDbOnly,
        );

        assert_eq!(params.model_providers, None);
        assert_eq!(params.cwd, None);
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_can_include_non_interactive_sources()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;

        let params = latest_session_lookup_params(
            /*uses_remote_workspace*/ true,
            &config,
            /*cwd_filter*/ None,
            /*include_non_interactive*/ true,
            LatestSessionLookupMode::StateDbOnly,
        );

        assert_eq!(
            params.source_kinds,
            Some(vec![
                ThreadSourceKind::Cli,
                ThreadSourceKind::VsCode,
                ThreadSourceKind::Exec,
                ThreadSourceKind::AppServer,
            ])
        );
        Ok(())
    }

    #[tokio::test]
    async fn latest_session_lookup_params_keep_explicit_cwd_filter_for_remote_sessions()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let config = build_config(&temp_dir).await?;
        let cwd = Path::new("repo/on/server");

        let params = latest_session_lookup_params(
            /*uses_remote_workspace*/ true,
            &config,
            Some(cwd),
            /*include_non_interactive*/ false,
            LatestSessionLookupMode::StateDbOnly,
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
            /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ false,
        );
        let show_all_filter = latest_session_cwd_filter(
            /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
            /*show_all*/ true,
        );
        let remote_filter = latest_session_cwd_filter(
            /*uses_remote_workspace*/ true,
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

        let mut app_server = AppServerSession::new(
            codex_app_server_client::AppServerClient::InProcess(
                start_test_embedded_app_server(config.clone()).await?,
            ),
            ThreadParamsMode::Embedded,
        );
        let filter_cwd = latest_session_cwd_filter(
            /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
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
            /*uses_remote_workspace*/ false, /*remote_cwd_override*/ None, &config,
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
    async fn latest_session_lookup_falls_back_for_rollout_missing_from_state_db()
    -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let project_cwd = temp_dir.path().join("project");
        std::fs::create_dir_all(&project_cwd)?;
        let config = ConfigBuilder::default()
            .codex_home(temp_dir.path().to_path_buf())
            .harness_overrides(ConfigOverrides {
                cwd: Some(project_cwd.clone()),
                ..Default::default()
            })
            .build()
            .await?;
        let mut app_server = AppServerSession::new(
            codex_app_server_client::AppServerClient::InProcess(
                start_test_embedded_app_server(config.clone()).await?,
            ),
            ThreadParamsMode::Embedded,
        );

        // Simulate a legacy writer creating a rollout after the state DB backfill completed.
        let thread_id = write_session_rollout(
            temp_dir.path(),
            "2025-01-02T10-00-00",
            "2025-01-02T10:00:00Z",
            "legacy writer session",
            config.model_provider_id.as_str(),
            &project_cwd,
        )?;

        let target = lookup_latest_session_target_with_app_server(
            &mut app_server,
            &config,
            Some(project_cwd.as_path()),
            /*include_non_interactive*/ false,
        )
        .await?
        .expect("expected scan-and-repair fallback to find the rollout");
        app_server.shutdown().await?;

        assert_eq!(target.thread_id, thread_id);
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

        let config_cwd = config_cwd_for_app_server_target(
            Some(remote_only_cwd),
            &target,
            environment_manager
                .default_environment()
                .is_some_and(|environment| environment.is_remote()),
        )?;

        assert_eq!(config_cwd, None);
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_canonicalizes_embedded_cli_cwd() -> std::io::Result<()>
    {
        let temp_dir = TempDir::new()?;
        let target = AppServerTarget::Embedded;
        let environment_manager = EnvironmentManager::default_for_tests();

        let config_cwd = config_cwd_for_app_server_target(
            Some(temp_dir.path()),
            &target,
            environment_manager
                .default_environment()
                .is_some_and(|environment| environment.is_remote()),
        )?;

        assert_eq!(
            config_cwd,
            Some(AbsolutePathBuf::from_absolute_path(dunce::canonicalize(
                temp_dir.path()
            )?)?)
        );
        Ok(())
    }

    #[tokio::test]
    async fn config_cwd_for_app_server_target_canonicalizes_local_daemon_cli_cwd()
    -> std::io::Result<()> {
        let temp_dir = TempDir::new()?;
        let target = AppServerTarget::LocalDaemon {
            endpoint: RemoteAppServerEndpoint::UnixSocket {
                socket_path: AbsolutePathBuf::relative_to_current_dir("codex.sock")?,
            },
        };
        let environment_manager = EnvironmentManager::default_for_tests();

        let config_cwd = config_cwd_for_app_server_target(
            Some(temp_dir.path()),
            &target,
            environment_manager
                .default_environment()
                .is_some_and(|environment| environment.is_remote()),
        )?;

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

        let err = config_cwd_for_app_server_target(
            Some(&missing),
            &target,
            environment_manager
                .default_environment()
                .is_some_and(|environment| environment.is_remote()),
        )
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
            Some(ExecServerRuntimePaths::new(
                std::env::current_exe().expect("current exe"),
                /*codex_linux_sandbox_exe*/ None,
            )?),
        )
        .await;

        let config_cwd = config_cwd_for_app_server_target(
            Some(remote_only_cwd),
            &target,
            environment_manager
                .default_environment()
                .is_some_and(|environment| environment.is_remote()),
        )?;

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
    async fn resume_picker_loads_complete_paginated_and_legacy_transcripts()
    -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        config.terminal_resize_reflow.max_rows =
            crate::legacy_core::config::TerminalResizeReflowMaxRows::Limit(2);
        let mut app_server = AppServerSession::new(
            AppServerClient::InProcess(start_test_embedded_app_server(config.clone()).await?),
            ThreadParamsMode::Embedded,
        );
        let filename_ts = "2025-01-05T12-00-00";
        let rollout_line = |ordinal: usize, payload: serde_json::Value| {
            serde_json::json!({
                "timestamp": "2025-01-05T12:00:00Z",
                "type": "event_msg",
                "payload": payload,
                "ordinal": ordinal,
            })
        };
        for (history_mode, create_rollout) in [
            app_test_support::create_fake_rollout,
            app_test_support::create_fake_paginated_rollout,
        ]
        .into_iter()
        .enumerate()
        {
            let thread_id = create_rollout(
                temp_dir.path(),
                filename_ts,
                "2025-01-05T12:00:00Z",
                "message 0",
                Some(config.model_provider_id.as_str()),
                /*git_info*/ None,
            )
            .expect("create session rollout");
            let path = app_test_support::rollout_path(temp_dir.path(), filename_ts, &thread_id);
            let mut contents = std::fs::read_to_string(&path)?;
            let started = rollout_line(
                /*ordinal*/ 3,
                serde_json::json!({ "type": "task_started", "turn_id": "history-turn", "model_context_window": null }),
            );
            contents.push_str(&format!("{started}\n"));
            for index in 0..=100 {
                let message = format!("message {index}");
                let payload = if history_mode == 1 {
                    serde_json::json!({
                        "type": "item_completed",
                        "thread_id": thread_id,
                        "turn_id": "history-turn",
                        "item": { "type": "UserMessage", "id": format!("user-{index}"),
                            "content": [{ "type": "text", "text": message }] },
                    })
                } else {
                    serde_json::json!({ "type": "user_message", "message": message })
                };
                let item = rollout_line(index + 4, payload);
                contents.push_str(&format!("{item}\n"));
            }
            if history_mode == 1 {
                for index in 0..125 {
                    let item = rollout_line(
                        index + 105,
                        serde_json::json!({
                            "type": "item_completed",
                            "thread_id": thread_id,
                            "turn_id": "history-turn",
                            "item": { "type": "Reasoning", "id": format!("hidden-{index}"),
                                "summary_text": [], "raw_content": [] },
                        }),
                    );
                    contents.push_str(&format!("{item}\n"));
                }
            }
            std::fs::write(path, contents)?;
            let thread_id = ThreadId::from_string(&thread_id)?;
            let started = app_server
                .resume_thread(
                    config.clone(),
                    thread_id,
                    app_server_session::ResumeModelSettings::RestoreFromThread,
                )
                .await?;
            if history_mode == 1 {
                assert!(
                    started
                        .turns
                        .iter()
                        .flat_map(|turn| &turn.items)
                        .any(|item| {
                            matches!(
                                item,
                                codex_app_server_protocol::ThreadItem::UserMessage { .. }
                            )
                        })
                );
                let preview = crate::resume_picker::load_transcript_preview(
                    &mut app_server,
                    thread_id,
                    /*config*/ None,
                )
                .await?;
                assert!(!preview.is_empty());
            }
            let cells = crate::thread_transcript::load_session_transcript(
                &mut app_server,
                thread_id,
                crate::thread_transcript::RawReasoningVisibility::Hidden,
                /*config*/ None,
            )
            .await?;
            assert!(cells.len() > 100);
        }
        app_server.shutdown().await?;
        Ok(())
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
            CloudConfigBundleLoader::default(),
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
        let sqlite =
            codex_state::SqliteConfig::new_for_testing(occupied_sqlite_home.as_path().abs());
        config.sqlite = sqlite.clone();

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
            sqlite.state_db_path().as_path()
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
    async fn embedded_state_db_corruption_preserves_failed_database_for_cli_recovery()
    -> color_eyre::Result<()> {
        let temp_dir = TempDir::new()?;
        let mut config = build_config(&temp_dir).await?;
        let sqlite_home = temp_dir.path().join("sqlite-home");
        std::fs::create_dir_all(&sqlite_home)?;
        let sqlite = codex_state::SqliteConfig::new_for_testing(sqlite_home.as_path().abs());
        let logs_db_path = sqlite.logs_db_path();
        std::fs::write(&logs_db_path, "not a sqlite database")?;
        config.sqlite = sqlite;

        let err =
            match init_state_db_for_app_server_target(&config, &AppServerTarget::Embedded).await {
                Ok(_) => panic!("embedded startup should surface state db init failures"),
                Err(err) => err,
            };
        let startup_error = err
            .get_ref()
            .and_then(|err| err.downcast_ref::<LocalStateDbStartupError>())
            .expect("state db startup failure should retain its typed context");

        assert_eq!(startup_error.database_path(), logs_db_path.as_path());
        assert!(
            codex_state::sqlite_error_detail_is_corruption(startup_error.detail()),
            "startup error should preserve the SQLite corruption cause, got: {}",
            startup_error.detail()
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
