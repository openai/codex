#![deny(clippy::print_stdout, clippy::print_stderr)]

use codex_common::CliConfigOverrides;
use codex_core::config::Config;
use codex_core::config::ConfigBuilder;
use codex_core::config_loader::ConfigLayerStackOrdering;
use codex_core::config_loader::LoaderOverrides;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::PathBuf;

use crate::message_processor::MessageProcessor;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;
use codex_app_server_protocol::ConfigLayerSource;
use codex_app_server_protocol::ConfigWarningNotification;
use codex_app_server_protocol::JSONRPCMessage;
use codex_core::check_execpolicy_for_warnings;
use codex_feedback::CodexFeedback;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::{self};
use tokio::sync::mpsc;
use toml::Value as TomlValue;
use tracing::debug;
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
mod error_code;
mod fuzzy_file_search;
mod message_processor;
mod models;
mod outgoing_message;

/// Size of the bounded channels used to communicate between tasks. The value
/// is a balance between throughput and memory usage – 128 messages should be
/// plenty for an interactive CLI.
const CHANNEL_CAPACITY: usize = 128;

fn project_config_warning(config: &Config) -> Option<ConfigWarningNotification> {
    let mut disabled_folders = Vec::new();

    for layer in config
        .config_layer_stack
        .get_layers(ConfigLayerStackOrdering::LowestPrecedenceFirst, true)
    {
        if !matches!(layer.name, ConfigLayerSource::Project { .. }) {
            continue;
        }
        if layer.disabled_reason.is_none() {
            continue;
        };
        if let ConfigLayerSource::Project { dot_codex_folder } = &layer.name {
            disabled_folders.push((
                dot_codex_folder.as_path().display().to_string(),
                layer
                    .disabled_reason
                    .as_ref()
                    .map(ToString::to_string)
                    .unwrap_or_else(|| "Config folder disabled.".to_string()),
            ));
        }
    }

    if disabled_folders.is_empty() {
        return None;
    }

    let mut message = "The following config folders are disabled:\n".to_string();
    for (index, (folder, reason)) in disabled_folders.iter().enumerate() {
        let display_index = index + 1;
        message.push_str(&format!("    {display_index}. {folder}\n"));
        message.push_str(&format!("       {reason}\n"));
    }

    Some(ConfigWarningNotification {
        summary: message,
        details: None,
    })
}

pub async fn run_main(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    loader_overrides: LoaderOverrides,
    default_analytics_enabled: bool,
) -> IoResult<()> {
    // Set up channels.
    let (incoming_tx, mut incoming_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<OutgoingMessage>(CHANNEL_CAPACITY);

    // Task: read from stdin, push to `incoming_tx`.
    let stdin_reader_handle = tokio::spawn({
        async move {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await.unwrap_or_default() {
                match serde_json::from_str::<JSONRPCMessage>(&line) {
                    Ok(msg) => {
                        if incoming_tx.send(msg).await.is_err() {
                            // Receiver gone – nothing left to do.
                            break;
                        }
                    }
                    Err(e) => error!("Failed to deserialize JSONRPCMessage: {e}"),
                }
            }

            debug!("stdin reader finished (EOF)");
        }
    });

    // Parse CLI overrides once and derive the base Config eagerly so later
    // components do not need to work with raw TOML values.
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;
    let loader_overrides_for_config_api = loader_overrides.clone();
    let mut config_warnings = Vec::new();
    let config = match ConfigBuilder::default()
        .cli_overrides(cli_kv_overrides.clone())
        .loader_overrides(loader_overrides)
        .build()
        .await
    {
        Ok(config) => config,
        Err(err) => {
            let message = ConfigWarningNotification {
                summary: "Invalid configuration; using defaults.".to_string(),
                details: Some(err.to_string()),
            };
            config_warnings.push(message);
            Config::load_default_with_cli_overrides(cli_kv_overrides.clone()).map_err(|e| {
                std::io::Error::new(
                    ErrorKind::InvalidData,
                    format!("error loading default config after config error: {e}"),
                )
            })?
        }
    };

    if let Ok(Some(err)) =
        check_execpolicy_for_warnings(&config.features, &config.config_layer_stack).await
    {
        let message = ConfigWarningNotification {
            summary: "Error parsing rules; custom rules not applied.".to_string(),
            details: Some(err.to_string()),
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

    // Task: process incoming messages.
    let processor_handle = tokio::spawn({
        let outgoing_message_sender = OutgoingMessageSender::new(outgoing_tx);
        let cli_overrides: Vec<(String, TomlValue)> = cli_kv_overrides.clone();
        let loader_overrides = loader_overrides_for_config_api;
        let mut processor = MessageProcessor::new(
            outgoing_message_sender,
            codex_linux_sandbox_exe,
            std::sync::Arc::new(config),
            cli_overrides,
            loader_overrides,
            feedback.clone(),
            config_warnings,
        );
        let mut thread_created_rx = processor.thread_created_receiver();
        async move {
            let mut listen_for_threads = true;
            loop {
                tokio::select! {
                    msg = incoming_rx.recv() => {
                        let Some(msg) = msg else {
                            break;
                        };
                        match msg {
                            JSONRPCMessage::Request(r) => processor.process_request(r).await,
                            JSONRPCMessage::Response(r) => processor.process_response(r).await,
                            JSONRPCMessage::Notification(n) => processor.process_notification(n).await,
                            JSONRPCMessage::Error(e) => processor.process_error(e),
                        }
                    }
                    created = thread_created_rx.recv(), if listen_for_threads => {
                        match created {
                            Ok(thread_id) => {
                                processor.try_attach_thread_listener(thread_id).await;
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

    // Task: write outgoing messages to stdout.
    let stdout_writer_handle = tokio::spawn(async move {
        let mut stdout = io::stdout();
        while let Some(outgoing_message) = outgoing_rx.recv().await {
            let Ok(value) = serde_json::to_value(outgoing_message) else {
                error!("Failed to convert OutgoingMessage to JSON value");
                continue;
            };
            match serde_json::to_string(&value) {
                Ok(mut json) => {
                    json.push('\n');
                    if let Err(e) = stdout.write_all(json.as_bytes()).await {
                        error!("Failed to write to stdout: {e}");
                        break;
                    }
                }
                Err(e) => error!("Failed to serialize JSONRPCMessage: {e}"),
            }
        }

        info!("stdout writer exited (channel closed)");
    });

    // Wait for all tasks to finish.  The typical exit path is the stdin reader
    // hitting EOF which, once it drops `incoming_tx`, propagates shutdown to
    // the processor and then to the stdout task.
    let _ = tokio::join!(stdin_reader_handle, processor_handle, stdout_writer_handle);

    Ok(())
}
