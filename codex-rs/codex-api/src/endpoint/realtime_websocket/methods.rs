use crate::endpoint::realtime_websocket::methods_common::conversation_handoff_append_message;
use crate::endpoint::realtime_websocket::methods_common::conversation_item_create_message;
use crate::endpoint::realtime_websocket::methods_common::normalized_session_mode;
use crate::endpoint::realtime_websocket::methods_common::session_update_session;
use crate::endpoint::realtime_websocket::protocol::RealtimeAudioFrame;
use crate::endpoint::realtime_websocket::protocol::RealtimeEvent;
use crate::endpoint::realtime_websocket::protocol::RealtimeEventParser;
use crate::endpoint::realtime_websocket::protocol::RealtimeOutboundMessage;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionConfig;
use crate::endpoint::realtime_websocket::protocol::RealtimeSessionMode;
use crate::endpoint::realtime_websocket::protocol::RealtimeTranscriptDelta;
use crate::endpoint::realtime_websocket::protocol::RealtimeTranscriptEntry;
use crate::endpoint::realtime_websocket::protocol::parse_realtime_event;
use crate::error::ApiError;
use crate::provider::Provider;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use bytes::Bytes;
use codex_client::build_reqwest_client_with_custom_ca;
use codex_utils_rustls_provider::ensure_rustls_crypto_provider;
use http::HeaderMap;
use http::HeaderValue;
use interceptor::registry::Registry;
use opus_rs::Application;
use opus_rs::OpusDecoder;
use opus_rs::OpusEncoder;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio::time::Duration;
use tokio::time::timeout;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::trace;
use tracing::warn;
use url::Url;
use webrtc::api::APIBuilder;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MIME_TYPE_OPUS;
use webrtc::api::media_engine::MediaEngine;
use webrtc::data_channel::RTCDataChannel;
use webrtc::data_channel::data_channel_message::DataChannelMessage;
use webrtc::media::Sample;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::rtp_transceiver::rtp_codec::RTPCodecType;
use webrtc::track::track_local::TrackLocal;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;
use webrtc::track::track_remote::TrackRemote;

const REALTIME_CALLS_PATH: &str = "/v1/realtime/calls";
const REALTIME_DATA_CHANNEL_LABEL: &str = "oai-events";
const REALTIME_AUDIO_TRACK_ID: &str = "audio";
const REALTIME_AUDIO_STREAM_ID: &str = "codex-realtime";
const REALTIME_AUDIO_CHANNELS: u16 = 1;
const REALTIME_AUDIO_PACKET_SAMPLES: usize = 480;
const REALTIME_AUDIO_PACKET_DURATION: Duration = Duration::from_millis(20);
const REALTIME_DATA_CHANNEL_OPEN_TIMEOUT: Duration = Duration::from_secs(10);
const REALTIME_AUDIO_TRACK_BINDING_SETTLE_TIME: Duration = Duration::from_millis(250);
const OPUS_MAX_PACKET_BYTES: usize = 4_000;
const OPUS_MAX_DECODED_SAMPLES_PER_CHANNEL: usize = 2_880;

enum RealtimeTransportEvent {
    DataChannelPayload(String),
    AudioOut(RealtimeAudioFrame),
    Closed,
}

struct RealtimeWebRtcTransport {
    peer_connection: Arc<RTCPeerConnection>,
    data_channel: Arc<RTCDataChannel>,
    input_audio_track: Arc<TrackLocalStaticSample>,
    input_audio_encoder: StdMutex<RealtimeInputAudioEncoder>,
}

struct RealtimeInputAudioEncoder {
    opus_encoder: OpusEncoder,
    pending_samples: Vec<i16>,
    encoded_packet: Vec<u8>,
}

pub struct RealtimeWebRtcConnection {
    writer: RealtimeWebRtcWriter,
    events: RealtimeWebRtcEvents,
}

#[derive(Clone)]
pub struct RealtimeWebRtcWriter {
    transport: Arc<RealtimeWebRtcTransport>,
    is_closed: Arc<AtomicBool>,
    event_parser: RealtimeEventParser,
}

#[derive(Clone)]
pub struct RealtimeWebRtcEvents {
    rx_message: Arc<Mutex<mpsc::UnboundedReceiver<Result<RealtimeTransportEvent, ApiError>>>>,
    active_transcript: Arc<Mutex<ActiveTranscriptState>>,
    event_parser: RealtimeEventParser,
    is_closed: Arc<AtomicBool>,
}

