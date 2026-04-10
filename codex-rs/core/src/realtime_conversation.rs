use crate::client::ModelClient;
use crate::codex::Session;
use crate::realtime_context::build_realtime_startup_context;
use crate::realtime_prompt::prepare_realtime_backend_prompt;
use anyhow::Context;
use async_channel::Receiver;
use async_channel::RecvError;
use async_channel::Sender;
use async_channel::TrySendError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::ApiError;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::RealtimeWebsocketClient;
use codex_api::RealtimeWebsocketEvents;
use codex_api::RealtimeWebsocketWriter;
use codex_api::map_api_error;
use codex_app_server_protocol::AuthMode;
use codex_config::config_toml::RealtimeWsMode;
use codex_config::config_toml::RealtimeWsVersion;
use codex_login::CodexAuth;
use codex_login::default_client::default_headers;
use codex_login::read_openai_api_key_from_env;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationSdpEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const USER_TEXT_IN_QUEUE_CAPACITY: usize = 64;
const HANDOFF_OUT_QUEUE_CAPACITY: usize = 64;
const OUTPUT_EVENTS_QUEUE_CAPACITY: usize = 256;
const REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET: usize = 5_000;
const DEFAULT_REALTIME_MODEL: &str = "gpt-realtime-1.5";
const REALTIME_V2_PROGRESS_UPDATE_SUFFIX: &str =
    "\n\nUpdate from background agent (task hasn't finished yet):";
const REALTIME_V2_STEER_ACKNOWLEDGEMENT: &str =
    "This was sent to steer the previous background agent task.";
const REALTIME_ACTIVE_RESPONSE_ERROR_PREFIX: &str =
    "Conversation already has an active response in progress:";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RealtimeConversationEnd {
    Requested,
    TransportClosed,
    Error,
}

enum RealtimeFanoutTaskStop {
    Abort,
    Detach,
}

pub(crate) struct RealtimeConversationManager {
    state: Mutex<Option<ConversationState>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RealtimeSessionKind {
    V1,
    V2,
}

#[derive(Clone, Debug)]
struct RealtimeHandoffState {
    output_tx: Sender<HandoffOutput>,
    active_handoff: Arc<Mutex<Option<String>>>,
    last_output_text: Arc<Mutex<Option<String>>>,
    session_kind: RealtimeSessionKind,
}

#[derive(Debug, PartialEq, Eq)]
enum HandoffOutput {
    ProgressUpdate {
        handoff_id: String,
        output_text: String,
    },
    FinalUpdate {
        handoff_id: String,
        output_text: String,
    },
}

#[derive(Debug, PartialEq, Eq)]
struct OutputAudioState {
    item_id: String,
    audio_end_ms: u32,
}

#[derive(Default)]
struct RealtimeResponseCreateQueue {
    active_default_response: bool,
    pending_create: bool,
}

impl RealtimeResponseCreateQueue {
    async fn request_create(
        &mut self,
        writer: &RealtimeWebsocketWriter,
        events_tx: &Sender<RealtimeEvent>,
        reason: &str,
    ) -> anyhow::Result<()> {
        if self.active_default_response {
            self.pending_create = true;
            return Ok(());
        }
        self.send_create_now(writer, events_tx, reason).await
    }

    fn mark_started(&mut self) {
        self.active_default_response = true;
    }

    async fn mark_finished(
        &mut self,
        writer: &RealtimeWebsocketWriter,
        events_tx: &Sender<RealtimeEvent>,
        reason: &str,
    ) -> anyhow::Result<()> {
        self.active_default_response = false;
        if !self.pending_create {
            return Ok(());
        }
        self.pending_create = false;
        self.send_create_now(writer, events_tx, reason).await
    }

