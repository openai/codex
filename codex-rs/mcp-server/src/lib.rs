//! Prototype MCP server.
#![deny(clippy::print_stdout, clippy::print_stderr)]

use std::io::Result as IoResult;
use std::path::PathBuf;

use mcp_types::JSONRPCMessage;
use tokio::io::{self, BufReader, AsyncBufReadExt, AsyncWriteExt};
use tokio::sync::mpsc;

mod codex_tool_config;
mod codex_tool_runner;
mod json_to_toml;
mod message_processor;

use crate::message_processor::MessageProcessor;

/// Size of the bounded channels used to communicate between tasks. The value
/// is a balance between throughput and memory usage – 128 messages should be
/// plenty for an interactive CLI.
const CHANNEL_CAPACITY: usize = 128;

pub async fn run_main(codex_linux_sandbox_exe: Option<PathBuf>) -> IoResult<()> {
    // Delegate to stream-driven runner
    run_with_streams(
        BufReader::new(io::stdin()),
        io::stdout(),
        codex_linux_sandbox_exe,
    )
    .await
}

/// Run an MCP server over arbitrary streams instead of stdio.
/// Useful for in-memory integration tests.
pub async fn run_with_streams<R, W>(
    reader: R,
    mut writer: W,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> IoResult<()>
where
    R: tokio::io::AsyncBufRead + Unpin + Send + 'static,
    W: tokio::io::AsyncWrite + Unpin + Send + 'static,
{
    // Initialize tracing (idempotent)
    let _ = tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .try_init();

    let (incoming_tx, mut incoming_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);
    let (outgoing_tx, mut outgoing_rx) = mpsc::channel::<JSONRPCMessage>(CHANNEL_CAPACITY);

    // Reader task: read from provided reader
    let reader_handle = tokio::spawn({
        let incoming_tx = incoming_tx.clone();
        let mut lines = reader.lines();
        async move {
            while let Ok(Some(line)) = lines.next_line().await {
                match serde_json::from_str::<JSONRPCMessage>(&line) {
                    Ok(msg) => {
                        if incoming_tx.send(msg).await.is_err() {
                            break;
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to parse JSONRPCMessage: {e}");
                    }
                }
            }
        }
    });

    // Processor task
    let processor_handle = tokio::spawn({
        let outgoing = outgoing_tx.clone();
        let mut processor = MessageProcessor::new(outgoing);
        async move {
            while let Some(msg) = incoming_rx.recv().await {
                match msg {
                    JSONRPCMessage::Request(r) => processor.process_request(r),
                    JSONRPCMessage::Response(r) => processor.process_response(r),
                    JSONRPCMessage::Notification(n) => processor.process_notification(n),
                    JSONRPCMessage::BatchRequest(b) => processor.process_batch_request(b),
                    JSONRPCMessage::Error(e) => processor.process_error(e),
                    JSONRPCMessage::BatchResponse(b) => processor.process_batch_response(b),
                }
            }
        }
    });

    // Writer task: write to provided writer
    let writer_handle = tokio::spawn(async move {
        while let Some(msg) = outgoing_rx.recv().await {
            if let Ok(json) = serde_json::to_string(&msg) {
                if writer.write_all(json.as_bytes()).await.is_err() {
                    break;
                }
                if writer.write_all(b"\n").await.is_err() {
                    break;
                }
                if writer.flush().await.is_err() {
                    break;
                }
            } else {
                tracing::error!("Failed to serialize JSONRPCMessage");
            }
        }
    });

    // Drop unused channel senders so writer and processor can exit
    drop(incoming_tx);
    drop(outgoing_tx);
    let _ = tokio::join!(reader_handle, processor_handle, writer_handle);
    Ok(())
}
