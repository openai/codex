use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;

use tokio::sync::Mutex;
use tokio::time::Duration;
use tokio::time::Instant;
use tokio::time::Sleep;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::exec::ExecToolCallOutput;
use crate::exec::MAX_EXEC_OUTPUT_DELTAS_PER_CALL;
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

pub(crate) const TRAILING_OUTPUT_GRACE: Duration = Duration::from_millis(100);

/// Upper bound for a single ExecCommandOutputDelta chunk emitted by unified exec.
///
/// The unified exec output buffer already caps *retained* output (see
/// `UNIFIED_EXEC_OUTPUT_MAX_BYTES`), but we also cap per-event payload size so
/// downstream event consumers (especially app-server JSON-RPC) don't have to
/// process arbitrarily large delta payloads.
const UNIFIED_EXEC_OUTPUT_DELTA_MAX_BYTES: usize = 8192;

/// Spawn a background task that continuously reads from the PTY, appends to the
/// shared transcript, and emits ExecCommandOutputDelta events on UTF‑8
/// boundaries.
pub(crate) fn start_streaming_output(
    session: &UnifiedExecSession,
    context: &UnifiedExecContext,
    transcript: Arc<Mutex<CommandTranscript>>,
) {
    let mut receiver = session.output_receiver();
    let output_drained = session.output_drained_notify();
    let exit_token = session.cancellation_token();

    let session_ref = Arc::clone(&context.session);
    let turn_ref = Arc::clone(&context.turn);
    let call_id = context.call_id.clone();

    tokio::spawn(async move {
        use tokio::sync::broadcast::error::RecvError;

        let mut pending = Vec::<u8>::new();
        let mut emitted_deltas: usize = 0;

        let mut grace_sleep: Option<Pin<Box<Sleep>>> = None;

        loop {
            tokio::select! {
                _ = exit_token.cancelled(), if grace_sleep.is_none() => {
                    let deadline = Instant::now() + TRAILING_OUTPUT_GRACE;
                    grace_sleep.replace(Box::pin(tokio::time::sleep_until(deadline)));
                }

                _ = async {
                    if let Some(sleep) = grace_sleep.as_mut() {
                        sleep.as_mut().await;
                    }
                }, if grace_sleep.is_some() => {
                    output_drained.notify_one();
                    break;
                }

                received = receiver.recv() => {
                    let chunk = match received {
                        Ok(chunk) => chunk,
                        Err(RecvError::Lagged(_)) => {
                            continue;
                        },
                        Err(RecvError::Closed) => {
                            output_drained.notify_one();
                            break;
                        }
                    };

                    process_chunk(
                        &mut pending,
                        &transcript,
                        &call_id,
                        &session_ref,
                        &turn_ref,
                        &mut emitted_deltas,
                        chunk,
                    ).await;
                }
            }
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
    let output_drained = session.output_drained_notify();

    tokio::spawn(async move {
        exit_token.cancelled().await;
        output_drained.notified().await;

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

async fn process_chunk(
    pending: &mut Vec<u8>,
    transcript: &Arc<Mutex<CommandTranscript>>,
    call_id: &str,
    session_ref: &Arc<Session>,
    turn_ref: &Arc<TurnContext>,
    emitted_deltas: &mut usize,
    chunk: Vec<u8>,
) {
    pending.extend_from_slice(&chunk);
    while let Some(prefix) = split_valid_utf8_prefix(pending) {
        {
            let mut guard = transcript.lock().await;
            guard.append(&prefix);
        }

        if *emitted_deltas >= MAX_EXEC_OUTPUT_DELTAS_PER_CALL {
            continue;
        }

        let event = ExecCommandOutputDeltaEvent {
            call_id: call_id.to_string(),
            stream: ExecOutputStream::Stdout,
            chunk: prefix,
        };
        session_ref
            .send_event(turn_ref.as_ref(), EventMsg::ExecCommandOutputDelta(event))
            .await;
        *emitted_deltas += 1;
    }
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
    split_valid_utf8_prefix_with_max(buffer, UNIFIED_EXEC_OUTPUT_DELTA_MAX_BYTES)
}

fn split_valid_utf8_prefix_with_max(buffer: &mut Vec<u8>, max_bytes: usize) -> Option<Vec<u8>> {
    if buffer.is_empty() {
        return None;
    }

    let max_len = buffer.len().min(max_bytes);
    let mut split = max_len;
    while split > 0 {
        if std::str::from_utf8(&buffer[..split]).is_ok() {
            let prefix = buffer[..split].to_vec();
            buffer.drain(..split);
            return Some(prefix);
        }

        if max_len - split > 4 {
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

#[cfg(test)]
mod tests {
    use super::split_valid_utf8_prefix_with_max;

    use pretty_assertions::assert_eq;

    #[test]
    fn split_valid_utf8_prefix_respects_max_bytes_for_ascii() {
        let mut buf = b"hello word!".to_vec();

        let first = split_valid_utf8_prefix_with_max(&mut buf, 5).expect("expected prefix");
        assert_eq!(first, b"hello".to_vec());
        assert_eq!(buf, b" word!".to_vec());

        let second = split_valid_utf8_prefix_with_max(&mut buf, 5).expect("expected prefix");
        assert_eq!(second, b" word".to_vec());
        assert_eq!(buf, b"!".to_vec());
    }

    #[test]
    fn split_valid_utf8_prefix_avoids_splitting_utf8_codepoints() {
        // "é" is 2 bytes in UTF-8. With a max of 3 bytes, we should only emit 1 char (2 bytes).
        let mut buf = "ééé".as_bytes().to_vec();

        let first = split_valid_utf8_prefix_with_max(&mut buf, 3).expect("expected prefix");
        assert_eq!(std::str::from_utf8(&first).unwrap(), "é");
        assert_eq!(buf, "éé".as_bytes().to_vec());
    }

    #[test]
    fn split_valid_utf8_prefix_makes_progress_on_invalid_utf8() {
        let mut buf = vec![0xff, b'a', b'b'];

        let first = split_valid_utf8_prefix_with_max(&mut buf, 2).expect("expected prefix");
        assert_eq!(first, vec![0xff]);
        assert_eq!(buf, b"ab".to_vec());
    }
}
