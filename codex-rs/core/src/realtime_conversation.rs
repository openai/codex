use crate::CodexAuth;
use crate::api_bridge::map_api_error;
use crate::auth::read_openai_api_key_from_env;
use crate::codex::Session;
use crate::config::RealtimeWsMode;
use crate::config::RealtimeWsVersion;
use crate::default_client::default_headers;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use crate::realtime_context::build_realtime_startup_context;
use async_channel::Receiver;
use async_channel::Sender;
use async_channel::TrySendError;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::RealtimeWebsocketClient;
use codex_api::endpoint::realtime_websocket::RealtimeWebsocketEvents;
use codex_api::endpoint::realtime_websocket::RealtimeWebsocketWriter;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
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

pub(crate) struct RealtimeConversationManager {
    state: Mutex<Option<ConversationState>>,
}

#[derive(Clone, Debug)]
struct RealtimeHandoffState {
    output_tx: Sender<HandoffOutput>,
    active_handoff: Arc<Mutex<Option<String>>>,
}

#[derive(Debug, PartialEq, Eq)]
struct HandoffOutput {
    handoff_id: String,
    output_text: String,
}

impl RealtimeHandoffState {
    fn new(output_tx: Sender<HandoffOutput>) -> Self {
        Self {
            output_tx,
            active_handoff: Arc::new(Mutex::new(None)),
        }
    }

    async fn send_output(&self, output_text: String) -> CodexResult<()> {
        let Some(handoff_id) = self.active_handoff.lock().await.clone() else {
            return Ok(());
        };

        self.output_tx
            .send(HandoffOutput {
                handoff_id,
                output_text,
            })
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }
}

#[allow(dead_code)]
struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    user_text_tx: Sender<String>,
    handoff: RealtimeHandoffState,
    task: JoinHandle<()>,
    realtime_active: Arc<AtomicBool>,
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

    pub(crate) async fn start(
        &self,
        api_provider: ApiProvider,
        extra_headers: Option<HeaderMap>,
        session_config: RealtimeSessionConfig,
    ) -> CodexResult<(Receiver<RealtimeEvent>, Arc<AtomicBool>)> {
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            state.realtime_active.store(false, Ordering::Relaxed);
            state.task.abort();
            let _ = state.task.await;
        }

        let client = RealtimeWebsocketClient::new(api_provider);
        let connection = client
            .connect(
                session_config,
                extra_headers.unwrap_or_default(),
                default_headers(),
            )
            .await
            .map_err(map_api_error)?;

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
        let handoff = RealtimeHandoffState::new(handoff_output_tx);
        let task = spawn_realtime_input_task(
            writer,
            events,
            user_text_rx,
            handoff_output_rx,
            audio_rx,
            events_tx,
            handoff.clone(),
        );

        let mut guard = self.state.lock().await;
        *guard = Some(ConversationState {
            audio_tx,
            user_text_tx,
            handoff,
            task,
            realtime_active: Arc::clone(&realtime_active),
        });
        Ok((events_rx, realtime_active))
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

        handoff.send_output(output_text).await
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
        }
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            state.realtime_active.store(false, Ordering::Relaxed);
            state.task.abort();
            let _ = state.task.await;
        }
        Ok(())
    }
}

pub(crate) async fn handle_start(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationStartParams,
) -> CodexResult<()> {
    let provider = sess.provider().await;
    let auth = sess.services.auth_manager.auth().await;
    let realtime_api_key = realtime_api_key(auth.as_ref(), &provider)?;
    let mut api_provider = provider.to_api_provider(Some(crate::auth::AuthMode::ApiKey))?;
    let config = sess.get_config().await;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }
    let prompt = config
        .experimental_realtime_ws_backend_prompt
        .clone()
        .unwrap_or(params.prompt);
    let startup_context = match config.experimental_realtime_ws_startup_context.clone() {
        Some(startup_context) => startup_context,
        None => {
            build_realtime_startup_context(sess.as_ref(), REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET)
                .await
                .unwrap_or_default()
        }
    };
    let prompt = if startup_context.is_empty() {
        prompt
    } else {
        format!("{prompt}\n\n{startup_context}")
    };
    let model = config.experimental_realtime_ws_model.clone();
    let event_parser = match config.realtime.version {
        RealtimeWsVersion::V1 => RealtimeEventParser::V1,
        RealtimeWsVersion::V2 => RealtimeEventParser::RealtimeV2,
    };
    let session_mode = match config.realtime.session_type {
        RealtimeWsMode::Conversational => RealtimeSessionMode::Conversational,
        RealtimeWsMode::Transcription => RealtimeSessionMode::Transcription,
    };
    let requested_session_id = params
        .session_id
        .or_else(|| Some(sess.conversation_id.to_string()));
    let session_config = RealtimeSessionConfig {
        instructions: prompt,
        model,
        session_id: requested_session_id.clone(),
        event_parser,
        session_mode,
    };
    let extra_headers =
        realtime_request_headers(requested_session_id.as_deref(), realtime_api_key.as_str())?;
    info!("starting realtime conversation");
    let (events_rx, realtime_active) = match sess
        .conversation
        .start(api_provider, extra_headers, session_config)
        .await
    {
        Ok(events_rx) => events_rx,
        Err(err) => {
            error!("failed to start realtime conversation: {err}");
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other).await;
            return Ok(());
        }
    };

    info!("realtime conversation started");

    sess.send_event_raw(Event {
        id: sub_id.clone(),
        msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            session_id: requested_session_id,
        }),
    })
    .await;

    let sess_clone = Arc::clone(sess);
    tokio::spawn(async move {
        let ev = |msg| Event {
            id: sub_id.clone(),
            msg,
        };
        while let Ok(event) = events_rx.recv().await {
            // if not audio out, log the event
            if !matches!(event, RealtimeEvent::AudioOut(_)) {
                info!(
                    event = ?event,
                    "received realtime conversation event"
                );
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
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationRealtime(
                    RealtimeConversationRealtimeEvent {
                        payload: event.clone(),
                    },
                )))
                .await;
        }
        if realtime_active.swap(false, Ordering::Relaxed) {
            info!("realtime conversation transport closed");
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationClosed(
                    RealtimeConversationClosedEvent {
                        reason: Some("transport_closed".to_string()),
                    },
                )))
                .await;
        }
    });

    Ok(())
}

