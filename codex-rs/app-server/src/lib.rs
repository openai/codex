#![recursion_limit = "256"]
#![deny(clippy::print_stdout, clippy::print_stderr)]

use codex_arg0::Arg0DispatchPaths;
use codex_config::ConfigLayerStackOrdering;
use codex_config::LoaderOverrides;
use codex_config::NoopThreadConfigLoader;
use codex_config::RemoteThreadConfigLoader;
use codex_config::ThreadConfigLoader;
use codex_core::config::Config;
use codex_core::resolve_installation_id;
use codex_login::AuthManager;
#[cfg(debug_assertions)]
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use std::collections::HashMap;
use std::collections::HashSet;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;

use crate::analytics_utils::analytics_events_client_from_config;
use crate::config_manager::ConfigManager;
use crate::connection_cleanup::ConnectionCleanupTasks;
use crate::message_processor::MessageProcessor;
use crate::message_processor::MessageProcessorArgs;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessageSender;
use crate::outgoing_message::QueuedOutgoingMessage;
use crate::transport::CHANNEL_CAPACITY;
use crate::transport::ConnectionState;
use crate::transport::OutboundConnectionState;
use crate::transport::RemoteControlPolicy;
use crate::transport::RemoteControlStartConfig;
use crate::transport::TransportEvent;
use crate::transport::acquire_app_server_startup_lock;
use crate::transport::app_server_startup_lock_path;
use crate::transport::auth::policy_from_settings;
use crate::transport::prepare_control_socket_path;
use crate::transport::route_outgoing_envelope;
use crate::transport::start_control_socket_acceptor;
use crate::transport::start_remote_control;
use crate::transport::start_stdio_connection;
use crate::transport::start_websocket_acceptor;
use codex_analytics::AppServerRpcTransport;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::TextPosition as AppTextPosition;
use codex_app_server_protocol::TextRange as AppTextRange;
use codex_config::ConfigLayerSource;
use codex_config::ConfigLoadError;
use codex_config::TextRange as CoreTextRange;
use codex_core::ExecPolicyError;
use codex_core::check_execpolicy_for_warnings;
use codex_core::config::find_codex_home;
use codex_exec_server::EnvironmentManager;
use codex_exec_server::ExecServerRuntimePaths;
use codex_feedback::CodexFeedback;
use codex_protocol::protocol::SessionSource;
use codex_rollout::state_db as rollout_state_db;
use codex_state::log_db;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::error;
use tracing::info;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::Registry;
use tracing_subscriber::util::SubscriberInitExt;

const SQLITE_RECOVERY_CONFIG_WARNING_SUMMARY: &str = "Codex rebuilt its local database.";

mod analytics_utils;
mod app_info;
mod app_server_tracing;
mod attestation;
mod auth_mode;
mod bespoke_event_handling;
mod command_exec;
mod config_layer;
mod config_manager;
mod config_manager_service;
mod connection_cleanup;
mod connection_rpc_gate;
mod current_time;
mod dynamic_tools;
mod effective_plugin_change;
mod error_code;
mod extensions;
mod external_agent_migration;
mod external_auth;
mod filters;
mod fs_watch;
mod fuzzy_file_search;
mod image_url;
pub mod in_process;
mod mcp_refresh;
mod message_processor;
mod models;
mod models_refresh_worker;
mod outgoing_message;
mod request_processors;
mod request_serialization;
mod server_request_error;
mod skills_watcher;
mod thread_state;
mod thread_status;
mod transport;

pub use crate::error_code::INPUT_TOO_LARGE_ERROR_CODE;
pub use crate::error_code::INVALID_PARAMS_ERROR_CODE;
pub use crate::transport::AppServerTransport;
pub use crate::transport::RemoteControlStartupMode;
pub use crate::transport::app_server_control_socket_path;
pub use crate::transport::auth::AppServerWebsocketAuthArgs;
pub use crate::transport::auth::AppServerWebsocketAuthSettings;
pub use crate::transport::auth::WebsocketAuthCliMode;
pub use crate::transport::take_remote_control_disabled_env;

const LOG_FORMAT_ENV_VAR: &str = "LOG_FORMAT";
const OTEL_SERVICE_NAME: &str = "codex-app-server";
#[cfg(debug_assertions)]
const TEST_USER_CONFIG_FILE_ENV_VAR: &str = "CODEX_APP_SERVER_TEST_USER_CONFIG_FILE";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum LogFormat {
    Default,
    Json,
}

type StderrLogLayer = Box<dyn Layer<Registry> + Send + Sync + 'static>;

fn configured_thread_config_loader(config: &Config) -> Arc<dyn ThreadConfigLoader> {
    match config.experimental_thread_config_endpoint.as_deref() {
        Some(endpoint) => Arc::new(RemoteThreadConfigLoader::new(endpoint)),
        None => Arc::new(NoopThreadConfigLoader),
    }
}

/// Control-plane messages from the processor/transport side to the outbound router task.
///
/// `run_main_with_transport_options` uses two loops/tasks:
/// - processor loop: handles incoming JSON-RPC and request dispatch
/// - outbound loop: performs potentially slow writes to per-connection writers
///
/// `OutboundControlEvent` keeps those loops coordinated without sharing mutable
/// connection state directly. In particular, the outbound loop needs to know
/// when a connection opens/closes so it can route messages correctly.
enum OutboundControlEvent {
    /// Register a new writer for an opened connection.
    Opened {
        connection_id: ConnectionId,
        writer: mpsc::Sender<QueuedOutgoingMessage>,
        disconnect_sender: Option<CancellationToken>,
        initialized: Arc<AtomicBool>,
        experimental_api_enabled: Arc<AtomicBool>,
        opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    },
    /// Remove state for a closed/disconnected connection.
    Closed { connection_id: ConnectionId },
    /// Disconnect all connection-oriented clients during graceful restart.
    DisconnectAll,
}