    async fn send_create_now(
        &mut self,
        writer: &RealtimeWebsocketWriter,
        events_tx: &Sender<RealtimeEvent>,
        reason: &str,
    ) -> anyhow::Result<()> {
        if let Err(err) = writer.send_response_create().await {
            let mapped_error = map_api_error(err);
            let error_message = mapped_error.to_string();
            if error_message.starts_with(REALTIME_ACTIVE_RESPONSE_ERROR_PREFIX) {
                warn!("realtime response.create raced an active response; deferring");
                self.active_default_response = true;
                self.pending_create = true;
                return Ok(());
            }
            warn!("failed to send {reason} response.create: {mapped_error}");
            let _ = events_tx.send(RealtimeEvent::Error(error_message)).await;
            return Err(mapped_error.into());
        }
        self.active_default_response = true;
        Ok(())
    }
}

struct RealtimeInputTask {
    writer: RealtimeWebsocketWriter,
    events: RealtimeWebsocketEvents,
    user_text_rx: Receiver<String>,
    handoff_output_rx: Receiver<HandoffOutput>,
    audio_rx: Receiver<RealtimeAudioFrame>,
    events_tx: Sender<RealtimeEvent>,
    handoff_state: RealtimeHandoffState,
    session_kind: RealtimeSessionKind,
    event_parser: RealtimeEventParser,
}

impl RealtimeHandoffState {
    fn new(output_tx: Sender<HandoffOutput>, session_kind: RealtimeSessionKind) -> Self {
        Self {
            output_tx,
            active_handoff: Arc::new(Mutex::new(None)),
            last_output_text: Arc::new(Mutex::new(None)),
            session_kind,
        }
    }
}

#[allow(dead_code)]
struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    user_text_tx: Sender<String>,
    writer: RealtimeWebsocketWriter,
    handoff: RealtimeHandoffState,
    input_task: JoinHandle<()>,
    fanout_task: Option<JoinHandle<()>>,
    realtime_active: Arc<AtomicBool>,
}

struct RealtimeStart {
    api_provider: ApiProvider,
    extra_headers: Option<HeaderMap>,
    session_config: RealtimeSessionConfig,
    model_client: ModelClient,
    sdp: Option<String>,
}

struct RealtimeStartOutput {
    realtime_active: Arc<AtomicBool>,
    events_rx: Receiver<RealtimeEvent>,
    sdp: Option<String>,
}

#[allow(dead_code)]
impl RealtimeConversationManager {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    pub(crate) async fn running_state(&self) -> Option<()> {
        let state = self.state.lock().await;
        state
            .as_ref()
            .and_then(|state| state.realtime_active.load(Ordering::Relaxed).then_some(()))
    }

    async fn start(&self, start: RealtimeStart) -> CodexResult<RealtimeStartOutput> {
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            stop_conversation_state(state, RealtimeFanoutTaskStop::Abort).await;
        }