pub(crate) async fn handle_audio(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationAudioParams,
) {
    if let Err(err) = sess.conversation.audio_in(params.frame).await {
        error!("failed to append realtime audio: {err}");
        send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest).await;
    }
}

fn realtime_text_from_handoff_request(handoff: &RealtimeHandoffRequested) -> Option<String> {
    let active_transcript = handoff
        .active_transcript
        .iter()
        .map(|entry| format!("{}: {}", entry.role, entry.text))
        .collect::<Vec<_>>()
        .join("\n");
    (!active_transcript.is_empty())
        .then_some(active_transcript)
        .or_else(|| {
            (!handoff.input_transcript.is_empty()).then(|| handoff.input_transcript.clone())
        })
}

fn realtime_api_key(
    auth: Option<&CodexAuth>,
    provider: &crate::ModelProviderInfo,
) -> CodexResult<String> {
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
    api_key: &str,
) -> CodexResult<Option<HeaderMap>> {
    let mut headers = HeaderMap::new();

    if let Some(session_id) = session_id
        && let Ok(session_id) = HeaderValue::from_str(session_id)
    {
        headers.insert("x-session-id", session_id);
    }

    let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
        CodexErr::InvalidRequest(format!("invalid realtime api key header: {err}"))
    })?;
    headers.insert(AUTHORIZATION, auth_value);

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
        send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest).await;
    }
}

pub(crate) async fn handle_close(sess: &Arc<Session>, sub_id: String) {
    match sess.conversation.shutdown().await {
        Ok(()) => {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: Some("requested".to_string()),
                }),
            })
            .await;
        }
        Err(err) => {
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other).await;
        }
    }
}

fn spawn_realtime_input_task(
    writer: RealtimeWebsocketWriter,
    events: RealtimeWebsocketEvents,
    user_text_rx: Receiver<String>,
    handoff_output_rx: Receiver<HandoffOutput>,
    audio_rx: Receiver<RealtimeAudioFrame>,
    events_tx: Sender<RealtimeEvent>,
    handoff_state: RealtimeHandoffState,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                text = user_text_rx.recv() => {
                    match text {
                        Ok(text) => {
                            if let Err(err) = writer.send_conversation_item_create(text).await {
                                let mapped_error = map_api_error(err);
                                warn!("failed to send input text: {mapped_error}");
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                handoff_output = handoff_output_rx.recv() => {
                    match handoff_output {
                        Ok(HandoffOutput {
                            handoff_id,
                            output_text,
                        }) => {
                            if let Err(err) = writer
                                .send_conversation_handoff_append(handoff_id, output_text)
                                .await
                            {
                                let mapped_error = map_api_error(err);
                                warn!("failed to send handoff output: {mapped_error}");
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
                event = events.next_event() => {
                    match event {
                        Ok(Some(event)) => {
                            if let RealtimeEvent::HandoffRequested(handoff) = &event {
                                *handoff_state.active_handoff.lock().await =
                                    Some(handoff.handoff_id.clone());
                            }
                            let should_stop = matches!(&event, RealtimeEvent::Error(_));
                            if events_tx.send(event).await.is_err() {
                                break;
                            }
                            if should_stop {
                                error!("realtime stream error event received");
                                break;
                            }
                        }
                        Ok(None) => {
                            let _ = events_tx
                                .send(RealtimeEvent::Error(
                                    "realtime websocket connection is closed".to_string(),
                                ))
                                .await;
                            break;
                        }
                        Err(err) => {
                            let mapped_error = map_api_error(err);
                            if events_tx
                                .send(RealtimeEvent::Error(mapped_error.to_string()))
                                .await
                                .is_err()
                            {
                                break;
                            }
                            error!("realtime stream closed: {mapped_error}");
                            break;
                        }
                    }
                }
                frame = audio_rx.recv() => {
                    match frame {
                        Ok(frame) => {
                            if let Err(err) = writer.send_audio_frame(frame).await {
                                let mapped_error = map_api_error(err);
                                error!("failed to send input audio: {mapped_error}");
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            }
        }
    })
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

#[cfg(test)]
#[path = "realtime_conversation_tests.rs"]
mod tests;