#[derive(Default)]
struct ShutdownState {
    requested: bool,
    forced: bool,
    last_logged_running_turn_count: Option<usize>,
}

enum ShutdownAction {
    Noop,
    Finish,
}

#[derive(Clone, Copy)]
enum ShutdownSignal {
    Forceable,
    #[cfg(unix)]
    GracefulOnly,
}

async fn shutdown_signal() -> IoResult<ShutdownSignal> {
    #[cfg(unix)]
    {
        use tokio::signal::unix::SignalKind;
        use tokio::signal::unix::signal;

        let mut term = signal(SignalKind::terminate())?;
        let mut hangup = signal(SignalKind::hangup())?;
        tokio::select! {
            ctrl_c_result = tokio::signal::ctrl_c() => ctrl_c_result.map(|_| ShutdownSignal::Forceable),
            _ = term.recv() => Ok(ShutdownSignal::Forceable),
            _ = hangup.recv() => Ok(ShutdownSignal::GracefulOnly),
        }
    }

    #[cfg(not(unix))]
    {
        tokio::signal::ctrl_c()
            .await
            .map(|_| ShutdownSignal::Forceable)
    }
}

impl ShutdownState {
    fn requested(&self) -> bool {
        self.requested
    }

    fn forced(&self) -> bool {
        self.forced
    }

    fn on_signal(
        &mut self,
        signal: ShutdownSignal,
        connection_count: usize,
        running_turn_count: usize,
    ) {
        if self.requested {
            if matches!(signal, ShutdownSignal::Forceable) {
                self.forced = true;
            }
            return;
        }

        self.requested = true;
        self.last_logged_running_turn_count = None;
        info!(
            "received shutdown signal; entering graceful restart drain (connections={}, runningAssistantTurns={}, requests still accepted until no assistant turns are running)",
            connection_count, running_turn_count,
        );
    }

    fn update(&mut self, running_turn_count: usize, connection_count: usize) -> ShutdownAction {
        if !self.requested {
            return ShutdownAction::Noop;
        }

        if self.forced || running_turn_count == 0 {
            if self.forced {
                info!(
                    "received second shutdown signal; forcing restart with {running_turn_count} running assistant turn(s) and {connection_count} connection(s)"
                );
            } else {
                info!(
                    "shutdown signal restart: no assistant turns running; stopping acceptor and disconnecting {connection_count} connection(s)"
                );
            }
            return ShutdownAction::Finish;
        }

        if self.last_logged_running_turn_count != Some(running_turn_count) {
            info!(
                "shutdown signal restart: waiting for {running_turn_count} running assistant turn(s) to finish"
            );
            self.last_logged_running_turn_count = Some(running_turn_count);
        }

        ShutdownAction::Noop
    }
}

fn config_warning_from_error(
    summary: impl Into<String>,
    err: &std::io::Error,
) -> ConfigWarningNotification {
    let (path, range) = match config_error_location(err) {
        Some((path, range)) => (Some(path), Some(range)),
        None => (None, None),
    };
    ConfigWarningNotification {
        summary: summary.into(),
        details: Some(err.to_string()),
        path,
        range,
    }
}

fn config_error_location(err: &std::io::Error) -> Option<(String, AppTextRange)> {
    err.get_ref()
        .and_then(|err| err.downcast_ref::<ConfigLoadError>())
        .map(|err| {
            let config_error = err.config_error();
            (
                config_error.path.to_string_lossy().to_string(),
                app_text_range(&config_error.range),
            )
        })
}

fn exec_policy_warning_location(err: &ExecPolicyError) -> (Option<String>, Option<AppTextRange>) {
    match err {
        ExecPolicyError::ParsePolicy { path, source } => {
            if let Some(location) = source.location() {
                let range = AppTextRange {
                    start: AppTextPosition {
                        line: location.range.start.line,
                        column: location.range.start.column,
                    },
                    end: AppTextPosition {
                        line: location.range.end.line,
                        column: location.range.end.column,
                    },
                };
                return (Some(location.path), Some(range));
            }
            (Some(path.clone()), None)
        }
        _ => (None, None),
    }
}

fn exec_policy_config_warning(err: &ExecPolicyError) -> ConfigWarningNotification {
    let (path, range) = exec_policy_warning_location(err);
    ConfigWarningNotification {
        summary: "Error parsing rules; custom rules not applied.".to_string(),
        details: Some(err.to_string()),
        path,
        range,
    }
}

fn app_text_range(range: &CoreTextRange) -> AppTextRange {
    AppTextRange {
        start: AppTextPosition {
            line: range.start.line,
            column: range.start.column,
        },
        end: AppTextPosition {
            line: range.end.line,
            column: range.end.column,
        },
    }
}

