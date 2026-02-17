//! Prototype MCP server.
#![deny(clippy::print_stdout, clippy::print_stderr)]

use std::collections::HashMap;
use std::io::ErrorKind;
use std::io::Result as IoResult;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::config::Config;
use codex_utils_cli::CliConfigOverrides;

use rmcp::model::ClientNotification;
use rmcp::model::ClientRequest;
use rmcp::model::JsonRpcMessage;
use serde_json::Value;
use tokio::io::AsyncBufReadExt;
use tokio::io::BufReader;
use tokio::io::{self};
use tokio::sync::mpsc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing_subscriber::EnvFilter;

mod codex_tool_config;
mod codex_tool_runner;
mod exec_approval;
pub(crate) mod message_processor;
mod outgoing_message;
mod patch_approval;
pub mod http_transport;
pub mod a2a_handler;

use crate::message_processor::MessageProcessor;
use crate::outgoing_message::OutgoingMessage;
use crate::outgoing_message::OutgoingMessageSender;

pub use crate::codex_tool_config::CodexToolCallParam;
pub use crate::codex_tool_config::CodexToolCallReplyParam;
pub use crate::exec_approval::ExecApprovalElicitRequestParams;
pub use crate::exec_approval::ExecApprovalResponse;
pub use crate::patch_approval::PatchApprovalElicitRequestParams;
pub use crate::patch_approval::PatchApprovalResponse;

/// Size of the bounded channels used to communicate between tasks.
const CHANNEL_CAPACITY: usize = 128;

type IncomingMessage = JsonRpcMessage<ClientRequest, Value, ClientNotification>;

/// Shared pending map type for routing MCP responses to their requestors.
type PendingMap = Arc<tokio::sync::Mutex<HashMap<String, tokio::sync::oneshot::Sender<crate::outgoing_message::OutgoingJsonRpcMessage>>>>;

/// Options for controlling which transports to start.
#[derive(Default)]
pub struct TransportOptions {
    /// If set, start an MCP HTTP server on this port in addition to (or
    /// instead of) stdin/stdout.
    pub http_port: Option<u16>,
    /// If set, start an A2A server on this port.
    pub a2a_port: Option<u16>,
    /// When true, disable the stdin/stdout transport entirely (HTTP-only mode).
    pub http_only: bool,
}

pub async fn run_main(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
) -> IoResult<()> {
    run_main_with_transport(
        codex_linux_sandbox_exe,
        cli_config_overrides,
        TransportOptions::default(),
    )
    .await
}

