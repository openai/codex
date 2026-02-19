#![deny(clippy::print_stdout, clippy::print_stderr)]

use codex_cloud_requirements::cloud_requirements_loader;
use codex_core::AuthManager;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::CloudRequirementsLoader;
use codex_core::config_loader::ConfigLayerStackOrdering;
use codex_core::config_loader::LoaderOverrides;
use codex_utils_cli::CliConfigOverrides;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::RwLock;
use std::sync::atomic::AtomicBool;

use crate::message_processor::MessageProcessor;
use crate::message_processor::MessageProcessorArgs;
use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingEnvelope;
use crate::outgoing_message::OutgoingMessageSender;
use crate::transport::CHANNEL_CAPACITY;
use crate::transport::ConnectionState;
use crate::transport::OutboundConnectionState;
use crate::transport::TransportEvent;
use crate::transport::route_outgoing_envelope;
use crate::transport::start_stdio_connection;
use crate::transport::start_websocket_acceptor;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::TextPosition as AppTextPosition;
use codex_app_server_protocol::TextRange as AppTextRange;
use codex_core::ExecPolicyError;
use codex_core::check_execpolicy_for_warnings;
use codex_core::config_loader::ConfigLoadError;
use codex_core::config_loader::TextRange as CoreTextRange;
use codex_feedback::CodexFeedback;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use toml::Value as TomlValue;
use tracing::error;
use tracing::info;
use tracing::warn;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

mod bespoke_event_handling;
mod codex_message_processor;
mod config_api;
mod dynamic_tools;
mod error_code;
mod filters;
mod fuzzy_file_search;
mod message_processor;
mod models;
mod outgoing_message;
mod thread_state;
mod thread_status;
mod transport;

pub use crate::transport::AppServerTransport;

