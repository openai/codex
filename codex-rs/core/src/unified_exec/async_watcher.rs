use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::exec::StreamOutput;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecCommandSource;
use crate::protocol::ExecOutputStream;
use crate::tools::events::ToolEmitter;
use crate::tools::events::ToolEventCtx;
use crate::tools::events::ToolEventStage;

use super::CommandTranscript;
use super::UnifiedExecContext;
use super::session::UnifiedExecSession;

/// Spawn a background task that continuously reads from the PTY, appends to the
/// shared transcript, and emits ExecCommandOutputDelta events on UTF‑8
/// boundaries.
pub(crate) fn start_streaming_output(
    session: &UnifiedExecSession,
    context: &UnifiedExecContext,
    transcript: Arc<Mutex<CommandTranscript>>,
) {
    let mut receiver = session.output_receiver();
    let session_ref = Arc::clone(&context.session);
    let turn_ref = Arc::clone(&context.turn);
    let call_id = context.call_id.clone();
    let cancellation_token = session.cancellation_token();

    tokio::spawn(async move {
        let mut pending: Vec<u8> = Vec::new();
        loop {
            tokio::select! {
                _ = cancellation_token.cancelled() => break,
                result = receiver.recv() => match result {
                    Ok(chunk) => {
                        pending.extend_from_slice(&chunk);
                        while let Some(prefix) = split_valid_utf8_prefix(&mut pending) {
                            {
                                let mut guard = transcript.lock().await;
                                guard.append(&prefix);
                            }

                            let event = ExecCommandOutputDeltaEvent {
                                call_id: call_id.clone(),
                                stream: ExecOutputStream::Stdout,
                                chunk: prefix,
                            };
                            session_ref
                                .send_event(turn_ref.as_ref(), EventMsg::ExecCommandOutputDelta(event))
                                .await;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                }
            };
        }
    });
}

/// Spawn a background watcher that waits for the PTY to exit and then emits a
/// single ExecCommandEnd event with the aggregated transcript.
#[allow(clippy::too_many_arguments)]
pub(crate) fn spawn_exit_watcher(
    session: Arc<UnifiedExecSession>,
    session_ref: Arc<Session>,
    turn_ref: Arc<TurnContext>,
    call_id: String,
    command: Vec<String>,
    cwd: PathBuf,
    process_id: String,
    transcript: Arc<Mutex<CommandTranscript>>,
    started_at: Instant,
) {
    let exit_token = session.cancellation_token();

    tokio::spawn(async move {
        exit_token.cancelled().await;

        let exit_code = session.exit_code().unwrap_or(-1);
        let duration = Instant::now().saturating_duration_since(started_at);
        emit_exec_end_for_unified_exec(
            session_ref,
            turn_ref,
            call_id,
            command,
            cwd,
            Some(process_id),
            transcript,
            String::new(),
            exit_code,
            duration,
        )
        .await;
    });
}

/// Emit an ExecCommandEnd event for a unified exec session, using the transcript
/// as the primary source of aggregated_output and falling back to the provided
/// text when the transcript is empty.
#[allow(clippy::too_many_arguments)]
pub(crate) async fn emit_exec_end_for_unified_exec(
    session_ref: Arc<Session>,
    turn_ref: Arc<TurnContext>,
    call_id: String,
    command: Vec<String>,
    cwd: PathBuf,
    process_id: Option<String>,
    transcript: Arc<Mutex<CommandTranscript>>,
    fallback_output: String,
    exit_code: i32,
    duration: Duration,
) {
    let aggregated_output = resolve_aggregated_output(&transcript, fallback_output).await;
    let output = ExecToolCallOutput {
        exit_code,
        stdout: StreamOutput::new(aggregated_output.clone()),
        stderr: StreamOutput::new(String::new()),
        aggregated_output: StreamOutput::new(aggregated_output),
        duration,
        timed_out: false,
    };
    let event_ctx = ToolEventCtx::new(session_ref.as_ref(), turn_ref.as_ref(), &call_id, None);
    let emitter = ToolEmitter::unified_exec(
        &command,
        cwd,
        ExecCommandSource::UnifiedExecStartup,
        process_id,
    );
    emitter
        .emit(event_ctx, ToolEventStage::Success(output))
        .await;
}

fn split_valid_utf8_prefix(buffer: &mut Vec<u8>) -> Option<Vec<u8>> {
    if buffer.is_empty() {
        return None;
    }

    let len = buffer.len();
    let mut split = len;
    while split > 0 {
        if std::str::from_utf8(&buffer[..split]).is_ok() {
            let prefix = buffer[..split].to_vec();
            buffer.drain(..split);
            return Some(prefix);
        }

        if len - split > 4 {
            break;
        }
        split -= 1;
    }

    // If no valid UTF-8 prefix was found, emit the first byte so the stream
    // keeps making progress and the transcript reflects all bytes.
    let byte = buffer.drain(..1).collect();
    Some(byte)
}

async fn resolve_aggregated_output(
    transcript: &Arc<Mutex<CommandTranscript>>,
    fallback: String,
) -> String {
    let guard = transcript.lock().await;
    if guard.data.is_empty() {
        return fallback;
    }

    String::from_utf8_lossy(&guard.data).to_string()
}