        self.start_inner(start).await
    }

    async fn start_inner(&self, start: RealtimeStart) -> CodexResult<RealtimeStartOutput> {
        let RealtimeStart {
            api_provider,
            extra_headers,
            session_config,
            model_client,
            sdp,
        } = start;
        let event_parser = session_config.event_parser;
        let session_kind = match event_parser {
            RealtimeEventParser::V1 => RealtimeSessionKind::V1,
            RealtimeEventParser::RealtimeV2 => RealtimeSessionKind::V2,
        };

        let client = RealtimeWebsocketClient::new(api_provider);
        let (connection, sdp) = if let Some(sdp) = sdp {
            let call = model_client
                .create_realtime_call_with_headers(
                    sdp,
                    session_config.clone(),
                    extra_headers.unwrap_or_default(),
                )
                .await?;
            let connection = client
                .connect_webrtc_sideband(
                    session_config,
                    &call.call_id,
                    call.sideband_headers,
                    default_headers(),
                )
                .await
                .map_err(map_api_error)?;
            (connection, Some(call.sdp))
        } else {
            let connection = client
                .connect(
                    session_config,
                    extra_headers.unwrap_or_default(),
                    default_headers(),
                )
                .await
                .map_err(map_api_error)?;
            (connection, None)
        };

        let writer = connection.writer();
        let events = connection.events();
        let (audio_tx, audio_rx) =
            async_channel::bounded::<RealtimeAudioFrame>(AUDIO_IN_QUEUE_CAPACITY);
        let (user_text_tx, user_text_rx) =
            async_channel::bounded::<String>(USER_TEXT_IN_QUEUE_CAPACITY);
        let (handoff_output_tx, handoff_output_rx) =
            async_channel::bounded::<HandoffOutput>(HANDOFF_OUT_QUEUE_CAPACITY);
        let (events_tx, events_rx) =
            async_channel::bounded::<RealtimeEvent>(OUTPUT_EVENTS_QUEUE_CAPACITY);

        let realtime_active = Arc::new(AtomicBool::new(true));
        let handoff = RealtimeHandoffState::new(handoff_output_tx, session_kind);
        let task = spawn_realtime_input_task(RealtimeInputTask {
            writer: writer.clone(),
            events,
            user_text_rx,
            handoff_output_rx,
            audio_rx,
            events_tx,
            handoff_state: handoff.clone(),
            session_kind,
            event_parser,
        });

        let mut guard = self.state.lock().await;
        *guard = Some(ConversationState {
            audio_tx,
            user_text_tx,
            writer,
            handoff,
            input_task: task,
            fanout_task: None,
            realtime_active: Arc::clone(&realtime_active),
        });
        Ok(RealtimeStartOutput {
            realtime_active,
            events_rx,
            sdp,
        })
    }

    pub(crate) async fn register_fanout_task(
        &self,
        realtime_active: &Arc<AtomicBool>,
        fanout_task: JoinHandle<()>,
    ) {
        let mut fanout_task = Some(fanout_task);
        {
            let mut guard = self.state.lock().await;
            if let Some(state) = guard.as_mut()
                && Arc::ptr_eq(&state.realtime_active, realtime_active)
            {
                state.fanout_task = fanout_task.take();
            }
        }

        if let Some(fanout_task) = fanout_task {
            fanout_task.abort();
            let _ = fanout_task.await;
        }
    }

    pub(crate) async fn finish_if_active(&self, realtime_active: &Arc<AtomicBool>) {
        let state = {
            let mut guard = self.state.lock().await;
            match guard.as_ref() {
                Some(state) if Arc::ptr_eq(&state.realtime_active, realtime_active) => guard.take(),
                _ => None,
            }
        };

        if let Some(state) = state {
            stop_conversation_state(state, RealtimeFanoutTaskStop::Detach).await;
        }
    }

    pub(crate) async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
        let sender = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.audio_tx.clone())
        };

        let Some(sender) = sender else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };

        match sender.try_send(frame) {
            Ok(()) => Ok(()),
            Err(TrySendError::Full(_)) => {
                warn!("dropping input audio frame due to full queue");
                Ok(())
            }
            Err(TrySendError::Closed(_)) => Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            )),
        }
    }

    pub(crate) async fn text_in(&self, text: String) -> CodexResult<()> {
        let sender = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.user_text_tx.clone())
        };

        let Some(sender) = sender else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };

        sender
            .send(text)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub(crate) async fn handoff_out(&self, output_text: String) -> CodexResult<()> {
        let handoff = {
            let guard = self.state.lock().await;
            let Some(state) = guard.as_ref() else {
                return Err(CodexErr::InvalidRequest(
                    "conversation is not running".to_string(),
                ));
            };
            state.handoff.clone()
        };

        let Some(handoff_id) = handoff.active_handoff.lock().await.clone() else {
            return Ok(());
        };

        *handoff.last_output_text.lock().await = Some(output_text.clone());
        handoff
            .output_tx
            .send(HandoffOutput::ProgressUpdate {
                handoff_id,
                output_text,
            })
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub(crate) async fn handoff_complete(&self) -> CodexResult<()> {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        let Some(handoff) = handoff else {
            return Ok(());
        };
        match handoff.session_kind {
            RealtimeSessionKind::V1 => return Ok(()),
            RealtimeSessionKind::V2 => {}
        }

        let Some(handoff_id) = handoff.active_handoff.lock().await.clone() else {
            return Ok(());
        };
        let Some(output_text) = handoff.last_output_text.lock().await.clone() else {
            return Ok(());
        };

        handoff
            .output_tx
            .send(HandoffOutput::FinalUpdate {
                handoff_id,
                output_text,
            })
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))
    }

    pub(crate) async fn active_handoff_id(&self) -> Option<String> {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        }?;
        handoff.active_handoff.lock().await.clone()
    }

    pub(crate) async fn clear_active_handoff(&self) {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        if let Some(handoff) = handoff {
            *handoff.active_handoff.lock().await = None;
            *handoff.last_output_text.lock().await = None;
        }
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            stop_conversation_state(state, RealtimeFanoutTaskStop::Abort).await;
        }
        Ok(())
    }
}

async fn stop_conversation_state(
    mut state: ConversationState,
    fanout_task_stop: RealtimeFanoutTaskStop,
) {
    state.realtime_active.store(false, Ordering::Relaxed);
    state.input_task.abort();
    let _ = state.input_task.await;

    if let Some(fanout_task) = state.fanout_task.take() {
        match fanout_task_stop {
            RealtimeFanoutTaskStop::Abort => {
                fanout_task.abort();
                let _ = fanout_task.await;
            }
            RealtimeFanoutTaskStop::Detach => {}
        }
    }
}

pub(crate) async fn handle_start(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationStartParams,
) -> CodexResult<()> {
    let prepared_start = match prepare_realtime_start(sess, params).await {
        Ok(prepared_start) => prepared_start,
        Err(err) => {
            error!("failed to prepare realtime conversation: {err}");
            let message = err.to_string();
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                    payload: RealtimeEvent::Error(message),
                }),
            })
            .await;
            return Ok(());
        }
    };

    if let Err(err) = handle_start_inner(sess, &sub_id, prepared_start).await {
        error!("failed to start realtime conversation: {err}");
        let message = err.to_string();
        sess.send_event_raw(Event {
            id: sub_id.clone(),
            msg: EventMsg::RealtimeConversationRealtime(RealtimeConversationRealtimeEvent {
                payload: RealtimeEvent::Error(message),
            }),
        })
        .await;
    }
    Ok(())
}