#[derive(Default)]
struct ActiveTranscriptState {
    entries: Vec<RealtimeTranscriptEntry>,
}

impl RealtimeWebRtcConnection {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        self.writer.send_audio_frame(frame).await
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        self.writer.send_conversation_item_create(text).await
    }

    pub async fn send_conversation_handoff_append(
        &self,
        handoff_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        self.writer
            .send_conversation_handoff_append(handoff_id, output_text)
            .await
    }

    pub async fn close(&self) -> Result<(), ApiError> {
        self.writer.close().await
    }

    pub async fn next_event(&self) -> Result<Option<RealtimeEvent>, ApiError> {
        self.events.next_event().await
    }

    pub fn writer(&self) -> RealtimeWebRtcWriter {
        self.writer.clone()
    }

    pub fn events(&self) -> RealtimeWebRtcEvents {
        self.events.clone()
    }

    fn new(
        transport: RealtimeWebRtcTransport,
        rx_message: mpsc::UnboundedReceiver<Result<RealtimeTransportEvent, ApiError>>,
        event_parser: RealtimeEventParser,
    ) -> Self {
        let transport = Arc::new(transport);
        let is_closed = Arc::new(AtomicBool::new(false));
        Self {
            writer: RealtimeWebRtcWriter {
                transport: Arc::clone(&transport),
                is_closed: Arc::clone(&is_closed),
                event_parser,
            },
            events: RealtimeWebRtcEvents {
                rx_message: Arc::new(Mutex::new(rx_message)),
                active_transcript: Arc::new(Mutex::new(ActiveTranscriptState::default())),
                event_parser,
                is_closed,
            },
        }
    }
}

impl RealtimeWebRtcWriter {
    pub async fn send_audio_frame(&self, frame: RealtimeAudioFrame) -> Result<(), ApiError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Err(ApiError::Stream(
                "realtime WebRTC connection is closed".to_string(),
            ));
        }

        let opus_samples = {
            let mut encoder = self.transport.input_audio_encoder.lock().map_err(|_| {
                ApiError::Stream("failed to lock realtime input audio encoder".to_string())
            })?;
            encoder.encode_frame(frame)?
        };

        for sample in opus_samples {
            self.transport
                .input_audio_track
                .write_sample(&sample)
                .await
                .map_err(|err| {
                    ApiError::Stream(format!("failed to send realtime audio sample: {err}"))
                })?;
        }
        Ok(())
    }

    pub async fn send_conversation_item_create(&self, text: String) -> Result<(), ApiError> {
        self.send_json(&conversation_item_create_message(self.event_parser, text))
            .await
    }

    pub async fn send_conversation_handoff_append(
        &self,
        handoff_id: String,
        output_text: String,
    ) -> Result<(), ApiError> {
        self.send_json(&conversation_handoff_append_message(
            self.event_parser,
            handoff_id,
            output_text,
        ))
        .await
    }

    pub async fn send_response_create(&self) -> Result<(), ApiError> {
        self.send_json(&RealtimeOutboundMessage::ResponseCreate)
            .await
    }

    pub async fn send_session_update(
        &self,
        instructions: String,
        session_mode: RealtimeSessionMode,
    ) -> Result<(), ApiError> {
        let session_mode = normalized_session_mode(self.event_parser, session_mode);
        let session = session_update_session(self.event_parser, instructions, session_mode);
        self.send_json(&RealtimeOutboundMessage::SessionUpdate { session })
            .await
    }

    pub async fn close(&self) -> Result<(), ApiError> {
        if self.is_closed.swap(true, Ordering::SeqCst) {
            return Ok(());
        }

        if let Err(err) = self.transport.data_channel.close().await {
            warn!("failed to close realtime data channel: {err}");
        }
        self.transport
            .peer_connection
            .close()
            .await
            .map_err(|err| ApiError::Stream(format!("failed to close WebRTC session: {err}")))?;
        Ok(())
    }

    async fn send_json(&self, message: &RealtimeOutboundMessage) -> Result<(), ApiError> {
        let payload = serde_json::to_string(message)
            .map_err(|err| ApiError::Stream(format!("failed to encode realtime request: {err}")))?;
        debug!(?message, "realtime data-channel request");
        self.send_payload(payload).await
    }

    pub async fn send_payload(&self, payload: String) -> Result<(), ApiError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Err(ApiError::Stream(
                "realtime WebRTC connection is closed".to_string(),
            ));
        }

        self.transport
            .data_channel
            .send_text(payload)
            .await
            .map_err(|err| ApiError::Stream(format!("failed to send realtime request: {err}")))?;
        Ok(())
    }
}