pub async fn run_main_with_transport(
    codex_linux_sandbox_exe: Option<PathBuf>,
    cli_config_overrides: CliConfigOverrides,
    transport: TransportOptions,
) -> IoResult<()> {
    // Install tracing subscriber.
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    // Set up channels.
    let (incoming_tx, mut incoming_rx) = mpsc::channel::<IncomingMessage>(CHANNEL_CAPACITY);
    let (outgoing_tx, outgoing_rx) = mpsc::unbounded_channel::<OutgoingMessage>();

    // Parse CLI overrides once and derive the base Config eagerly.
    let cli_kv_overrides = cli_config_overrides.parse_overrides().map_err(|e| {
        std::io::Error::new(
            ErrorKind::InvalidInput,
            format!("error parsing -c overrides: {e}"),
        )
    })?;
    let config = Config::load_with_cli_overrides(cli_kv_overrides)
        .await
        .map_err(|e| {
            std::io::Error::new(ErrorKind::InvalidData, format!("error loading config: {e}"))
        })?;

    // --- Stdin reader (optional) ---
    let stdin_handle = if !transport.http_only {
        let tx = incoming_tx.clone();
        Some(tokio::spawn(async move {
            let stdin = io::stdin();
            let reader = BufReader::new(stdin);
            let mut lines = reader.lines();

            while let Some(line) = lines.next_line().await.unwrap_or_default() {
                match serde_json::from_str::<IncomingMessage>(&line) {
                    Ok(msg) => {
                        if tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => error!("Failed to deserialize JSON-RPC message: {e}"),
                }
            }

            debug!("stdin reader finished (EOF)");
        }))
    } else {
        None
    };

    // Shared pending map: routes MCP responses to HTTP and A2A requestors.
    // Both A2A executor and HTTP handlers register oneshot channels here
    // using unique request IDs (A2A prefixes with "a2a-").
    let shared_pending: PendingMap = Arc::new(tokio::sync::Mutex::new(HashMap::new()));

    // --- A2A server (optional) ---
    let a2a_handle = if let Some(a2a_port) = transport.a2a_port {
        // Broadcast channel for forwarding MCP notifications to A2A event bus.
        let (a2a_notif_tx, _) = tokio::sync::broadcast::channel::<String>(256);

        let handler = a2a_handler::CodexA2AExecutor::new(
            incoming_tx.clone(),
            shared_pending.clone(),
            a2a_notif_tx.clone(),
        );

        let addr = format!("0.0.0.0:{a2a_port}");
        info!("A2A server listening on http://{addr}/");

        Some((tokio::spawn(async move {
            if let Err(e) = a2a_rs::A2AServer::new(handler, a2a_rs::InMemoryTaskStore::new())
                .bind(&addr)
                .run()
                .await
            {
                error!("A2A server error: {e}");
            }
        }), a2a_notif_tx))
    } else {
        None
    };

    // --- HTTP server (optional) ---
    let mut outgoing_rx = Some(outgoing_rx);
    let http_handle = if let Some(port) = transport.http_port {
        let (sse_tx, _) = tokio::sync::broadcast::channel::<String>(256);

        let state = http_transport::HttpState {
            incoming_tx: incoming_tx.clone(),
            pending: shared_pending.clone(),
            sse_tx: sse_tx.clone(),
        };

        let router = http_transport::build_router(state);

        // Start the outgoing interceptor that routes responses to HTTP/A2A
        // handlers and/or stdout.
        let write_stdout = !transport.http_only;
        let a2a_notif_tx_for_http = a2a_handle.as_ref().map(|(_, tx)| tx.clone());
        let interceptor_handle = tokio::spawn(
            http_transport::outgoing_http_interceptor(
                outgoing_rx.take().expect("outgoing_rx already taken"),
                shared_pending.clone(),
                sse_tx,
                write_stdout,
                a2a_notif_tx_for_http,
            ),
        );

        let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
        info!("HTTP MCP server listening on http://{addr}/mcp");

        let listener = tokio::net::TcpListener::bind(addr)
            .await
            .map_err(|e| {
                std::io::Error::new(
                    ErrorKind::AddrInUse,
                    format!("failed to bind HTTP server to {addr}: {e}"),
                )
            })?;

        let server_handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, router).await {
                error!("HTTP server error: {e}");
            }
        });

        Some((server_handle, interceptor_handle))
    } else {
        None
    };

    // --- Stdout writer (when no HTTP or dual mode without interceptor) ---
    let a2a_notif_tx_for_stdout = a2a_handle.as_ref().map(|(_, tx)| tx.clone());
    let stdout_pending = shared_pending.clone();
    let stdout_handle = if let Some(mut outgoing_rx) = outgoing_rx.take() {
        Some(tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut stdout = io::stdout();
            while let Some(outgoing_message) = outgoing_rx.recv().await {
                let msg: crate::outgoing_message::OutgoingJsonRpcMessage =
                    outgoing_message.into();

                // Route responses to pending requestors (from A2A executor).
                let id_str = extract_outgoing_id(&msg);
                let mut routed = false;
                if let Some(id) = id_str {
                    let mut map = stdout_pending.lock().await;
                    if let Some(tx) = map.remove(&id) {
                        let _ = tx.send(msg.clone());
                        routed = true;
                    }
                }

                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        // Forward notifications to A2A broadcast channel.
                        if let Some(ref a2a_tx) = a2a_notif_tx_for_stdout {
                            let _ = a2a_tx.send(json.clone());
                        }
                        if !routed {
                            if let Err(e) = stdout.write_all(json.as_bytes()).await {
                                error!("Failed to write to stdout: {e}");
                                break;
                            }
                            if let Err(e) = stdout.write_all(b"\n").await {
                                error!("Failed to write newline to stdout: {e}");
                                break;
                            }
                        }
                    }
                    Err(e) => error!("Failed to serialize JSON-RPC message: {e}"),
                }
            }
            info!("stdout writer exited (channel closed)");
        }))
    } else {
        None
    };

    // --- Message processor (shared between both transports) ---
    let processor_handle = tokio::spawn({
        let outgoing_message_sender = OutgoingMessageSender::new(outgoing_tx);
        let mut processor = MessageProcessor::new(
            outgoing_message_sender,
            codex_linux_sandbox_exe,
            std::sync::Arc::new(config),
        );
        async move {
            while let Some(msg) = incoming_rx.recv().await {
                match msg {
                    JsonRpcMessage::Request(r) => processor.process_request(r).await,
                    JsonRpcMessage::Response(r) => processor.process_response(r).await,
                    JsonRpcMessage::Notification(n) => processor.process_notification(n).await,
                    JsonRpcMessage::Error(e) => processor.process_error(e),
                }
            }
            info!("processor task exited (channel closed)");
        }
    });

    // Wait for tasks to complete.
    if let Some(h) = stdin_handle {
        let _ = h.await;
    }
    let _ = processor_handle.await;
    if let Some(h) = stdout_handle {
        let _ = h.await;
    }
    if let Some((server_h, interceptor_h)) = http_handle {
        let _ = tokio::join!(server_h, interceptor_h);
    }
    if let Some((h, _)) = a2a_handle {
        let _ = h.await;
    }

    Ok(())
}

/// Extract the JSON-RPC `id` field from a serialized outgoing message.
fn extract_outgoing_id(msg: &crate::outgoing_message::OutgoingJsonRpcMessage) -> Option<String> {
    if let Ok(v) = serde_json::to_value(msg) {
        if let Some(id) = v.get("id") {
            if !id.is_null() {
                return Some(id.to_string());
            }
        }
    }
    None
}