struct PreparedRealtimeConversationStart {
    api_provider: ApiProvider,
    extra_headers: Option<HeaderMap>,
    requested_session_id: Option<String>,
    version: RealtimeWsVersion,
    session_config: RealtimeSessionConfig,
    transport: ConversationStartTransport,
}

async fn prepare_realtime_start(
    sess: &Arc<Session>,
    params: ConversationStartParams,
) -> CodexResult<PreparedRealtimeConversationStart> {
    let provider = sess.provider().await;
    let auth_manager = sess
        .services
        .model_client
        .auth_manager()
        .unwrap_or_else(|| Arc::clone(&sess.services.auth_manager));
    let auth = auth_manager.auth().await;
    let config = sess.get_config().await;
    let transport = params
        .transport
        .unwrap_or(ConversationStartTransport::Websocket);
    let mut api_provider = provider.to_api_provider(Some(AuthMode::ApiKey))?;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }
    let version = config.realtime.version;
    let session_config =
        build_realtime_session_config(sess, params.prompt, params.session_id, params.voice).await?;
    let requested_session_id = session_config.session_id.clone();
    let extra_headers = match transport {
        ConversationStartTransport::Websocket => {
            let realtime_api_key = realtime_api_key(auth.as_ref(), &provider)?;
            realtime_request_headers(
                requested_session_id.as_deref(),
                Some(realtime_api_key.as_str()),
            )?
        }
        ConversationStartTransport::Webrtc { .. } => {
            realtime_request_headers(requested_session_id.as_deref(), /*api_key*/ None)?
        }
    };
    Ok(PreparedRealtimeConversationStart {
        api_provider,
        extra_headers,
        requested_session_id,
        version,
        session_config,
        transport,
    })
}

pub(crate) async fn build_realtime_session_config(
    sess: &Arc<Session>,
    prompt: Option<Option<String>>,
    session_id: Option<String>,
    voice: Option<RealtimeVoice>,
) -> CodexResult<RealtimeSessionConfig> {
    let config = sess.get_config().await;
    let prompt = prepare_realtime_backend_prompt(
        prompt,
        config.experimental_realtime_ws_backend_prompt.clone(),
    );
    let startup_context = match config.experimental_realtime_ws_startup_context.clone() {
        Some(startup_context) => startup_context,
        None => {
            build_realtime_startup_context(sess.as_ref(), REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET)
                .await
                .unwrap_or_default()
        }
    };
    let prompt = match (prompt.is_empty(), startup_context.is_empty()) {
        (true, true) => String::new(),
        (true, false) => startup_context,
        (false, true) => prompt,
        (false, false) => format!("{prompt}\n\n{startup_context}"),
    };
    let model = Some(
        config
            .experimental_realtime_ws_model
            .clone()
            .unwrap_or_else(|| DEFAULT_REALTIME_MODEL.to_string()),
    );
    let event_parser = match config.realtime.version {
        RealtimeWsVersion::V1 => RealtimeEventParser::V1,
        RealtimeWsVersion::V2 => RealtimeEventParser::RealtimeV2,
    };
    let session_mode = match config.realtime.session_type {
        RealtimeWsMode::Conversational => RealtimeSessionMode::Conversational,
        RealtimeWsMode::Transcription => RealtimeSessionMode::Transcription,
    };
    let voice = voice
        .or(config.realtime.voice)
        .unwrap_or_else(|| default_realtime_voice(config.realtime.version));
    validate_realtime_voice(config.realtime.version, voice)?;
    Ok(RealtimeSessionConfig {
        instructions: prompt,
        model,
        session_id: Some(session_id.unwrap_or_else(|| sess.conversation_id.to_string())),
        event_parser,
        session_mode,
        voice,
    })
}

fn default_realtime_voice(version: RealtimeWsVersion) -> RealtimeVoice {
    let voices = RealtimeVoicesList::builtin();
    match version {
        RealtimeWsVersion::V1 => voices.default_v1,
        RealtimeWsVersion::V2 => voices.default_v2,
    }
}