impl RealtimeWebRtcEvents {
    pub async fn next_event(&self) -> Result<Option<RealtimeEvent>, ApiError> {
        if self.is_closed.load(Ordering::SeqCst) {
            return Ok(None);
        }

        loop {
            let msg = match self.rx_message.lock().await.recv().await {
                Some(Ok(msg)) => msg,
                Some(Err(err)) => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    error!("realtime WebRTC read failed: {err}");
                    return Err(err);
                }
                None => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    info!("realtime WebRTC event stream ended");
                    return Ok(None);
                }
            };

            match msg {
                RealtimeTransportEvent::DataChannelPayload(payload) => {
                    if let Some(mut event) = parse_realtime_event(&payload, self.event_parser) {
                        self.update_active_transcript(&mut event).await;
                        debug!(?event, "realtime data-channel parsed event");
                        return Ok(Some(event));
                    }
                    debug!("realtime data channel ignored unsupported event");
                }
                RealtimeTransportEvent::AudioOut(mut event) => {
                    let mut wrapped = RealtimeEvent::AudioOut(event.clone());
                    self.update_active_transcript(&mut wrapped).await;
                    if let RealtimeEvent::AudioOut(audio_frame) = wrapped {
                        event = audio_frame;
                    }
                    return Ok(Some(RealtimeEvent::AudioOut(event)));
                }
                RealtimeTransportEvent::Closed => {
                    self.is_closed.store(true, Ordering::SeqCst);
                    info!("realtime WebRTC session closed");
                    return Ok(None);
                }
            }
        }
    }

    async fn update_active_transcript(&self, event: &mut RealtimeEvent) {
        let mut active_transcript = self.active_transcript.lock().await;
        match event {
            RealtimeEvent::InputAudioSpeechStarted(_) => {}
            RealtimeEvent::InputTranscriptDelta(RealtimeTranscriptDelta { delta }) => {
                append_transcript_delta(&mut active_transcript.entries, "user", delta);
            }
            RealtimeEvent::OutputTranscriptDelta(RealtimeTranscriptDelta { delta }) => {
                append_transcript_delta(&mut active_transcript.entries, "assistant", delta);
            }
            RealtimeEvent::HandoffRequested(handoff) => {
                if self.event_parser == RealtimeEventParser::V1 {
                    handoff.active_transcript = std::mem::take(&mut active_transcript.entries);
                }
            }
            RealtimeEvent::SessionUpdated { .. }
            | RealtimeEvent::AudioOut(_)
            | RealtimeEvent::ResponseCancelled(_)
            | RealtimeEvent::ConversationItemAdded(_)
            | RealtimeEvent::ConversationItemDone { .. }
            | RealtimeEvent::Error(_) => {}
        }
    }
}

fn append_transcript_delta(entries: &mut Vec<RealtimeTranscriptEntry>, role: &str, delta: &str) {
    if delta.is_empty() {
        return;
    }

    if let Some(last_entry) = entries.last_mut()
        && last_entry.role == role
    {
        last_entry.text.push_str(delta);
        return;
    }

    entries.push(RealtimeTranscriptEntry {
        role: role.to_string(),
        text: delta.to_string(),
    });
}

pub struct RealtimeWebRtcClient {
    provider: Provider,
}

impl RealtimeWebRtcClient {
    pub fn new(provider: Provider) -> Self {
        Self { provider }
    }

    pub async fn connect(
        &self,
        config: RealtimeSessionConfig,
        extra_headers: HeaderMap,
        default_headers: HeaderMap,
    ) -> Result<RealtimeWebRtcConnection, ApiError> {
        ensure_rustls_crypto_provider();
        let calls_url = calls_url_from_api_url(
            self.provider.base_url.as_str(),
            self.provider.query_params.as_ref(),
        )?;

        let headers = merge_request_headers(
            &self.provider.headers,
            with_session_id_header(extra_headers, config.session_id.as_deref())?,
            default_headers,
        );

        info!("connecting realtime WebRTC session: {calls_url}");
        let (transport, rx_message) = connect_webrtc_transport(
            calls_url,
            headers,
            config.event_parser,
            config.model.clone(),
            config.instructions.clone(),
            config.session_mode,
        )
        .await?;
        let connection = RealtimeWebRtcConnection::new(transport, rx_message, config.event_parser);
        debug!(
            session_id = config.session_id.as_deref().unwrap_or("<none>"),
            "realtime WebRTC sending session.update"
        );
        connection
            .writer
            .send_session_update(config.instructions, config.session_mode)
            .await?;
        Ok(connection)
    }
}