fn project_config_warning(config: &Config) -> Option<ConfigWarningNotification> {
    let mut disabled_folders = Vec::new();

    for layer in config.config_layer_stack.get_layers(
        ConfigLayerStackOrdering::LowestPrecedenceFirst,
        /*include_disabled*/ true,
    ) {
        let ConfigLayerSource::Project { dot_codex_folder } = &layer.name else {
            continue;
        };
        let Some(disabled_reason) = &layer.disabled_reason else {
            continue;
        };
        disabled_folders.push((
            dot_codex_folder.as_path().display().to_string(),
            disabled_reason.clone(),
        ));
    }

    if disabled_folders.is_empty() {
        return None;
    }

    let mut message = concat!(
        "Project-local config, hooks, and exec policies are disabled in the following folders ",
        "until the project is trusted, but skills still load.\n",
    )
    .to_string();
    for (index, (folder, reason)) in disabled_folders.iter().enumerate() {
        let display_index = index + 1;
        message.push_str(&format!("    {display_index}. {folder}\n"));
        message.push_str(&format!("       {reason}\n"));
    }

    Some(ConfigWarningNotification {
        summary: message,
        details: None,
        path: None,
        range: None,
    })
}

impl LogFormat {
    fn from_env_value(value: Option<&str>) -> Self {
        match value.map(str::trim).map(str::to_ascii_lowercase) {
            Some(value) if value == "json" => Self::Json,
            _ => Self::Default,
        }
    }
}

fn log_format_from_env() -> LogFormat {
    let value = std::env::var(LOG_FORMAT_ENV_VAR).ok();
    LogFormat::from_env_value(value.as_deref())
}