fn validate_realtime_voice(version: RealtimeWsVersion, voice: RealtimeVoice) -> CodexResult<()> {
    let voices = RealtimeVoicesList::builtin();
    let allowed = match version {
        RealtimeWsVersion::V1 => &voices.v1,
        RealtimeWsVersion::V2 => &voices.v2,
    };
    if allowed.contains(&voice) {
        return Ok(());
    }

    let version = match version {
        RealtimeWsVersion::V1 => "v1",
        RealtimeWsVersion::V2 => "v2",
    };
    let allowed = allowed
        .iter()
        .map(|voice| voice.wire_name())
        .collect::<Vec<_>>()
        .join(", ");
    Err(CodexErr::InvalidRequest(format!(
        "realtime voice `{}` is not supported for {version}; supported voices: {allowed}",
        voice.wire_name()
    )))
}

async fn handle_start_inner(
    sess: &Arc<Session>,
    sub_id: &str,
    prepared_start: PreparedRealtimeConversationStart,
) -> CodexResult<()> {
    let PreparedRealtimeConversationStart {
        api_provider,
        extra_headers,
        requested_session_id,
        version,
        session_config,
        transport,
    } = prepared_start;
    info!("starting realtime conversation");
    let sdp = match transport {
        ConversationStartTransport::Websocket => None,
        ConversationStartTransport::Webrtc { sdp } => Some(sdp),
    };
    let start = RealtimeStart {
        api_provider,
        extra_headers,
        session_config,
        model_client: sess.services.model_client.clone(),
        sdp,
    };
    let start_output = sess.conversation.start(start).await?;

    info!("realtime conversation started");

    sess.send_event_raw(Event {
        id: sub_id.to_string(),
        msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            session_id: requested_session_id,
            version,
        }),
    })
    .await;

    let RealtimeStartOutput {
        realtime_active,
        events_rx,
        sdp,
    } = start_output;
    if let Some(sdp) = sdp {
        sess.send_event_raw(Event {
            id: sub_id.to_string(),
            msg: EventMsg::RealtimeConversationSdp(RealtimeConversationSdpEvent { sdp }),
        })
        .await;
    }

    let sess_clone = Arc::clone(sess);
    let sub_id = sub_id.to_string();
    let fanout_realtime_active = Arc::clone(&realtime_active);
    let fanout_task = tokio::spawn(async move {
        let ev = |msg| Event {
            id: sub_id.clone(),
            msg,
        };
        let mut end = RealtimeConversationEnd::TransportClosed;
        while let Ok(event) = events_rx.recv().await {
            if !fanout_realtime_active.load(Ordering::Relaxed) {
                break;
            }
            match &event {
                RealtimeEvent::AudioOut(_) => {}
                _ => {
                    info!(
                        event = ?event,
                        "received realtime conversation event"
                    );
                }
            }
            if let RealtimeEvent::Error(_) = &event {
                end = RealtimeConversationEnd::Error;
            }
            let maybe_routed_text = match &event {
                RealtimeEvent::HandoffRequested(handoff) => {
                    realtime_text_from_handoff_request(handoff)
                }
                _ => None,
            };
            if let Some(text) = maybe_routed_text {
                debug!(text = %text, "[realtime-text] realtime conversation text output");
                let sess_for_routed_text = Arc::clone(&sess_clone);
                sess_for_routed_text.route_realtime_text_input(text).await;
            }
            if !fanout_realtime_active.load(Ordering::Relaxed) {
                break;
            }
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationRealtime(
                    RealtimeConversationRealtimeEvent {
                        payload: event.clone(),
                    },
                )))
                .await;
        }
        if fanout_realtime_active.swap(false, Ordering::Relaxed) {
            match end {
                RealtimeConversationEnd::TransportClosed => {
                    info!("realtime conversation transport closed");
                }
                RealtimeConversationEnd::Requested | RealtimeConversationEnd::Error => {}
            }
            sess_clone
                .conversation
                .finish_if_active(&fanout_realtime_active)
                .await;
            send_realtime_conversation_closed(&sess_clone, sub_id, end).await;
        }
    });
    sess.conversation
        .register_fanout_task(&realtime_active, fanout_task)
        .await;

    Ok(())
}

pub(crate) async fn handle_audio(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationAudioParams,
) {
    if let Err(err) = sess.conversation.audio_in(params.frame).await {
        error!("failed to append realtime audio: {err}");
        if sess.conversation.running_state().await.is_some() {
            warn!("realtime audio input failed while the session was already ending");
        } else {
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest)
                .await;
        }
    }
}

fn realtime_text_from_handoff_request(handoff: &RealtimeHandoffRequested) -> Option<String> {
    let active_transcript = handoff
        .active_transcript
        .iter()
        .map(|entry| format!("{role}: {text}", role = entry.role, text = entry.text))
        .collect::<Vec<_>>()
        .join("\n");
    (!active_transcript.is_empty())
        .then_some(active_transcript)
        .or((!handoff.input_transcript.is_empty()).then_some(handoff.input_transcript.clone()))
}