fn merge_request_headers(
    provider_headers: &HeaderMap,
    extra_headers: HeaderMap,
    default_headers: HeaderMap,
) -> HeaderMap {
    let mut headers = provider_headers.clone();
    headers.extend(extra_headers);
    for (name, value) in &default_headers {
        if let http::header::Entry::Vacant(entry) = headers.entry(name) {
            entry.insert(value.clone());
        }
    }
    headers
}

fn with_session_id_header(
    mut headers: HeaderMap,
    session_id: Option<&str>,
) -> Result<HeaderMap, ApiError> {
    let Some(session_id) = session_id else {
        return Ok(headers);
    };
    headers.insert(
        "x-session-id",
        HeaderValue::from_str(session_id).map_err(|err| {
            ApiError::Stream(format!("invalid realtime session id header: {err}"))
        })?,
    );
    Ok(headers)
}

fn calls_url_from_api_url(
    api_url: &str,
    query_params: Option<&HashMap<String, String>>,
) -> Result<Url, ApiError> {
    let mut url = Url::parse(api_url)
        .map_err(|err| ApiError::Stream(format!("failed to parse realtime api_url: {err}")))?;

    normalize_realtime_calls_path(&mut url);

    match url.scheme() {
        "http" | "https" => {}
        "ws" => {
            let _ = url.set_scheme("http");
        }
        "wss" => {
            let _ = url.set_scheme("https");
        }
        scheme => {
            return Err(ApiError::Stream(format!(
                "unsupported realtime api_url scheme: {scheme}"
            )));
        }
    }

    if let Some(query_params) = query_params {
        let mut query = url.query_pairs_mut();
        for (key, value) in query_params {
            query.append_pair(key, value);
        }
    }

    Ok(url)
}

fn normalize_realtime_calls_path(url: &mut Url) {
    let path = url.path().to_string();
    if path.is_empty() || path == "/" {
        url.set_path(REALTIME_CALLS_PATH);
        return;
    }

    if path.ends_with("/realtime/calls") {
        return;
    }

    if path.ends_with("/realtime/calls/") {
        url.set_path(path.trim_end_matches('/'));
        return;
    }

    if path.ends_with("/realtime") {
        url.set_path(&format!("{path}/calls"));
        return;
    }

    if path.ends_with("/realtime/") {
        url.set_path(&format!("{}/calls", path.trim_end_matches('/')));
        return;
    }

    if path.ends_with("/v1") {
        url.set_path(&format!("{path}/realtime/calls"));
        return;
    }

    if path.ends_with("/v1/") {
        url.set_path(&format!("{path}realtime/calls"));
    }
}

async fn connect_webrtc_transport(
    calls_url: Url,
    headers: HeaderMap,
    event_parser: RealtimeEventParser,
    model: Option<String>,
    instructions: String,
    session_mode: RealtimeSessionMode,
) -> Result<
    (
        RealtimeWebRtcTransport,
        mpsc::UnboundedReceiver<Result<RealtimeTransportEvent, ApiError>>,
    ),
    ApiError,
