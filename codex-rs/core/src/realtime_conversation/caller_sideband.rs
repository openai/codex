//! Drives caller-owned existing calls without coupling inbound events to outbound backpressure.
//! Detach stops writes and drains accepted inbound events before flushing the transcript.
//! Pending writes are never replayed.
//! Existing calls support v1/v3 only, so inbound handling does not issue v2 response-control writes.

use super::RealtimeInputTask;
use super::RealtimeInputTaskExit;
use super::RealtimeResponseCreateQueue;
use super::RealtimeSessionKind;
use super::RealtimeTranscriptTailFlush;
use super::classify_realtime_input_error;
use super::flush_realtime_transcript_tail;
use super::handle_handoff_output;
use super::handle_realtime_server_event;
use super::handle_text_input;
use super::handle_user_audio_input;
use super::realtime_transcript_delta;
use super::report_realtime_transport_loss;
use crate::context::RealtimeDelegation;
use tokio::task::JoinHandle;

pub(super) fn spawn_input_task(
    input: RealtimeInputTask,
    transcript_tail_flush: RealtimeTranscriptTailFlush,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let RealtimeInputTask {
            writer,
            events,
            text_rx,
            handoff_output_rx,
            audio_rx,
            events_tx,
            handoff_state,
            session_kind,
            event_parser,
            stop_token,
        } = input;
        debug_assert!(matches!(session_kind, RealtimeSessionKind::V1));
        let transcript_state = events.transcript_state();

        let read = async {
            let mut output_audio_state = None;
            let mut response_create_queue = RealtimeResponseCreateQueue::default();
            loop {
                let Some(event) = events.next_event().await? else {
                    return anyhow::Ok(());
                };
                let completed = events
                    .take_completed_transcript(&event, RealtimeDelegation::FIELD_BYTES)
                    .await;
                handle_realtime_server_event(
                    (event, realtime_transcript_delta(&completed)),
                    &writer,
                    &events_tx,
                    &handoff_state,
                    session_kind,
                    &mut output_audio_state,
                    &mut response_create_queue,
                )
                .await?;
            }
        };
        let write = async {
            let mut response_create_queue = RealtimeResponseCreateQueue::default();
            loop {
                tokio::select! {
                    text = text_rx.recv() => handle_text_input(text, &writer).await?,
                    output = handoff_output_rx.recv() => {
                        handle_handoff_output(
                            output,
                            &writer,
                            &handoff_state,
                            event_parser,
                            &mut response_create_queue,
                        ).await?;
                    }
                    audio = audio_rx.recv() => handle_user_audio_input(audio, &writer).await?,
                }
            }
        };

        // Stop accepting input before draining it, and keep the read future alive
        // even when it already owns an event awaiting delivery to the fanout task.
        let result = {
            tokio::pin!(read);
            tokio::select! {
                biased;
                _ = stop_token.cancelled() => {
                    writer.close_sideband_input();
                    let _ = read.await;
                    Ok(())
                },
                result = &mut read => result,
                result = write => {
                    writer.close_sideband_input();
                    let _ = read.await;
                    result
                },
            }
        };
        // Dropping the last writer aborts the transport pump without waiting for
        // space in the host's outgoing queue (including for a close command).
        drop(writer);
        if let Err(err) = result
            && let RealtimeInputTaskExit::TransportLost { err, .. } =
                classify_realtime_input_error(err)
        {
            report_realtime_transport_loss(&events_tx, err).await;
        }
        let transcript_tail = transcript_state.take_transcript_tail().await;
        flush_realtime_transcript_tail(&transcript_tail_flush, &transcript_tail).await;
    })
}