fn realtime_api_key(auth: Option<&CodexAuth>, provider: &ModelProviderInfo) -> CodexResult<String> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(api_key);
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(token);
    }

    if let Some(api_key) = auth.and_then(CodexAuth::api_key) {
        return Ok(api_key.to_string());
    }

    // TODO(aibrahim): Remove this temporary fallback once realtime auth no longer
    // requires API key auth for ChatGPT/SIWC sessions.
    if provider.is_openai()
        && let Some(api_key) = read_openai_api_key_from_env()
    {
        return Ok(api_key);
    }

    Err(CodexErr::InvalidRequest(
        "realtime conversation requires API key auth".to_string(),
    ))
}

fn realtime_request_headers(
    session_id: Option<&str>,
    api_key: Option<&str>,
) -> CodexResult<Option<HeaderMap>> {
    let mut headers = HeaderMap::new();

    if let Some(session_id) = session_id
        && let Ok(session_id) = HeaderValue::from_str(session_id)
    {
        headers.insert("x-session-id", session_id);
    }

    if let Some(api_key) = api_key {
        let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
            CodexErr::InvalidRequest(format!("invalid realtime api key header: {err}"))
        })?;
        headers.insert(AUTHORIZATION, auth_value);
    }

    Ok(Some(headers))
}

pub(crate) async fn handle_text(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationTextParams,
) {
    debug!(text = %params.text, "[realtime-text] appending realtime conversation text input");
    if let Err(err) = sess.conversation.text_in(params.text).await {
        error!("failed to append realtime text: {err}");
        if sess.conversation.running_state().await.is_some() {
            warn!("realtime text input failed while the session was already ending");
        } else {
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest)
                .await;
        }
    }
}

pub(crate) async fn handle_close(sess: &Arc<Session>, sub_id: String) {
    end_realtime_conversation(sess, sub_id, RealtimeConversationEnd::Requested).await;
}

fn spawn_realtime_input_task(input: RealtimeInputTask) -> JoinHandle<()> {
    let RealtimeInputTask {
        writer,
        events,
        user_text_rx,
        handoff_output_rx,
        audio_rx,
        events_tx,
        handoff_state,
        session_kind,
        event_parser,
    } = input;

    tokio::spawn(async move {
        let mut output_audio_state: Option<OutputAudioState> = None;
        let mut response_create_queue = RealtimeResponseCreateQueue::default();

        loop {
            let result = tokio::select! {
                // Text typed by the user that should be sent into realtime.
                user_text = user_text_rx.recv() => {
                    handle_user_text_input(
                        user_text,
                        &writer,
                        &events_tx,
                        session_kind,
                        &mut response_create_queue,
                    )
                        .await
                }
                // Background agent progress or final output that should be sent back to realtime.
                background_agent_output = handoff_output_rx.recv() => {
                    handle_handoff_output(
                        background_agent_output,
                        &writer,
                        &events_tx,
                        &handoff_state,
                        event_parser,
                        &mut response_create_queue,
                    )
                        .await
                }
                // Events received from the realtime server.
                realtime_event = events.next_event() => {
                    handle_realtime_server_event(
                        realtime_event,
                        &writer,
                        &events_tx,
                        &handoff_state,
                        session_kind,
                        &mut output_audio_state,
                        &mut response_create_queue,
                    )
                    .await
                }
                // Audio frames captured from the user microphone.
                user_audio_frame = audio_rx.recv() => {
                    handle_user_audio_input(user_audio_frame, &writer, &events_tx)
                        .await
                }
            };
            if result.is_err() {
                break;
            }
        }
    })
}

async fn handle_user_text_input(
    text: Result<String, RecvError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
    session_kind: RealtimeSessionKind,
    response_create_queue: &mut RealtimeResponseCreateQueue,
) -> anyhow::Result<()> {
    let text = text.context("user text input channel closed")?;

    if let Err(err) = writer.send_conversation_item_create(text).await {
        let mapped_error = map_api_error(err);
        warn!("failed to send input text: {mapped_error}");
        let _ = events_tx
            .send(RealtimeEvent::Error(mapped_error.to_string()))
            .await;
        return Err(mapped_error.into());
    }
    match session_kind {
        RealtimeSessionKind::V1 => {}
        RealtimeSessionKind::V2 => {
            response_create_queue
                .request_create(writer, events_tx, "text")
                .await?;
        }
    }
    Ok(())
}