> {
    let peer_connection = create_peer_connection().await?;
    let input_audio_track = Arc::new(TrackLocalStaticSample::new(
        RTCRtpCodecCapability {
            mime_type: MIME_TYPE_OPUS.to_string(),
            ..Default::default()
        },
        REALTIME_AUDIO_TRACK_ID.to_string(),
        REALTIME_AUDIO_STREAM_ID.to_string(),
    ));
    let rtp_sender = peer_connection
        .add_track(Arc::clone(&input_audio_track) as Arc<dyn TrackLocal + Send + Sync>)
        .await
        .map_err(|err| ApiError::Stream(format!("failed to add realtime audio track: {err}")))?;
    tokio::spawn(async move {
        let mut rtcp_buf = vec![0u8; 1500];
        while let Ok((_, _)) = rtp_sender.read(&mut rtcp_buf).await {}
    });

    let (tx_message, rx_message) =
        mpsc::unbounded_channel::<Result<RealtimeTransportEvent, ApiError>>();
    let (tx_data_channel_open, rx_data_channel_open) = oneshot::channel::<()>();
    let data_channel = peer_connection
        .create_data_channel(REALTIME_DATA_CHANNEL_LABEL, None)
        .await
        .map_err(|err| {
            ApiError::Stream(format!("failed to create realtime data channel: {err}"))
        })?;

    register_data_channel_handlers(&data_channel, tx_message.clone(), tx_data_channel_open);
    register_peer_connection_handlers(&peer_connection, tx_message.clone());
    register_remote_audio_handler(&peer_connection, tx_message);

    let mut gather_complete = peer_connection.gathering_complete_promise().await;
    let offer = peer_connection
        .create_offer(None)
        .await
        .map_err(|err| ApiError::Stream(format!("failed to create WebRTC offer: {err}")))?;
    peer_connection
        .set_local_description(offer)
        .await
        .map_err(|err| {
            ApiError::Stream(format!("failed to set local WebRTC description: {err}"))
        })?;
    let _ = gather_complete.recv().await;

    let local_description = peer_connection.local_description().await.ok_or_else(|| {
        ApiError::Stream("WebRTC local description was not available".to_string())
    })?;
    let answer = fetch_realtime_answer(
        calls_url.clone(),
        headers,
        local_description.sdp,
        event_parser,
        model,
        instructions,
        session_mode,
    )
    .await?;
    let remote_description = RTCSessionDescription::answer(answer)
        .map_err(|err| ApiError::Stream(format!("invalid WebRTC answer SDP: {err}")))?;
    peer_connection
        .set_remote_description(remote_description)
        .await
        .map_err(|err| {
            ApiError::Stream(format!("failed to set remote WebRTC description: {err}"))
        })?;

    timeout(REALTIME_DATA_CHANNEL_OPEN_TIMEOUT, rx_data_channel_open)
        .await
        .map_err(|_| {
            ApiError::Stream(
                "timed out waiting for realtime WebRTC data channel to open".to_string(),
            )
        })?
        .map_err(|_| ApiError::Stream("realtime WebRTC data channel did not open".to_string()))?;

    // `TrackLocalStaticSample::write_sample()` silently drops samples until the RTP sender has
    // finished binding after negotiation. Give webrtc-rs a short grace period so the first user
    // audio frame is not discarded during startup.
    tokio::time::sleep(REALTIME_AUDIO_TRACK_BINDING_SETTLE_TIME).await;

    info!(
        calls_url = %calls_url,
        "realtime WebRTC session connected"
    );

    let opus_encoder = new_realtime_opus_encoder(Application::Voip)?;

    Ok((
        RealtimeWebRtcTransport {
            peer_connection,
            data_channel,
            input_audio_track,
            input_audio_encoder: StdMutex::new(RealtimeInputAudioEncoder {
                opus_encoder,
                pending_samples: Vec::new(),
                encoded_packet: vec![0; OPUS_MAX_PACKET_BYTES],
            }),
        },
        rx_message,
    ))
}

async fn create_peer_connection() -> Result<Arc<RTCPeerConnection>, ApiError> {
    let mut media_engine = MediaEngine::default();
    media_engine
        .register_default_codecs()
        .map_err(|err| ApiError::Stream(format!("failed to register WebRTC codecs: {err}")))?;
    let registry =
        register_default_interceptors(Registry::new(), &mut media_engine).map_err(|err| {
            ApiError::Stream(format!("failed to register WebRTC interceptors: {err}"))
        })?;
    let api = APIBuilder::new()
        .with_media_engine(media_engine)
        .with_interceptor_registry(registry)
        .build();
    api.new_peer_connection(RTCConfiguration::default())
        .await
        .map(Arc::new)
        .map_err(|err| ApiError::Stream(format!("failed to create WebRTC peer connection: {err}")))
}

