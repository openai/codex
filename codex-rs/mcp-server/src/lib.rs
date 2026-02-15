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

    // --- A2A server (optional) ---
    let a2a_handle = if let Some(a2a_port) = transport.a2a_port {
        let pending = Arc::new(tokio::sync::Mutex::new(
            HashMap::<String, tokio::sync::oneshot::Sender<crate::outgoing_message::OutgoingJsonRpcMessage>>::new(),
        ));

        let handler = a2a_handler::CodexA2AExecutor::new(
            incoming_tx.clone(),
            pending.clone(),
        );

        let addr = format!("0.0.0.0:{a2a_port}");
        info!("A2A server listening on http://{addr}/");

        Some(tokio::spawn(async move {
            if let Err(e) = a2a_rs::A2AServer::new(handler, a2a_rs::InMemoryTaskStore::new())
                .bind(&addr)
                .run()
                .await
            {
                error!("A2A server error: {e}");
            }
        }))
    } else {
        None
    };

    // --- HTTP server (optional) ---
    let mut outgoing_rx = Some(outgoing_rx);
    let http_handle = if let Some(port) = transport.http_port {
        let pending = Arc::new(tokio::sync::Mutex::new(
            HashMap::<String, tokio::sync::oneshot::Sender<crate::outgoing_message::OutgoingJsonRpcMessage>>::new(),
        ));
        let (sse_tx, _) = tokio::sync::broadcast::channel::<String>(256);

        let state = http_transport::HttpState {
            incoming_tx: incoming_tx.clone(),
            pending: pending.clone(),
            sse_tx: sse_tx.clone(),
        };

        let router = http_transport::build_router(state);

        // Start the outgoing interceptor that routes responses to HTTP
        // handlers and/or stdout.
        let write_stdout = !transport.http_only;
        let interceptor_handle = tokio::spawn(
            http_transport::outgoing_http_interceptor(
                outgoing_rx.take().expect("outgoing_rx already taken"),
                pending,
                sse_tx,
                write_stdout,
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
    let stdout_handle = if let Some(mut outgoing_rx) = outgoing_rx.take() {
        Some(tokio::spawn(async move {
            use tokio::io::AsyncWriteExt;
            let mut stdout = io::stdout();
            while let Some(outgoing_message) = outgoing_rx.recv().await {
                let msg: crate::outgoing_message::OutgoingJsonRpcMessage =
                    outgoing_message.into();
                match serde_json::to_string(&msg) {
                    Ok(json) => {
                        if let Err(e) = stdout.write_all(json.as_bytes()).await {
                            error!("Failed to write to stdout: {e}");
                            break;
                        }
                        if let Err(e) = stdout.write_all(b"\n").await {
                            error!("Failed to write newline to stdout: {e}");
                            break;
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
    if let Some(h) = a2a_handle {
        let _ = h.await;
    }

    Ok(())
}