async fn handle_handoff_output(
    handoff_output: Result<HandoffOutput, RecvError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
    handoff_state: &RealtimeHandoffState,
    event_parser: RealtimeEventParser,
    response_create_queue: &mut RealtimeResponseCreateQueue,
) -> anyhow::Result<()> {
    let handoff_output = handoff_output.context("handoff output channel closed")?;

    let result = match event_parser {
        RealtimeEventParser::V1 => match handoff_output {
            HandoffOutput::ProgressUpdate {
                handoff_id,
                output_text,
            }
            | HandoffOutput::FinalUpdate {
                handoff_id,
                output_text,
            } => {
                writer
                    .send_conversation_handoff_append(handoff_id, output_text)
                    .await
            }
        },
        RealtimeEventParser::RealtimeV2 => match handoff_output {
            HandoffOutput::ProgressUpdate {
                handoff_id,
                output_text,
            } => {
                let active_handoff = handoff_state.active_handoff.lock().await.clone();
                match active_handoff {
                    Some(active_handoff) if active_handoff == handoff_id => {}
                    Some(_) | None => {
                        debug!("dropping stale realtime handoff progress update");
                        return Ok(());
                    }
                }
                writer
                    .send_conversation_item_create(format!(
                        "{output_text}{REALTIME_V2_PROGRESS_UPDATE_SUFFIX}"
                    ))
                    .await
            }
            HandoffOutput::FinalUpdate {
                handoff_id,
                output_text,
            } => {
                if let Err(err) = writer
                    .send_conversation_handoff_append(handoff_id, output_text)
                    .await
                {
                    Err(err)
                } else {
                    return response_create_queue
                        .request_create(writer, events_tx, "handoff")
                        .await;
                }
            }
        },
    };
    if let Err(err) = result {
        let mapped_error = map_api_error(err);
        warn!("failed to send handoff output: {mapped_error}");
        let _ = events_tx
            .send(RealtimeEvent::Error(mapped_error.to_string()))
            .await;
        return Err(mapped_error.into());
    }
    Ok(())
}

async fn handle_realtime_server_event(
    event: Result<Option<RealtimeEvent>, ApiError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
    handoff_state: &RealtimeHandoffState,
    session_kind: RealtimeSessionKind,
    output_audio_state: &mut Option<OutputAudioState>,
    response_create_queue: &mut RealtimeResponseCreateQueue,
) -> anyhow::Result<()> {
    let event = match event {
        Ok(Some(event)) => event,
        Ok(None) => anyhow::bail!("realtime event stream ended"),
        Err(err) => {
            let mapped_error = map_api_error(err);
            if events_tx
                .send(RealtimeEvent::Error(mapped_error.to_string()))
                .await
                .is_err()
            {
                return Err(mapped_error.into());
            }
            error!("realtime stream closed: {mapped_error}");
            return Err(mapped_error.into());
        }
    };

    let should_stop = match &event {
        RealtimeEvent::AudioOut(frame) => {
            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => {
                    update_output_audio_state(output_audio_state, frame);
                }
            }
            false
        }
        RealtimeEvent::InputAudioSpeechStarted(event) => {
            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => {
                    if let Some(output_audio_state) = output_audio_state.take()
                        && event
                            .item_id
                            .as_deref()
                            .is_none_or(|item_id| item_id == output_audio_state.item_id)
                        && let Err(err) = writer
                            .send_payload(
                                json!({
                                    "type": "conversation.item.truncate",
                                    "item_id": output_audio_state.item_id,
                                    "content_index": 0,
                                    "audio_end_ms": output_audio_state.audio_end_ms,
                                })
                                .to_string(),
                            )
                            .await
                    {
                        let mapped_error = map_api_error(err);
                        warn!("failed to truncate realtime audio: {mapped_error}");
                    }
                }
            }
            false
        }
        RealtimeEvent::ResponseCreated(_) => {
            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => response_create_queue.mark_started(),
            }
            false
        }
        RealtimeEvent::ResponseCancelled(_) => {
            *output_audio_state = None;
            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => {
                    response_create_queue
                        .mark_finished(writer, events_tx, "deferred")
                        .await?;
                }
            }
            false
        }
        RealtimeEvent::ResponseDone(_) => {
            *output_audio_state = None;
            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => {
                    response_create_queue
                        .mark_finished(writer, events_tx, "deferred")
                        .await?;
                }
            }
            false
        }
        RealtimeEvent::HandoffRequested(handoff) => {
            *output_audio_state = None;

            match session_kind {
                RealtimeSessionKind::V1 => {
                    *handoff_state.last_output_text.lock().await = None;
                    *handoff_state.active_handoff.lock().await = Some(handoff.handoff_id.clone());
                }
                RealtimeSessionKind::V2 => {
                    let active_handoff = handoff_state.active_handoff.lock().await.clone();
                    match active_handoff {
                        Some(_) => {
                            if let Err(err) = writer
                                .send_conversation_handoff_append(
                                    handoff.handoff_id.clone(),
                                    REALTIME_V2_STEER_ACKNOWLEDGEMENT.to_string(),
                                )
                                .await
                            {
                                let mapped_error = map_api_error(err);
                                warn!(
                                    "failed to send handoff steering acknowledgement: {mapped_error}"
                                );
                                let _ = events_tx
                                    .send(RealtimeEvent::Error(mapped_error.to_string()))
                                    .await;
                                return Err(mapped_error.into());
                            }
                            response_create_queue
                                .request_create(writer, events_tx, "handoff steering")
                                .await?;
                        }
                        None => {
                            *handoff_state.last_output_text.lock().await = None;
                            *handoff_state.active_handoff.lock().await =
                                Some(handoff.handoff_id.clone());
                        }
                    }
                }
            }
            false
        }
        RealtimeEvent::Error(_) => true,
        RealtimeEvent::SessionUpdated { .. }
        | RealtimeEvent::InputTranscriptDelta(_)
        | RealtimeEvent::OutputTranscriptDelta(_)
        | RealtimeEvent::ConversationItemAdded(_)
        | RealtimeEvent::ConversationItemDone { .. } => false,
    };

    if events_tx.send(event).await.is_err() {
        anyhow::bail!("realtime output event channel closed");
    }
    if should_stop {
        error!("realtime stream error event received");
        anyhow::bail!("realtime stream error event received");
    }
    Ok(())
}