fn register_data_channel_handlers(
    data_channel: &Arc<RTCDataChannel>,
    tx_message: mpsc::UnboundedSender<Result<RealtimeTransportEvent, ApiError>>,
    tx_data_channel_open: oneshot::Sender<()>,
) {
    let tx_open = StdMutex::new(Some(tx_data_channel_open));
    data_channel.on_open(Box::new(move || {
        if let Ok(mut tx_open) = tx_open.lock()
            && let Some(tx_open) = tx_open.take()
        {
            let _ = tx_open.send(());
        }
        Box::pin(async {})
    }));

    let tx_data_channel_message = tx_message.clone();
    data_channel.on_message(Box::new(move |message: DataChannelMessage| {
        let tx_data_channel_message = tx_data_channel_message.clone();
        Box::pin(async move {
            if !message.is_string {
                let _ = tx_data_channel_message.send(Err(ApiError::Stream(
                    "unexpected binary realtime data-channel message".to_string(),
                )));
                return;
            }
            let payload = match String::from_utf8(message.data.to_vec()) {
                Ok(payload) => payload,
                Err(err) => {
                    let _ = tx_data_channel_message.send(Err(ApiError::Stream(format!(
                        "invalid realtime data-channel UTF-8 payload: {err}"
                    ))));
                    return;
                }
            };
            trace!(
                payload_len = payload.len(),
                "realtime data-channel received text event"
            );
            let _ = tx_data_channel_message
                .send(Ok(RealtimeTransportEvent::DataChannelPayload(payload)));
        })
    }));

    data_channel.on_close(Box::new(move || {
        let tx_message = tx_message.clone();
        Box::pin(async move {
            let _ = tx_message.send(Ok(RealtimeTransportEvent::Closed));
        })
    }));
}

fn register_peer_connection_handlers(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_message: mpsc::UnboundedSender<Result<RealtimeTransportEvent, ApiError>>,
) {
    peer_connection.on_peer_connection_state_change(Box::new(move |state| {
        let tx_message = tx_message.clone();
        Box::pin(async move {
            match state {
                RTCPeerConnectionState::Closed => {
                    let _ = tx_message.send(Ok(RealtimeTransportEvent::Closed));
                }
                RTCPeerConnectionState::Failed => {
                    let _ = tx_message.send(Err(ApiError::Stream(
                        "realtime WebRTC peer connection failed".to_string(),
                    )));
                }
                RTCPeerConnectionState::New
                | RTCPeerConnectionState::Connecting
                | RTCPeerConnectionState::Connected
                | RTCPeerConnectionState::Disconnected
                | RTCPeerConnectionState::Unspecified => {}
            }
        })
    }));
}

fn register_remote_audio_handler(
    peer_connection: &Arc<RTCPeerConnection>,
    tx_message: mpsc::UnboundedSender<Result<RealtimeTransportEvent, ApiError>>,
) {
    peer_connection.on_track(Box::new(move |track, _, _| {
        let tx_message = tx_message.clone();
        Box::pin(async move {
            if track.kind() != RTPCodecType::Audio {
                return;
            }
            pump_remote_audio_track(track, tx_message).await;
        })
    }));
}

async fn pump_remote_audio_track(
    track: Arc<TrackRemote>,
    tx_message: mpsc::UnboundedSender<Result<RealtimeTransportEvent, ApiError>>,
) {
    let mut decoder = match new_realtime_opus_decoder() {
        Ok(decoder) => decoder,
        Err(err) => {
            let _ = tx_message.send(Err(ApiError::Stream(format!(
                "failed to initialize realtime Opus decoder: {err}"
            ))));
            return;
        }
    };
    let mut decoded = vec![0.0f32; OPUS_MAX_DECODED_SAMPLES_PER_CHANNEL];

    loop {
        let packet = match track.read_rtp().await {
            Ok((packet, _)) => packet,
            Err(err) => {
                let _ = tx_message.send(Err(ApiError::Stream(format!(
                    "failed to read realtime WebRTC audio packet: {err}"
                ))));
                return;
            }
        };
        if packet.payload.is_empty() {
            continue;
        }

        let samples_per_channel = match decoder.decode(
            &packet.payload,
            OPUS_MAX_DECODED_SAMPLES_PER_CHANNEL,
            &mut decoded,
        ) {
            Ok(samples_per_channel) => samples_per_channel,
            Err(err) => {
                warn!(
                    %err,
                    payload_len = packet.payload.len(),
                    "failed to decode realtime Opus packet"
                );
                let _ = tx_message.send(Err(ApiError::Stream(format!(
                    "failed to decode realtime Opus packet: {err}"
                ))));
                return;
            }
        };
        if samples_per_channel == 0 {
            continue;
        }

        let mut pcm_bytes = Vec::with_capacity(samples_per_channel * 2);
        for sample in &decoded[..samples_per_channel] {
            pcm_bytes.extend_from_slice(&f32_to_i16(*sample).to_le_bytes());
        }
        let _ = tx_message.send(Ok(RealtimeTransportEvent::AudioOut(RealtimeAudioFrame {
            data: BASE64_STANDARD.encode(pcm_bytes),
            sample_rate: 24_000,
            num_channels: REALTIME_AUDIO_CHANNELS,
            samples_per_channel: Some(samples_per_channel as u32),
            item_id: None,
        })));
    }
}