/// Control-plane messages from the processor/transport side to the outbound router task.
///
/// `run_main_with_transport` now uses two loops/tasks:
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
        writer: mpsc::Sender<crate::outgoing_message::OutgoingMessage>,
        initialized: Arc<AtomicBool>,
        opted_out_notification_methods: Arc<RwLock<HashSet<String>>>,
    },
    /// Remove state for a closed/disconnected connection.
    Closed { connection_id: ConnectionId },
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

    for layer in config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
    {
        if !matches!(layer.name, ConfigLayerSource::Project { .. })
            || layer.disabled_reason.is_none()
        {
            continue;
        }
        if let ConfigLayerSource::Project { dot_codex_folder } = &layer.name {
            disabled_folders.push((
                dot_codex_folder.as_path().display().to_string(),
                layer
                    .disabled_reason
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "config.toml is disabled.".to_string()),
            ));
        }
    }

    if disabled_folders.is_empty() {
        return None;
    }

    let mut message = concat!(
        "Project config.toml files are disabled in the following folders. ",
        "Settings in those files are ignored, but skills and exec policies still load.\n",
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

pub async fn run_main(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    default_analytics_enabled: bool,
) -> IoResult<()> {
    run_main_with_transport(
        codex_linux_sandbox_exe,
        cli_config_overrides,
        loader_overrides,
        default_analytics_enabled,
        AppServerTransport::Stdio,
    )
    .await
}

pub async fn run_main_with_transport(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    default_analytics_enabled: bool,
    transport: AppServerTransport,
) -> IoResult<()> {
    let (transport_event_tx, mut transport_event_rx) =
        mpsc::channel::<TransportEvent>(CHANNEL_CAPACITY);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingEnvelope>(CHANNEL_CAPACITY);
    let (outbound_control_tx, mut outbound_control_rx) =
        mpsc::channel::<OutboundControlEvent>(CHANNEL_CAPACITY);

    let mut stdio_handles = Vec::<JoinHandle<()>>::new();
    let mut websocket_accept_handle = None;
    match transport {
        AppServerTransport::Stdio => {
            start_stdio_connection(transport_event_tx.clone(), &mut stdio_handles).await?;
        }
        AppServerTransport::WebSocket { bind_address } => {
            websocket_accept_handle =
                Some(start_websocket_acceptor(bind_address, transport_event_tx.clone()).await?);
        }
    }
    let shutdown_when_no_connections = matches!(transport, AppServerTransport::Stdio);

    // Parse CLI overrides once and derive the base Config eagerly so later
    // components do not need to work with raw TOML values.
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;
    let cloud_requirements = match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides.clone())
        .loader_overrides(loader_overrides.clone())
        .build()
        .await
    {
        Ok(config) => {
            let effective_toml = config.config_layer_stack.effective_config();
            match effective_toml.try_into() {
                Ok(config_toml) => {
                    if let Err(err) = codex_core::personality_migration::maybe_migrate_personality(
                        &config.codex_home,
                        &config_toml,
                    )
                    .await
                    {
                        warn!(error = %err, "Failed to run personality migration");
                    }
                }
                Err(err) => {
                    warn!(error = %err, "Failed to deserialize config for personality migration");
                }
            }

            let auth_manager = AuthManager::shared(
                config.codex_home.clone(),
                false,
                config.cli_auth_credentials_store_mode,
            );
            cloud_requirements_loader(
                auth_manager,
                config.chatgpt_base_url,
                config.codex_home.clone(),
            )
        }
        Err(err) => {
            warn!(error = %err, "Failed to preload config for cloud requirements");
            // TODO(gt): Make cloud requirements preload failures blocking once we can fail-closed.
            CloudRequirementsLoader::default()
        }
    };
    let loader_overrides_for_config_api = loader_overrides.clone();
    let mut config_warnings = Vec::new();
    let config = match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides.clone())
        .loader_overrides(loader_overrides)
        .cloud_requirements(cloud_requirements.clone())
        .build()
        .await
    {
        Ok(config) => config,
        Err(err) => {
            let message = config_warning_from_error("Invalid configuration; using defaults.", &err);
            config_warnings.push(message);
            Config::load_default_with_cli_overrides(cli_kv_overrides.clone()).map_err(|e| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("error loading default config after config error: {e}"),
                )
            })?
        }
    };

    if let Ok(Some(err)) = check_execpolicy_for_warnings(&config.config_layer_stack).await {
        let (path, range) = exec_policy_warning_location(&err);
        let message = ConfigWarningNotification {
            summary: "Error parsing rules; custom rules not applied.".to_string(),
            details: Some(err.to_string()),
            path,
            range,
        };
        config_warnings.push(message);
    }

    if let Some(warning) = project_config_warning(&config) {
        config_warnings.push(warning);
    }

    let feedback = CodexFeedback::new();

    let otel = codex_core::otel_init::build_provider(
        &config,
        env!("CARGO_PKG_VERSION"),
        Some("codex_app_server"),
        default_analytics_enabled,
    )
    .map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidData,
            format!("error loading otel config: {e}"),
        )
    })?;

    // Install a simple subscriber so `tracing` output is visible.  Users can
    // control the log level with `RUST_LOG`.
    let stderr_fmt = tracing_subscriber::fmt::layer()
        .with_writer(std::io::stderr)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::FULL)
        .with_filter(EnvFilter::from_default_env());

    let feedback_layer = feedback.logger_layer();
    let feedback_metadata_layer = feedback.metadata_layer();

    let otel_logger_layer = otel.as_ref().and_then(|o| o.logger_layer());

    let otel_tracing_layer = otel.as_ref().and_then(|o| o.tracing_layer());

    let _ = tracing_subscriber::registry()
        .with(stderr_fmt)
        .with(feedback_layer)
        .with(feedback_metadata_layer)
        .with(otel_logger_layer)
        .with(otel_tracing_layer)
        .try_init();
    for warning in &config_warnings {
        match &warning.details {
            Some(details) => error!("{} {}", warning.summary, details),
            None => error!("{}", warning.summary),
        }
    }

    let transport_event_tx_for_outbound = transport_event_tx.clone();
    let outbound_handle = tokio::spawn(async move {
        let mut outbound_connections = HashMap::<ConnectionId, OutboundConnectionState>::new();
        let mut pending_closed_connections = VecDeque::<ConnectionId>::new();
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
                            initialized,
                            opted_out_notification_methods,
                        } => {
                            outbound_connections.insert(
                                connection_id,
                                OutboundConnectionState::new(
                                    writer,
                                    initialized,
                                    opted_out_notification_methods,
                                ),
                            );
                        }
                        OutboundControlEvent::Closed { connection_id } => {
                            outbound_connections.remove(&connection_id);
                        }
                    }
                }
                envelope = outgoing_rx.recv() => {
                    let Some(envelope) = envelope else {
                        break;
                    };
                    let disconnected_connections =
                        route_outgoing_envelope(&mut outbound_connections, envelope).await;
                    pending_closed_connections.extend(disconnected_connections);
                }
            }

            while let Some(connection_id) = pending_closed_connections.front().copied() {
                match transport_event_tx_for_outbound
                    .try_send(TransportEvent::ConnectionClosed { connection_id })
                {
                    Ok(()) => {
                        pending_closed_connections.pop_front();
                    }
                    Err(mpsc::error::TrySendError::Full(_)) => {
                        break;
                    }
                    Err(mpsc::error::TrySendError::Closed(_)) => {
                        return;
                    }
                }
            }
        }
        info!("outbound router task exited (channel closed)");
    });

    let processor_handle = tokio::spawn({
        let outgoing_message_sender = Arc::new(OutgoingMessageSender::new(outgoing_tx));
        let outbound_control_tx = outbound_control_tx;
        let cli_overrides: Vec<(String, TomlValue)> = cli_kv_overrides.clone();
        let loader_overrides = loader_overrides_for_config_api;
        let mut processor = MessageProcessor::new(MessageProcessorArgs {
            outgoing: outgoing_message_sender,
            codex_linux_sandbox_exe,
            config: Arc::new(config),
            cli_overrides,
            loader_overrides,
            cloud_requirements: cloud_requirements.clone(),
            feedback: feedback.clone(),
            config_warnings,
        });
        let mut thread_created_rx = processor.thread_created_receiver();
        let mut connections = HashMap::<ConnectionId, ConnectionState>::new();
        async move {
            let mut listen_for_threads = true;
            loop {
                tokio::select! {
                    event = transport_event_rx.recv() => {
                        let Some(event) = event else {
                            break;
                        };
                        match event {
                            TransportEvent::ConnectionOpened { connection_id, writer } => {
                                let outbound_initialized = Arc::new(AtomicBool::new(false));
                                let outbound_opted_out_notification_methods =
                                    Arc::new(RwLock::new(HashSet::new()));
                                if outbound_control_tx
                                    .send(OutboundControlEvent::Opened {
                                        connection_id,
                                        writer,
                                        initialized: Arc::clone(&outbound_initialized),
                                        opted_out_notification_methods: Arc::clone(
                                            &outbound_opted_out_notification_methods,
                                        ),
                                    })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                connections.insert(
                                    connection_id,
                                    ConnectionState::new(
                                        outbound_initialized,
                                        outbound_opted_out_notification_methods,
                                    ),
                                );
                            }
                            TransportEvent::ConnectionClosed { connection_id } => {
                                if outbound_control_tx
                                    .send(OutboundControlEvent::Closed { connection_id })
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                                processor.connection_closed(connection_id).await;
                                connections.remove(&connection_id);
                                if shutdown_when_no_connections && connections.is_empty() {
                                    break;
                                }
                            }
                            TransportEvent::IncomingMessage { connection_id, message } => {
                                match message {
                                    JSONRPCMessage::Request(request) => {
                                        let Some(connection_state) = connections.get_mut(&connection_id) else {
                                            warn!("dropping request from unknown connection: {:?}", connection_id);
                                            continue;
                                        };
                                        let was_initialized = connection_state.session.initialized;
                                        processor
                                            .process_request(
                                                connection_id,
                                                request,
                                                &mut connection_state.session,
                                                &connection_state.outbound_initialized,
                                            )
                                            .await;
                                        if let Ok(mut opted_out_notification_methods) = connection_state
                                            .outbound_opted_out_notification_methods
                                            .write()
                                        {
                                            *opted_out_notification_methods = connection_state
                                                .session
                                                .opted_out_notification_methods
                                                .clone();
                                        } else {
                                            warn!(
                                                "failed to update outbound opted-out notifications"
                                            );
                                        }
                                        if !was_initialized && connection_state.session.initialized {
                                            processor.send_initialize_notifications().await;
                                        }
                                    }
                                    JSONRPCMessage::Response(response) => {
                                        processor.process_response(response).await;
                                    }
                                    JSONRPCMessage::Notification(notification) => {
                                        processor.process_notification(notification).await;
                                    }
                                    JSONRPCMessage::Error(err) => {
                                        processor.process_error(err).await;
                                    }
                                }
                            }
                        }
                    }
                    created = thread_created_rx.recv(), if listen_for_threads => {
                        match created {
                            Ok(thread_id) => {
                                let initialized_connection_ids: Vec<ConnectionId> = connections
                                    .iter()
                                    .filter_map(|(connection_id, connection_state)| {
                                        connection_state.session.initialized.then_some(*connection_id)
                                    })
                                    .collect();
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
            }

            info!("processor task exited (channel closed)");
        }
    });

    drop(transport_event_tx);

    let _ = processor_handle.await;
    let _ = outbound_handle.await;

    if let Some(handle) = websocket_accept_handle {
        handle.abort();
    }

    for handle in stdio_handles {
        let _ = handle.await;
    }

    Ok(())
}