pub async fn run_main(
    arg0_paths: Arg0DispatchPaths,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    default_analytics_enabled: bool,
) -> IoResult<()> {
    run_main_with_transport_options(
        arg0_paths,
        cli_config_overrides,
        loader_overrides,
        strict_config,
        default_analytics_enabled,
        AppServerTransport::Stdio,
        SessionSource::VSCode,
        AppServerWebsocketAuthSettings::default(),
        AppServerRuntimeOptions::default(),
    )
    .await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PluginStartupTasks {
    Start,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppServerRuntimeOptions {
    pub plugin_startup_tasks: PluginStartupTasks,
    pub remote_control_startup_mode: RemoteControlStartupMode,
    pub install_shutdown_signal_handler: bool,
}

impl Default for AppServerRuntimeOptions {
    fn default() -> Self {
        Self {
            plugin_startup_tasks: PluginStartupTasks::Start,
            remote_control_startup_mode: RemoteControlStartupMode::ResolvePersisted,
            install_shutdown_signal_handler: true,
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub async fn run_main_with_transport_options(
    arg0_paths: Arg0DispatchPaths,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    strict_config: bool,
    default_analytics_enabled: bool,
    transport: AppServerTransport,
    session_source: SessionSource,
    auth: AppServerWebsocketAuthSettings,
    runtime_options: AppServerRuntimeOptions,
) -> IoResult<()> {
    let loader_overrides = loader_overrides_with_test_user_config_file(
        loader_overrides,
        test_user_config_file_from_env(),
    )?;
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(CHANNEL_CAPACITY);
    let (outbound_control_tx, mut outbound_control_rx) =
        mpsc::channel::<OutboundControlEvent>(CHANNEL_CAPACITY);

    // Parse CLI overrides once and derive the base Config eagerly so later
    // components do not need to work with raw TOML values.
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;
    let codex_home = find_codex_home()?;
    let local_runtime_paths = ExecServerRuntimePaths::from_optional_paths(
        arg0_paths.codex_self_exe.clone(),
        arg0_paths.codex_linux_sandbox_exe.clone(),
    )?;
    let environment_manager = if loader_overrides.ignore_user_config {
        EnvironmentManager::from_env(Some(local_runtime_paths)).await
    } else {
        EnvironmentManager::from_codex_home(codex_home.clone(), Some(local_runtime_paths)).await
    }
    .map(Arc::new)
    .map_err(std::io::Error::other)?;
    let config_manager = ConfigManager::new(
        codex_home.to_path_buf(),
        cli_kv_overrides.clone(),
        loader_overrides,
        strict_config,
        Default::default(),
        arg0_paths.clone(),
        Arc::new(NoopThreadConfigLoader),
    );
    match config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await
    {
        Ok(config) => {
            let discovered_thread_config_loader = configured_thread_config_loader(&config);
            config_manager
                .replace_thread_config_loader(Arc::clone(&discovered_thread_config_loader));
            let auth_manager =
                AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;
            config_manager.replace_cloud_config_bundle_loader(
                auth_manager,
                config.chatgpt_base_url.clone(),
                config.http_client_factory(),
            );
        }
        Err(err) => {
            warn!(error = %err, "Failed to preload config for cloud config bundle");
            // TODO: Decide whether bootstrap config preload failures should block startup.
            // If this fails, we cannot install cloud/thread config loaders, so non-strict
            // startup may continue without managed cloud config.
        }
    };
    let mut config_warnings = Vec::new();
    let config = match config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await
    {
        Ok(config) => config,
        Err(err) => {
            if strict_config {
                return Err(err);
            }

            let message = config_warning_from_error("Invalid configuration; using defaults.", &err);
            config_warnings.push(message);
            config_manager.load_default_config().await.map_err(|e| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("error loading default config after config error: {e}"),
                )
            })?
        }
    };

    let otel = codex_core::otel_init::build_provider(
        &config,
        env!("CARGO_PKG_VERSION"),
        Some(OTEL_SERVICE_NAME),
        default_analytics_enabled,
    )
    .map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("error loading otel config: {e}"),
        )
    })?;
    codex_core::otel_init::record_process_start(otel.as_ref(), OTEL_SERVICE_NAME);
    codex_core::otel_init::install_sqlite_telemetry(otel.as_ref(), OTEL_SERVICE_NAME);
    let unix_socket_startup_lock = match &transport {
        AppServerTransport::UnixSocket { socket_path } => {
            let startup_lock_path = app_server_startup_lock_path(&codex_home)?;
            let startup_lock = acquire_app_server_startup_lock(startup_lock_path).await?;
            prepare_control_socket_path(socket_path.as_path()).await?;
            Some(startup_lock)
        }
        _ => None,
    };
    let state_db_init = match init_sqlite_state_db_with_fresh_start_on_corruption(&config).await {
        Ok(state_db_init) => state_db_init,
        Err(err) => {
            return Err(std::io::Error::other(format!(
                "failed to initialize sqlite state runtime under {}: {err}",
                config.sqlite_home.display()
            )));
        }
    };
    let state_db = state_db_init.state_db;
    if let Some(recovery_notice) = state_db_init.recovery_notice {
        config_warnings.push(ConfigWarningNotification {
            summary: SQLITE_RECOVERY_CONFIG_WARNING_SUMMARY.to_string(),
            details: Some(recovery_notice.details),
            path: None,
            range: None,
        });
    }

    if let Ok(Some(err)) = check_execpolicy_for_warnings(&config.config_layer_stack).await {
        config_warnings.push(exec_policy_config_warning(&err));
    }

    if let Some(warning) = project_config_warning(&config) {
        config_warnings.push(warning);
    }
    for warning in &config.startup_warnings {
        config_warnings.push(ConfigWarningNotification {
            summary: warning.clone(),
            details: None,
            path: None,
            range: None,
        });
    }
    if let Some(warning) =
        codex_core::config::system_bwrap_warning(config.permissions.permission_profile())
    {
        config_warnings.push(ConfigWarningNotification {
            summary: warning,
            details: None,
            path: None,
            range: None,
        });
    }

    let feedback = CodexFeedback::new();

    // Install a simple subscriber so `tracing` output is visible. Users can
    // control the log level with `RUST_LOG` and switch to JSON logs with
    // `LOG_FORMAT=json`.
    let stderr_fmt: StderrLogLayer = match log_format_from_env() {
        LogFormat::Json => tracing_subscriber::fmt::layer()
            .json()
            .with_writer(std::io::stderr)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_filter(EnvFilter::from_default_env())
            .boxed(),
        LogFormat::Default => tracing_subscriber::fmt::layer()
            .with_writer(std::io::stderr)
            .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
            .with_filter(EnvFilter::from_default_env())
            .boxed(),
    };

    let feedback_layer = feedback.logger_layer();
    let feedback_metadata_layer = feedback.metadata_layer();
    let log_db = state_db.clone().map(log_db::start);
    let log_db_layer = log_db
        .clone()
        .map(|layer| layer.with_filter(log_db::default_filter()));
    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());
    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());
    let _ = tracing_subscriber::registry()
        .with(stderr_fmt)
        .with(feedback_layer)
        .with(feedback_metadata_layer)
        .with(log_db_layer)
        .with(otel_logger_layer)
        .with(otel_tracing_layer)
        .try_init();
    for warning in &config_warnings {
        match &warning.details {
            Some(details) => error!("{} {}", warning.summary, details),
            None => error!("{}", warning.summary),
        }
    }
    let remote_control_policy = if config
        .config_layer_stack
        .requirements()
        .allow_remote_control
        .as_ref()
        .is_some_and(|requirement| !requirement.value)
    {
        RemoteControlPolicy::DisabledByRequirements
    } else {
        RemoteControlPolicy::Allowed
    };
    let remote_control_startup_mode = runtime_options.remote_control_startup_mode;
    let remote_control_explicitly_requested =
        remote_control_startup_mode == RemoteControlStartupMode::EnabledEphemeral;
    if remote_control_explicitly_requested
        && remote_control_policy == RemoteControlPolicy::DisabledByRequirements
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            "remote control is disabled by managed requirements",
        ));
    }
    let installation_id = resolve_installation_id(&config.codex_home).await?;
    let transport_shutdown_token = CancellationToken::new();
    let mut transport_accept_handles = Vec::<JoinHandle<()>>::new();

    let single_client_mode = matches!(&transport, AppServerTransport::Stdio);
    let shutdown_when_no_connections = single_client_mode;
    let graceful_signal_restart_enabled =
        runtime_options.install_shutdown_signal_handler && !single_client_mode;
    let mut app_server_client_name_rx = None;

    match &transport {
        AppServerTransport::Stdio => {
            let (stdio_client_name_tx, stdio_client_name_rx) = oneshot::channel::<String>();
            app_server_client_name_rx = Some(stdio_client_name_rx);
            start_stdio_connection(
                transport_event_tx.clone(),
                &mut transport_accept_handles,
                stdio_client_name_tx,
            )
            .await?;
        }
        AppServerTransport::UnixSocket { socket_path } => {
            let accept_handle = start_control_socket_acceptor(
                socket_path.clone(),
                transport_event_tx.clone(),
                transport_shutdown_token.clone(),
            )
            .await?;
            transport_accept_handles.push(accept_handle);
        }
        AppServerTransport::WebSocket { bind_address } => {
            let accept_handle = start_websocket_acceptor(
                *bind_address,
                transport_event_tx.clone(),
                transport_shutdown_token.clone(),
                policy_from_settings(&auth)?,
            )
            .await?;
            transport_accept_handles.push(accept_handle);
        }
        AppServerTransport::Off => {}
    }
    drop(unix_socket_startup_lock);

    let auth_manager =
        AuthManager::shared_from_config(&config, /*enable_codex_api_key_env*/ false).await;

    let remote_control_enabled = remote_control_policy == RemoteControlPolicy::Allowed
        && remote_control_explicitly_requested
        && state_db.is_some();
    if remote_control_explicitly_requested && state_db.is_none() {
        error!("remote control disabled because sqlite state db is unavailable");
    }
    let no_local_transport = transport_accept_handles.is_empty();
    if no_local_transport
        && remote_control_startup_mode != RemoteControlStartupMode::ResolvePersisted
        && !remote_control_enabled
    {
        return Err(std::io::Error::new(
            ErrorKind::InvalidInput,
            if remote_control_policy == RemoteControlPolicy::DisabledByRequirements {
                "no transport configured; remote control disabled by managed requirements"
            } else if remote_control_explicitly_requested && state_db.is_none() {
                "no transport configured; remote control disabled because sqlite state db is unavailable"
            } else {
                "no transport configured; use --listen or enable remote control"
            },
        ));
    }

    let (remote_control_accept_handle, remote_control_handle) = start_remote_control(
        RemoteControlStartConfig {
            remote_control_url: config.chatgpt_base_url.clone(),
            installation_id: installation_id.clone(),
            policy: remote_control_policy,
        },
        state_db.clone(),
        auth_manager.clone(),
        transport_event_tx.clone(),
        transport_shutdown_token.clone(),
        app_server_client_name_rx,
        remote_control_startup_mode,
    )
    .await?;
    if no_local_transport
        && remote_control_startup_mode == RemoteControlStartupMode::ResolvePersisted
    {
        let persisted_enabled = match remote_control_handle
            .resolve_persisted_preference(/*app_server_client_name*/ None)
            .await
        {
            Ok(persisted_enabled) => persisted_enabled,
            Err(err) => {
                warn!("failed to resolve persisted remote control preference: {err}");
                false
            }
        };
        if !persisted_enabled {
            transport_shutdown_token.cancel();
            let _ = remote_control_accept_handle.await;
            return Err(std::io::Error::new(
                ErrorKind::InvalidInput,
                if remote_control_policy == RemoteControlPolicy::DisabledByRequirements {
                    "no transport configured; remote control disabled by managed requirements"
                } else {
                    "no transport configured; use --listen or enable remote control"
                },
            ));
        }
    }
    transport_accept_handles.push(remote_control_accept_handle);

    let outbound_handle = tokio::spawn(async move {
        let mut outbound_connections = HashMap::<ConnectionId, OutboundConnectionState>::new();
        loop {
            tokio::select! {
                    biased;
                    event = outbound_control_rx.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        match event {
                            OutboundControlEvent::Opened {
                                connection_id,
                                writer,
                                disconnect_sender,
                                initialized,
                                experimental_api_enabled,
                                opted_out_notification_methods,
                            } => {
                                outbound_connections.insert(
                                    connection_id,
                                    OutboundConnectionState::new(
                                        writer,
                                        initialized,
                                        experimental_api_enabled,
                                        opted_out_notification_methods,
                                        disconnect_sender,
                                    ),
                                );
                            }
                            OutboundControlEvent::Closed { connection_id } => {
                                outbound_connections.remove(&connection_id);
                            }
                            OutboundControlEvent::DisconnectAll => {
                                info!(
                                    "disconnecting {} outbound websocket connection(s) for graceful restart",
                                    outbound_connections.len()
                                );
                                for connection_state in outbound_connections.values() {
                                    connection_state.request_disconnect();
                                }
                                outbound_connections.clear();
                            }
                        }
                    }
                    envelope = outgoing_rx.recv() => {
                    let Some(envelope) = envelope else {
                        break;
                    };
                    route_outgoing_envelope(&mut outbound_connections, envelope).await;
                }
            }
        }
        info!("outbound router task exited (channel closed)");
    });

    let processor_handle = tokio::spawn({
        let auth_manager = Arc::clone(&auth_manager);
        let analytics_events_client =
            analytics_events_client_from_config(Arc::clone(&auth_manager), &config);
        let outgoing_message_sender = Arc::new(OutgoingMessageSender::new(
            outgoing_tx,
            analytics_events_client.clone(),
        ));
        let initialize_notification_sender = outgoing_message_sender.clone();
        let outbound_control_tx = outbound_control_tx;
        let processor = Arc::new(MessageProcessor::new(MessageProcessorArgs {
            outgoing: outgoing_message_sender,
            analytics_events_client,
            arg0_paths,
            config: Arc::new(config),
            config_manager,
            environment_manager,
            feedback: feedback.clone(),
            log_db,
            state_db: state_db.clone(),
            config_warnings,
            session_source,
            auth_manager,
            installation_id,
            rpc_transport: analytics_rpc_transport(&transport),
            remote_control_handle: Some(remote_control_handle.clone()),
            plugin_startup_tasks: runtime_options.plugin_startup_tasks,
        }));
        let mut thread_created_rx = processor.thread_created_receiver();
        let mut running_turn_count_rx = processor.subscribe_running_assistant_turn_count();
        let mut connections = HashMap::<ConnectionId, ConnectionState>::new();
        let mut connection_cleanup_tasks = ConnectionCleanupTasks::new();
        let mut remote_control_status_rx = remote_control_handle.status_receiver();
        let mut remote_control_status = remote_control_status_rx.borrow().clone();
        let transport_shutdown_token = transport_shutdown_token.clone();
        async move {
            let mut listen_for_threads = true;
            let mut shutdown_state = ShutdownState::default();
            let exit_reason = loop {
                let running_turn_count = {
                    let running_turn_count = running_turn_count_rx.borrow();
                    *running_turn_count
                };
                if matches!(
                    shutdown_state.update(running_turn_count, connections.len()),
                    ShutdownAction::Finish
                ) {
                    transport_shutdown_token.cancel();
                    let _ = outbound_control_tx
                        .send(OutboundControlEvent::DisconnectAll)
                        .await;
                    break "shutdown_requested";
                }

                tokio::select! {
                    shutdown_signal_result = shutdown_signal(), if graceful_signal_restart_enabled && !shutdown_state.forced() => {
                        let signal = match shutdown_signal_result {
                            Ok(signal) => signal,
                            Err(err) => {
                                warn!("failed to listen for shutdown signal during graceful restart drain: {err}");
                                continue;
                            }
                        };
                        let running_turn_count = *running_turn_count_rx.borrow();
                        shutdown_state.on_signal(signal, connections.len(), running_turn_count);
                    }
                    changed = running_turn_count_rx.changed(), if graceful_signal_restart_enabled && shutdown_state.requested() => {
                        if changed.is_err() {
                            warn!("running-turn watcher closed during graceful restart drain");
                        }
                    }
                    event = transport_event_rx.recv() => {
                        let Some(event) = event else {
                            break "transport_channel_closed";
                        };
                        match event {
                            TransportEvent::ConnectionOpened {
                                connection_id,
                                origin,
                                writer,
                                disconnect_sender,
                            } => {
                                let outbound_initialized = Arc::new(AtomicBool::new(false));
                                let outbound_experimental_api_enabled =
                                    Arc::new(AtomicBool::new(false));
                                let outbound_opted_out_notification_methods =
                                    Arc::new(RwLock::new(HashSet::new()));
                                if outbound_control_tx
                                    .send(OutboundControlEvent::Opened {
                                        connection_id,
                                        writer,
                                        disconnect_sender,
                                        initialized: Arc::clone(&outbound_initialized),
                                        experimental_api_enabled: Arc::clone(
                                            &outbound_experimental_api_enabled,
                                        ),
                                        opted_out_notification_methods: Arc::clone(
                                            &outbound_opted_out_notification_methods,
                                        ),
                                    })
                                    .await
                                    .is_err()
                                {
                                    break "outbound_router_closed";
                                }
                                connections.insert(
                                    connection_id,
                                    ConnectionState::new(
                                        origin,
                                        outbound_initialized,
                                        outbound_experimental_api_enabled,
                                        outbound_opted_out_notification_methods,
                                    ),
                                );
                            }
                            TransportEvent::ConnectionClosed { connection_id } => {
                                let Some(connection_state) = connections.remove(&connection_id) else {
                                    continue;
                                };
                                connection_state.session.rpc_gate.close().await;
                                let outbound_closed = outbound_control_tx
                                    .send(OutboundControlEvent::Closed { connection_id })
                                    .await
                                    .is_ok();
                                let processor = Arc::clone(&processor);
                                connection_cleanup_tasks.spawn(async move {
                                    processor
                                        .connection_closed(connection_id, &connection_state.session)
                                        .await;
                                });
                                if !outbound_closed {
                                    break "outbound_router_closed";
                                }
                                if shutdown_when_no_connections && connections.is_empty() {
                                    break "last_connection_closed";
                                }
                            }
                            TransportEvent::IncomingMessage { connection_id, message } => {
                                match message {
                                    JSONRPCMessage::Request(request) => {
                                        let Some(connection_state) = connections.get_mut(&connection_id) else {
                                            warn!("dropping request from unknown connection: {connection_id:?}");
                                            continue;
                                        };
                                        let was_initialized =
                                            connection_state.session.initialized();
                                        processor
                                            .process_request(
                                                connection_id,
                                                request,
                                                &transport,
                                                Arc::clone(&connection_state.session),
                                            )
                                            .await;
                                        let opted_out_notification_methods_snapshot = connection_state
                                            .session
                                            .opted_out_notification_methods();
                                        let experimental_api_enabled =
                                            connection_state.session.experimental_api_enabled();
                                        let is_initialized = connection_state.session.initialized();
                                        if let Ok(mut opted_out_notification_methods) = connection_state
                                            .outbound_opted_out_notification_methods
                                            .write()
                                        {
                                            *opted_out_notification_methods =
                                                opted_out_notification_methods_snapshot;
                                        } else {
                                            warn!(
                                                "failed to update outbound opted-out notifications"
                                            );
                                        }
                                        connection_state
                                            .outbound_experimental_api_enabled
                                            .store(
                                                experimental_api_enabled,
                                                std::sync::atomic::Ordering::Release,
                                            );
                                        if !was_initialized && is_initialized {
                                            processor
                                                .send_initialize_notifications_to_connection(
                                                    connection_id,
                                                )
                                                .await;
                                            initialize_notification_sender
                                                .send_server_notification_to_connections(
                                                    &[connection_id],
                                                    ServerNotification::RemoteControlStatusChanged(
                                                        remote_control_status.clone(),
                                                    ),
                                                )
                                                .await;
                                            processor
                                                .connection_initialized(
                                                    connection_id,
                                                    connection_state
                                                        .session
                                                        .request_attestation(),
                                                )
                                                .await;
                                            connection_state
                                                .outbound_initialized
                                                .store(true, std::sync::atomic::Ordering::Release);
                                        }
                                    }
                                    JSONRPCMessage::Response(response) => {
                                        if !connections.contains_key(&connection_id) {
                                            warn!("dropping response from unknown connection: {connection_id:?}");
                                            continue;
                                        }
                                        processor.process_response(response).await;
                                    }
                                    JSONRPCMessage::Notification(notification) => {
                                        if !connections.contains_key(&connection_id) {
                                            warn!("dropping notification from unknown connection: {connection_id:?}");
                                            continue;
                                        }
                                        processor.process_notification(notification).await;
                                    }
                                    JSONRPCMessage::Error(err) => {
                                        if !connections.contains_key(&connection_id) {
                                            warn!("dropping error from unknown connection: {connection_id:?}");
                                            continue;
                                        }
                                        processor.process_error(err).await;
                                    }
                                }
                            }
                        }
                    }
                    _ = connection_cleanup_tasks.reap_next() => {}
                    changed = remote_control_status_rx.changed() => {
                        if changed.is_err() {
                            continue;
                        }
                        let status = remote_control_status_rx.borrow().clone();
                        if remote_control_status == status {
                            continue;
                        }
                        remote_control_status = status.clone();
                        let notification = ServerNotification::RemoteControlStatusChanged(status);
                        initialize_notification_sender
                            .send_server_notification(notification)
                            .await;
                    }
                    created = thread_created_rx.recv(), if listen_for_threads => {
                        match created {
                            Ok(thread_id) => {
                                let mut initialized_connection_ids = Vec::new();
                                for (connection_id, connection_state) in &connections {
                                    if connection_state.session.initialized() {
                                        initialized_connection_ids.push(*connection_id);
                                    }
                                }
                                processor
                                    .try_attach_thread_listener(
                                        thread_id,
                                        initialized_connection_ids,
                                    )
                                    .await;
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                                // TODO(jif) handle lag.
                                // Assumes thread creation volume is low enough that lag never happens.
                                // If it does, we log and continue without resyncing to avoid attaching
                                // listeners for threads that should remain unsubscribed.
                                warn!("thread_created receiver lagged; skipping resync");
                            }
                            Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                                listen_for_threads = false;
                            }
                        }
                    }
                }
            };

            if !shutdown_state.forced() {
                futures::future::join_all(
                    connections
                        .values()
                        .map(|connection_state| connection_state.session.rpc_gate.shutdown()),
                )
                .await;
                connection_cleanup_tasks.drain().await;
                processor.drain_background_tasks().await;
                processor.shutdown_threads().await;
            } else {
                connection_cleanup_tasks.abort();
            }
            info!(
                exit_reason,
                remaining_connection_count = connections.len(),
                shutdown_forced = shutdown_state.forced(),
                "processor task exited"
            );
        }
    });

    drop(transport_event_tx);

    let _ = processor_handle.await;
    let _ = outbound_handle.await;

    transport_shutdown_token.cancel();
    for handle in transport_accept_handles {
        let _ = handle.await;
    }

    if let Some(otel) = otel {
        otel.shutdown();
    }

    Ok(())
}

