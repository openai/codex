use super::RealtimeInputTask;
use super::RealtimeInputTaskExit;
use super::RealtimeWebrtcSidebandInputTask;
use super::flush_realtime_transcript_tail;
use super::report_realtime_transport_loss;
use super::run_realtime_input_task;
use codex_api::ApiError;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeTranscriptState;
use codex_api::map_api_error;
use codex_login::default_client::default_headers;
use http::StatusCode;
use std::sync::atomic::Ordering;
use std::time::Duration;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio::time::sleep;
use tracing::info;
use tracing::warn;

const RECONNECT_BASE_DELAY: Duration = Duration::from_millis(200);
const RECONNECT_MAX_DELAY: Duration = Duration::from_secs(5);
const STABLE_CONNECTION_DURATION: Duration = Duration::from_secs(30);

pub(super) fn spawn_webrtc_sideband_input_task(
    input: RealtimeWebrtcSidebandInputTask,
) -> JoinHandle<()> {
    let RealtimeWebrtcSidebandInputTask {
        client,
        session_config,
        call_id,
        sideband_headers,
        input_channels,
        events_tx,
        handoff_state,
        session_kind,
        event_parser,
        realtime_active,
        transcript_tail_flush,
        stop_token,
    } = input;

    tokio::spawn(async move {
        let mut reconnecting = false;
        let mut rapid_disconnects = 0_u32;
        let mut pending_outbound = None;
        let transcript_state = RealtimeTranscriptState::default();
        while realtime_active.load(Ordering::Relaxed) {
            let connection = match tokio::select! {
                biased;
                _ = stop_token.cancelled() => break,
                connection = client.connect_webrtc_sideband(
                    session_config.clone(),
                    &call_id,
                    sideband_headers.clone(),
                    default_headers(),
                    transcript_state.clone(),
                ) => connection,
            } {
                Ok(connection) => connection,
                Err(err) => {
                    if !realtime_active.load(Ordering::Relaxed) || stop_token.is_cancelled() {
                        break;
                    }
                    if reconnecting && webrtc_sideband_session_ended(&err) {
                        info!(
                            call_id,
                            "realtime sideband session ended while reconnecting"
                        );
                        break;
                    }
                    let mapped_error = map_api_error(err);
                    warn!(
                        reconnecting,
                        "failed to connect realtime sideband: {mapped_error}"
                    );
                    let _ = events_tx
                        .send(RealtimeEvent::Error(mapped_error.to_string()))
                        .await;
                    break;
                }
            };

            if !realtime_active.load(Ordering::Relaxed) || stop_token.is_cancelled() {
                break;
            }

            let connected_at = Instant::now();
            let exit = run_realtime_input_task(
                RealtimeInputTask {
                    writer: connection.writer(),
                    events: connection.events(),
                    text_rx: input_channels.text_rx.clone(),
                    handoff_output_rx: input_channels.handoff_output_rx.clone(),
                    audio_rx: input_channels.audio_rx.clone(),
                    events_tx: events_tx.clone(),
                    handoff_state: handoff_state.clone(),
                    session_kind,
                    event_parser,
                    stop_token: stop_token.clone(),
                },
                pending_outbound.take(),
            )
            .await;

            let RealtimeInputTaskExit::TransportLost {
                err,
                pending_outbound: failed_outbound,
            } = exit
            else {
                break;
            };
            pending_outbound = failed_outbound;
            if !realtime_active.load(Ordering::Relaxed) || stop_token.is_cancelled() {
                break;
            }
            if event_parser != RealtimeEventParser::FramelessBidi {
                report_realtime_transport_loss(&events_tx, err).await;
                break;
            }
            if connected_at.elapsed() >= STABLE_CONNECTION_DURATION {
                rapid_disconnects = 0;
            }
            rapid_disconnects = rapid_disconnects.saturating_add(1);
            let delay = reconnect_delay(rapid_disconnects);
            warn!(
                call_id,
                delay_ms = delay.as_millis(),
                "live sideband transport lost; reconnecting: {err}"
            );
            reconnecting = true;
            tokio::select! {
                biased;
                _ = stop_token.cancelled() => break,
                _ = sleep(delay) => {}
            }
        }

        let transcript_tail = transcript_state.take_transcript_tail().await;
        flush_realtime_transcript_tail(&transcript_tail_flush, &transcript_tail).await;
    })
}

fn webrtc_sideband_session_ended(err: &ApiError) -> bool {
    matches!(
        err,
        ApiError::Api { status, .. }
            if matches!(*status, StatusCode::NOT_FOUND | StatusCode::GONE)
    )
}

fn reconnect_delay(rapid_disconnects: u32) -> Duration {
    let multiplier = 2_u32.saturating_pow(rapid_disconnects.saturating_sub(1));
    RECONNECT_BASE_DELAY
        .saturating_mul(multiplier)
        .min(RECONNECT_MAX_DELAY)
}

#[cfg(test)]
#[path = "sideband_tests.rs"]
mod tests;
