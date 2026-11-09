use super::RealtimeHandoffState;
use super::RealtimeInputChannels;
use super::RealtimeOutput;
use super::RealtimeSessionKind;
use super::RealtimeSidebandSessionInitialization;
use super::RealtimeTranscriptTailFlush;
use super::RealtimeWebrtcSidebandInputTask;
use super::spawn_webrtc_sideband_input_task;
use crate::client::ModelClient;
use async_channel::Sender;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeTranscriptState;
use codex_api::RealtimeWebsocketClient;
use codex_api::map_api_error;
use codex_async_utils::OrCancelExt;
use codex_http_client::HttpClientFactory;
use codex_login::default_client::default_headers;
use codex_protocol::error::Result as CodexResult;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

pub(super) struct ExistingCallAttachment {
    pub(super) attachment_cancellation: CancellationToken,
    pub(super) client: RealtimeWebsocketClient,
    pub(super) model_client: ModelClient,
    pub(super) auth_context: codex_model_provider::ProviderAuthContext,
    pub(super) http_client_factory: HttpClientFactory,
    pub(super) sideband_base_url: String,
    pub(super) session_config: RealtimeSessionConfig,
    pub(super) call_id: String,
    pub(super) extra_headers: HeaderMap,
    pub(super) input_channels: RealtimeInputChannels,
    pub(super) events_tx: Sender<RealtimeOutput>,
    pub(super) handoff_state: RealtimeHandoffState,
    pub(super) session_kind: RealtimeSessionKind,
    pub(super) event_parser: RealtimeEventParser,
    pub(super) realtime_active: Arc<AtomicBool>,
    pub(super) transcript_tail_flush: RealtimeTranscriptTailFlush,
    pub(super) stop_token: CancellationToken,
}

pub(super) async fn attach(attachment: ExistingCallAttachment) -> CodexResult<JoinHandle<()>> {
    let ExistingCallAttachment {
        attachment_cancellation,
        client,
        model_client,
        auth_context,
        http_client_factory,
        sideband_base_url,
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
    let (auth, sideband_headers, factory) = model_client
        .realtime_sideband_auth(
            extra_headers,
            &sideband_base_url,
            &auth_context,
            http_client_factory,
        )
        .or_cancel(&attachment_cancellation)
        .await??;
    let client = client.with_http_client_factory(factory).with_auth(auth);
    let transcript_state = RealtimeTranscriptState::default();
    let connection = client
        .connect_existing_call_sideband(
            session_config.clone(),
            &call_id,
            sideband_headers.clone(),
            default_headers(),
            transcript_state,
        )
        .or_cancel(&attachment_cancellation)
        .await?
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