struct SqliteRecoveryNotice {
    details: String,
}

struct RecoveredSqliteDatabase {
    database_path: String,
    backup_folder: String,
}

struct StateDbInitResult {
    state_db: Option<rollout_state_db::StateDbHandle>,
    recovery_notice: Option<SqliteRecoveryNotice>,
}

async fn init_sqlite_state_db_with_fresh_start_on_corruption(
    config: &Config,
) -> anyhow::Result<StateDbInitResult> {
    let mut attempted_backups = HashSet::new();
    let mut recovered_databases = Vec::new();
    loop {
        let err = match rollout_state_db::try_init(config).await {
            Ok(state_db) => {
                let recovery_notice = sqlite_recovery_notice(&recovered_databases);
                if recovery_notice.is_some() {
                    emit_state_db_backup_warning(SQLITE_RECOVERY_CONFIG_WARNING_SUMMARY);
                    for recovered_database in &recovered_databases {
                        emit_state_db_backup_warning(&format!(
                            "Database path: {}",
                            recovered_database.database_path
                        ));
                        emit_state_db_backup_warning(&format!(
                            "Backup folder: {}",
                            recovered_database.backup_folder
                        ));
                    }
                }
                return Ok(StateDbInitResult {
                    state_db: Some(state_db),
                    recovery_notice,
                });
            }
            Err(err) => err,
        };
        let database_path = codex_state::runtime_db_path_for_corruption_error(&err)
            .unwrap_or_else(|| codex_state::state_db_path(config.sqlite_home.as_path()));
        if !codex_state::is_sqlite_corruption_error(&err)
            && !sqlite_home_is_blocking_file(database_path.as_path())
        {
            return Err(err);
        }

        if !attempted_backups.insert(database_path.clone()) {
            return Err(anyhow::anyhow!(
                "failed to initialize sqlite state runtime after moving damaged database file into a backup folder: {err}"
            ));
        }

        let original_error = err.to_string();
        emit_state_db_backup_warning(&format!(
            "Codex local database at {} appears damaged. Moving it into a backup folder so the app server can rebuild it from saved data.",
            database_path.display()
        ));
        let backups = codex_state::backup_runtime_db_for_fresh_start(database_path.as_path())
            .await
            .map_err(|backup_err| {
                anyhow::anyhow!(
                    "failed to move damaged sqlite state database files into a backup folder: {backup_err}; original error: {original_error}"
                )
            })?;
        for backup in &backups {
            emit_state_db_backup_warning(&format!(
                "Moved damaged Codex local database file {} to {}",
                backup.original_path.display(),
                backup.backup_path.display()
            ));
        }
        if let Some(first_backup) = backups.first()
            && let Some(backup_folder) = first_backup.backup_path.parent()
        {
            recovered_databases.push(RecoveredSqliteDatabase {
                database_path: first_backup.original_path.display().to_string(),
                backup_folder: backup_folder.display().to_string(),
            });
        }
    }
}