async fn fetch_realtime_answer(
    calls_url: Url,
    headers: HeaderMap,
    offer_sdp: String,
    event_parser: RealtimeEventParser,
    model: Option<String>,
    instructions: String,
    session_mode: RealtimeSessionMode,
) -> Result<String, ApiError> {
    let client = build_reqwest_client_with_custom_ca(reqwest::Client::builder())
        .map_err(|err| ApiError::Stream(format!("failed to build realtime HTTP client: {err}")))?;
    let session_mode = normalized_session_mode(event_parser, session_mode);
    let session = session_update_session(event_parser, instructions, session_mode);
    let mut session_json = serde_json::to_value(&session)
        .map_err(|err| ApiError::Stream(format!("failed to encode realtime session: {err}")))?;
    if let Some(model) = model
        && let Some(session_json) = session_json.as_object_mut()
    {
        session_json.insert("model".to_string(), serde_json::Value::String(model));
    }
    let session_payload = serde_json::to_string(&session_json)
        .map_err(|err| ApiError::Stream(format!("failed to encode realtime session: {err}")))?;
    let form = reqwest::multipart::Form::new()
        .text("sdp", offer_sdp)
        .text("session", session_payload);
    let response = client
        .post(calls_url)
        .headers(headers)
        .multipart(form)
        .send()
        .await
        .map_err(|err| ApiError::Stream(format!("failed to create realtime WebRTC call: {err}")))?;
    let status = response.status();
    let body = response.text().await.map_err(|err| {
        ApiError::Stream(format!("failed to read realtime WebRTC answer SDP: {err}"))
    })?;
    if !status.is_success() {
        return Err(ApiError::Stream(format!(
            "failed to create realtime WebRTC call: status={status} body={body}"
        )));
    }
    Ok(body)
}

impl RealtimeInputAudioEncoder {
    fn encode_frame(&mut self, frame: RealtimeAudioFrame) -> Result<Vec<Sample>, ApiError> {
        if frame.sample_rate != 24_000 || frame.num_channels != REALTIME_AUDIO_CHANNELS {
            return Err(ApiError::Stream(format!(
                "realtime WebRTC audio input must be {} Hz mono PCM; got {} Hz / {} channels",
                24_000, frame.sample_rate, frame.num_channels
            )));
        }

        let pcm_bytes = BASE64_STANDARD.decode(frame.data).map_err(|err| {
            ApiError::Stream(format!("failed to decode realtime input audio: {err}"))
        })?;
        if pcm_bytes.len() % 2 != 0 {
            return Err(ApiError::Stream(
                "realtime input audio had odd byte length".to_string(),
            ));
        }
        for sample in pcm_bytes.chunks_exact(2) {
            self.pending_samples
                .push(i16::from_le_bytes([sample[0], sample[1]]));
        }

        let mut samples = Vec::new();
        while self.pending_samples.len() >= REALTIME_AUDIO_PACKET_SAMPLES {
            let frame: Vec<f32> = self.pending_samples[..REALTIME_AUDIO_PACKET_SAMPLES]
                .iter()
                .copied()
                .map(i16_to_f32)
                .collect();
            let encoded_len = self
                .opus_encoder
                .encode(
                    &frame,
                    REALTIME_AUDIO_PACKET_SAMPLES,
                    &mut self.encoded_packet,
                )
                .map_err(|err| {
                    warn!(
                        %err,
                        pending_samples = self.pending_samples.len(),
                        "failed to encode realtime input Opus"
                    );
                    ApiError::Stream(format!("failed to encode realtime input Opus: {err}"))
                })?;
            self.pending_samples.drain(..REALTIME_AUDIO_PACKET_SAMPLES);
            samples.push(Sample {
                data: Bytes::copy_from_slice(&self.encoded_packet[..encoded_len]),
                duration: REALTIME_AUDIO_PACKET_DURATION,
                ..Default::default()
            });
        }

        Ok(samples)
    }
}

