use super::RealtimeHandoffState;
use super::RealtimeInputChannels;
use super::RealtimeSessionKind;
use super::RealtimeSidebandSessionInitialization;
use super::RealtimeTranscriptTailFlush;
use super::RealtimeWebrtcSidebandInputTask;
use super::spawn_webrtc_sideband_input_task;
use crate::client::ModelClient;
use async_channel::Sender;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeTranscriptState;
use codex_api::RealtimeWebsocketClient;
use codex_api::map_api_error;
use codex_login::default_client::default_headers;
use codex_protocol::error::Result as CodexResult;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) struct ExistingCallAttachment {
    pub(super) client: RealtimeWebsocketClient,
    pub(super) model_client: ModelClient,
    pub(super) session_config: RealtimeSessionConfig,
    pub(super) call_id: String,
    pub(super) extra_headers: HeaderMap,
    pub(super) input_channels: RealtimeInputChannels,
    pub(super) events_tx: Sender<RealtimeEvent>,
    pub(super) handoff_state: RealtimeHandoffState,
    pub(super) session_kind: RealtimeSessionKind,
    pub(super) event_parser: RealtimeEventParser,
    pub(super) realtime_active: Arc<AtomicBool>,
    pub(super) transcript_tail_flush: RealtimeTranscriptTailFlush,
    pub(super) stop_token: CancellationToken,
}

pub(super) async fn attach(attachment: ExistingCallAttachment) -> CodexResult<JoinHandle<()>> {
    let ExistingCallAttachment {
        client,
        model_client,
        session_config,
        call_id,
        extra_headers,
        input_channels,
        events_tx,
        handoff_state,
        session_kind,
        event_parser,
        realtime_active,
        transcript_tail_flush,
        stop_token,
    } = attachment;
    let sideband_headers = model_client
        .realtime_sideband_headers(extra_headers)
        .await?;
    let transcript_state = RealtimeTranscriptState::default();
    let connection = client
        .connect_existing_call_sideband(
            session_config.clone(),
            &call_id,
            sideband_headers.clone(),
            default_headers(),
            transcript_state,
        )
        .await
        .map_err(map_api_error)?;

    Ok(spawn_webrtc_sideband_input_task(
        RealtimeWebrtcSidebandInputTask {
            client,
            session_config,
            call_id,
            sideband_headers,
            session_initialization: RealtimeSidebandSessionInitialization::ExistingCall {
                initial_connection: Some(connection),
            },
            input_channels,
            events_tx,
            handoff_state,
            session_kind,
            event_parser,
            realtime_active,
            transcript_tail_flush,
            stop_token,
        },
    ))
}
