use crate::CodexAuth;
use crate::api_bridge::map_api_error;
use crate::codex::Session;
use crate::default_client::default_headers;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use async_channel::Receiver;
use async_channel::Sender;
use async_channel::TrySendError;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
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
use http::HeaderMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::warn;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const TEXT_IN_QUEUE_CAPACITY: usize = 64;
const OUTPUT_EVENTS_QUEUE_CAPACITY: usize = 256;

pub(crate) struct RealtimeConversationManager {
    state: Mutex<Option<ConversationState>>,
}

#[allow(dead_code)]
struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    text_tx: Sender<String>,
    task: JoinHandle<()>,
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
        state.as_ref().map(|_| ())
    }

    pub(crate) async fn start(
        &self,
        api_provider: ApiProvider,
        extra_headers: Option<HeaderMap>,
        prompt: String,
        session_id: Option<String>,
    ) -> CodexResult<Receiver<RealtimeEvent>> {
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            state.task.abort();
            let _ = state.task.await;
        }

        let session_config = RealtimeSessionConfig { prompt, session_id };
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
        let (text_tx, text_rx) = async_channel::bounded::<String>(TEXT_IN_QUEUE_CAPACITY);
        let (events_tx, events_rx) =
            async_channel::bounded::<RealtimeEvent>(OUTPUT_EVENTS_QUEUE_CAPACITY);

        let task = spawn_realtime_input_task(writer, events, text_rx, audio_rx, events_tx);

        let mut guard = self.state.lock().await;
        *guard = Some(ConversationState {
            audio_tx,
            text_tx,
            task,
        });
        Ok(events_rx)
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
            guard.as_ref().map(|state| state.text_tx.clone())
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

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
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
    let mut api_provider = provider.to_api_provider(auth.as_ref().map(CodexAuth::auth_mode))?;
    let config = sess.get_config().await;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }
    let prompt = config
        .experimental_realtime_ws_backend_prompt
        .clone()
        .unwrap_or(params.prompt);

    let requested_session_id = params
        .session_id
        .or_else(|| Some(sess.conversation_id.to_string()));
    let events_rx = match sess
        .conversation
        .start(api_provider, None, prompt, requested_session_id.clone())
        .await
    {
        Ok(events_rx) => events_rx,
        Err(err) => {
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other).await;
            return Ok(());
        }
    };

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
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationRealtime(
                    RealtimeConversationRealtimeEvent { payload: event },
                )))
                .await;
        }
        if let Some(()) = sess_clone.conversation.running_state().await {
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
        send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest).await;
    }
}

pub(crate) async fn handle_text(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationTextParams,
) {
    if let Err(err) = sess.conversation.text_in(params.text).await {
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
    text_rx: Receiver<String>,
    audio_rx: Receiver<RealtimeAudioFrame>,
    events_tx: Sender<RealtimeEvent>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        loop {
            tokio::select! {
                biased;
                text = text_rx.recv() => {
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
                event = events.next_event() => {
                    match event {
                        Ok(Some(event)) => {
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