fn new_realtime_opus_encoder(application: Application) -> Result<OpusEncoder, ApiError> {
    let encoder = OpusEncoder::new(24_000, usize::from(REALTIME_AUDIO_CHANNELS), application)
        .map_err(|err| {
            warn!(
                %err,
                sample_rate = 24_000,
                channels = REALTIME_AUDIO_CHANNELS,
                "failed to initialize realtime Opus encoder"
            );
            ApiError::Stream(format!("failed to initialize realtime Opus encoder: {err}"))
        })?;
    debug!(
        sample_rate = 24_000,
        channels = REALTIME_AUDIO_CHANNELS,
        "initialized realtime Opus encoder"
    );
    Ok(encoder)
}

fn new_realtime_opus_decoder() -> Result<OpusDecoder, ApiError> {
    let decoder =
        OpusDecoder::new(24_000, usize::from(REALTIME_AUDIO_CHANNELS)).map_err(|err| {
            warn!(
                %err,
                sample_rate = 24_000,
                channels = REALTIME_AUDIO_CHANNELS,
                "failed to initialize realtime Opus decoder"
            );
            ApiError::Stream(format!("failed to initialize realtime Opus decoder: {err}"))
        })?;
    debug!(
        sample_rate = 24_000,
        channels = REALTIME_AUDIO_CHANNELS,
        "initialized realtime Opus decoder"
    );
    Ok(decoder)
}

fn i16_to_f32(sample: i16) -> f32 {
    (sample as f32) / (i16::MAX as f32)
}

fn f32_to_i16(sample: f32) -> i16 {
    (sample.clamp(-1.0, 1.0) * i16::MAX as f32) as i16
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;
    use std::collections::HashMap;

    #[test]
    fn calls_url_from_api_url_normalizes_http_root() {
        let query_params = HashMap::from([("model".to_string(), "gpt-realtime".to_string())]);
        let calls_url =
            calls_url_from_api_url("http://example.com", Some(&query_params)).expect("calls url");

        assert_eq!(
            calls_url.as_str(),
            "http://example.com/v1/realtime/calls?model=gpt-realtime"
        );
    }

    #[test]
    fn calls_url_from_api_url_preserves_v1_realtime_path_and_query() {
        let query_params = HashMap::from([("model".to_string(), "gpt-realtime".to_string())]);
        let calls_url =
            calls_url_from_api_url("wss://example.com/v1/realtime?foo=bar", Some(&query_params))
                .expect("calls url");

        assert_eq!(
            calls_url.as_str(),
            "https://example.com/v1/realtime/calls?foo=bar&model=gpt-realtime"
        );
    }

    #[test]
    fn parse_session_updated_event() {
        let payload = json!({
            "type": "session.updated",
            "session": {"id": "sess_123", "instructions": "backend prompt"}
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeEventParser::V1),
            Some(RealtimeEvent::SessionUpdated {
                session_id: "sess_123".to_string(),
                instructions: Some("backend prompt".to_string()),
            })
        );
    }

    #[test]
    fn parse_audio_delta_event() {
        let payload = json!({
            "type": "conversation.output_audio.delta",
            "delta": "AAA=",
            "sample_rate": 24_000,
            "channels": 1,
            "samples_per_channel": 480,
            "item_id": "item_123"
        })
        .to_string();

        assert_eq!(
            parse_realtime_event(payload.as_str(), RealtimeEventParser::V1),
            Some(RealtimeEvent::AudioOut(RealtimeAudioFrame {
                data: "AAA=".to_string(),
                sample_rate: 24_000,
                num_channels: 1,
                samples_per_channel: Some(480),
                item_id: Some("item_123".to_string()),
            }))
        );
    }

    #[test]
    fn input_audio_encoder_rejects_non_mono_24khz_audio() {
        let mut encoder = RealtimeInputAudioEncoder {
            opus_encoder: new_realtime_opus_encoder(Application::Audio).expect("encoder"),
            pending_samples: Vec::new(),
            encoded_packet: vec![0u8; OPUS_MAX_PACKET_BYTES],
        };

        let err = encoder
            .encode_frame(RealtimeAudioFrame {
                data: "AAAA".to_string(),
                sample_rate: 48_000,
                num_channels: 2,
                samples_per_channel: Some(1),
                item_id: None,
            })
            .expect_err("reject unsupported audio");

        let ApiError::Stream(message) = err else {
            panic!("expected stream error, got {err:?}");
        };

        assert_eq!(
            message,
            "realtime WebRTC audio input must be 24000 Hz mono PCM; got 48000 Hz / 2 channels"
        );
    }
}