fn sqlite_home_is_blocking_file(database_path: &Path) -> bool {
    database_path
        .parent()
        .and_then(|path| std::fs::metadata(path).ok())
        .is_some_and(|metadata| metadata.is_file())
}

fn sqlite_recovery_notice(
    recovered_databases: &[RecoveredSqliteDatabase],
) -> Option<SqliteRecoveryNotice> {
    if recovered_databases.is_empty() {
        return None;
    }

    let details = recovered_databases
        .iter()
        .map(|recovered_database| {
            format!(
                "Database path: {}\nBackup folder: {}",
                recovered_database.database_path, recovered_database.backup_folder
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    Some(SqliteRecoveryNotice { details })
}

fn emit_state_db_backup_warning(message: &str) {
    warn!("{message}");
    if !tracing::dispatcher::has_been_set() {
        #[allow(clippy::print_stderr)]
        {
            eprintln!("{message}");
        }
    }
}

fn test_user_config_file_from_env() -> Option<std::path::PathBuf> {
    #[cfg(debug_assertions)]
    {
        std::env::var_os(TEST_USER_CONFIG_FILE_ENV_VAR)
            .filter(|value| !value.is_empty())
            .map(std::path::PathBuf::from)
    }

    #[cfg(not(debug_assertions))]
    None
}

fn loader_overrides_with_test_user_config_file(
    mut loader_overrides: LoaderOverrides,
    test_user_config_file: Option<std::path::PathBuf>,
) -> IoResult<LoaderOverrides> {
    #[cfg(debug_assertions)]
    if let Some(path) = test_user_config_file {
        let path = AbsolutePathBuf::from_absolute_path(path).map_err(|err| {
            std::io::Error::new(
                ErrorKind::InvalidInput,
                format!("invalid test user config path: {err}"),
            )
        })?;
        warn!(
            path = %path.as_path().display(),
            "using debug-only app-server test user config file"
        );
        loader_overrides.user_config_path = Some(path);
    }

    #[cfg(not(debug_assertions))]
    let _ = test_user_config_file;

    Ok(loader_overrides)
}

fn analytics_rpc_transport(transport: &AppServerTransport) -> AppServerRpcTransport {
    match transport {
        AppServerTransport::Stdio => AppServerRpcTransport::Stdio,
        AppServerTransport::UnixSocket { .. }
        | AppServerTransport::WebSocket { .. }
        | AppServerTransport::Off => AppServerRpcTransport::Websocket,
    }
}

#[cfg(test)]
mod tests {
    use super::LogFormat;
    #[cfg(debug_assertions)]
    use super::loader_overrides_with_test_user_config_file;
    #[cfg(debug_assertions)]
    use codex_config::LoaderOverrides;
    #[cfg(debug_assertions)]
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;

    #[test]
    fn log_format_from_env_value_matches_json_values_case_insensitively() {
        assert_eq!(LogFormat::from_env_value(Some("json")), LogFormat::Json);
        assert_eq!(LogFormat::from_env_value(Some("JSON")), LogFormat::Json);
        assert_eq!(LogFormat::from_env_value(Some("  Json  ")), LogFormat::Json);
    }

    #[test]
    fn log_format_from_env_value_defaults_for_non_json_values() {
        assert_eq!(
            LogFormat::from_env_value(/*value*/ None),
            LogFormat::Default
        );
        assert_eq!(LogFormat::from_env_value(Some("")), LogFormat::Default);
        assert_eq!(LogFormat::from_env_value(Some("text")), LogFormat::Default);
        assert_eq!(LogFormat::from_env_value(Some("jsonl")), LogFormat::Default);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn debug_test_user_config_file_overrides_loader_path() {
        let path = std::env::temp_dir().join("codex-app-server-test-config.toml");
        let loader_overrides = loader_overrides_with_test_user_config_file(
            LoaderOverrides::default(),
            Some(path.clone()),
        )
        .expect("test config path should be valid");

        assert_eq!(
            loader_overrides.user_config_path,
            Some(AbsolutePathBuf::from_absolute_path(path).expect("absolute test path"))
        );
    }
}