async fn handle_user_audio_input(
    frame: Result<RealtimeAudioFrame, RecvError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
) -> anyhow::Result<()> {
    let frame = frame.context("user audio input channel closed")?;

    if let Err(err) = writer.send_audio_frame(frame).await {
        let mapped_error = map_api_error(err);
        error!("failed to send input audio: {mapped_error}");
        let _ = events_tx
            .send(RealtimeEvent::Error(mapped_error.to_string()))
            .await;
        return Err(mapped_error.into());
    }
    Ok(())
}

fn update_output_audio_state(
    output_audio_state: &mut Option<OutputAudioState>,
    frame: &RealtimeAudioFrame,
) {
    let Some(item_id) = frame.item_id.clone() else {
        return;
    };
    let audio_end_ms = audio_duration_ms(frame);
    if audio_end_ms == 0 {
        return;
    }

    if let Some(current) = output_audio_state.as_mut()
        && current.item_id == item_id
    {
        current.audio_end_ms = current.audio_end_ms.saturating_add(audio_end_ms);
        return;
    }

    *output_audio_state = Some(OutputAudioState {
        item_id,
        audio_end_ms,
    });
}

fn audio_duration_ms(frame: &RealtimeAudioFrame) -> u32 {
    let Some(samples_per_channel) = frame
        .samples_per_channel
        .or(decoded_samples_per_channel(frame))
    else {
        return 0;
    };
    let sample_rate = u64::from(frame.sample_rate.max(1));
    ((u64::from(samples_per_channel) * 1_000) / sample_rate) as u32
}

fn decoded_samples_per_channel(frame: &RealtimeAudioFrame) -> Option<u32> {
    let bytes = BASE64_STANDARD.decode(&frame.data).ok()?;
    let channels = usize::from(frame.num_channels.max(1));
    let samples = bytes.len().checked_div(2)?.checked_div(channels)?;
    u32::try_from(samples).ok()
}

async fn send_conversation_error(
    sess: &Arc<Session>,
    sub_id: String,
    message: String,
    codex_error_info: CodexErrorInfo,
) {
    sess.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::Error(ErrorEvent {
            message,
            codex_error_info: Some(codex_error_info),
        }),
    })
    .await;
}

async fn end_realtime_conversation(
    sess: &Arc<Session>,
    sub_id: String,
    end: RealtimeConversationEnd,
) {
    let _ = sess.conversation.shutdown().await;
    send_realtime_conversation_closed(sess, sub_id, end).await;
}

async fn send_realtime_conversation_closed(
    sess: &Arc<Session>,
    sub_id: String,
    end: RealtimeConversationEnd,
) {
    let reason = match end {
        RealtimeConversationEnd::Requested => Some("requested".to_string()),
        RealtimeConversationEnd::TransportClosed => Some("transport_closed".to_string()),
        RealtimeConversationEnd::Error => Some("error".to_string()),
    };

    sess.send_event_raw(Event {
        id: sub_id,
        msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent { reason }),
    })
    .await;
}

#[cfg(test)]
#[path = "realtime_conversation_tests.rs"]
mod tests;
