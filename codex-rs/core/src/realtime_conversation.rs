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
use serde_json::Value;
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
        let running = state.is_some();
        eprintln!("[rt-debug] conversation.running_state -> {running}");
        state.as_ref().map(|_| ())
    }

    pub(crate) async fn start(
        &self,
        api_provider: ApiProvider,
        extra_headers: Option<HeaderMap>,
        prompt: String,
        session_id: Option<String>,
    ) -> CodexResult<Receiver<RealtimeEvent>> {
        eprintln!(
            "[rt-debug] conversation.start begin prompt_len={} session_id={session_id:?}",
            prompt.len()
        );
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            eprintln!("[rt-debug] conversation.start aborting previous realtime task");
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
        eprintln!("[rt-debug] conversation.start websocket connected");

        let writer = connection.writer();
        let events = connection.events();
        let (audio_tx, audio_rx) =
            async_channel::bounded::<RealtimeAudioFrame>(AUDIO_IN_QUEUE_CAPACITY);
        let (text_tx, text_rx) = async_channel::bounded::<String>(TEXT_IN_QUEUE_CAPACITY);
        let (events_tx, events_rx) =
            async_channel::bounded::<RealtimeEvent>(OUTPUT_EVENTS_QUEUE_CAPACITY);

        let task = spawn_realtime_input_task(writer, events, text_rx, audio_rx, events_tx);
        eprintln!("[rt-debug] conversation.start spawned realtime input task");

        let mut guard = self.state.lock().await;
        *guard = Some(ConversationState {
            audio_tx,
            text_tx,
            task,
        });
        eprintln!("[rt-debug] conversation.start state installed");
        Ok(events_rx)
    }

    pub(crate) async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
        eprintln!(
            "[rt-debug] conversation.audio_in sample_rate={} channels={} samples_per_channel={:?} data_len={}",
            frame.sample_rate,
            frame.num_channels,
            frame.samples_per_channel,
            frame.data.len()
        );
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
            Ok(()) => {
                eprintln!("[rt-debug] conversation.audio_in queued");
                Ok(())
            }
            Err(TrySendError::Full(_)) => {
                eprintln!("[rt-debug] conversation.audio_in queue full; dropping frame");
                warn!("dropping input audio frame due to full queue");
                Ok(())
            }
            Err(TrySendError::Closed(_)) => {
                eprintln!("[rt-debug] conversation.audio_in queue closed");
                Err(CodexErr::InvalidRequest(
                    "conversation is not running".to_string(),
                ))
            }
        }
    }

    pub(crate) async fn text_in(&self, text: String) -> CodexResult<()> {
        eprintln!(
            "[rt-debug] conversation.text_in len={} text={text:?}",
            text.len()
        );
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
        eprintln!("[rt-debug] conversation.text_in queued");
        Ok(())
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        eprintln!("[rt-debug] conversation.shutdown begin");
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            eprintln!("[rt-debug] conversation.shutdown aborting realtime task");
            state.task.abort();
            let _ = state.task.await;
        }
        eprintln!("[rt-debug] conversation.shutdown done");
        Ok(())
    }
}

pub(crate) async fn handle_start(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationStartParams,
) -> CodexResult<()> {
    eprintln!(
        "[rt-debug] handle_start begin sub_id={sub_id} prompt_len={} requested_session_id={:?}",
        params.prompt.len(),
        params.session_id
    );
    let provider = sess.provider().await;
    let auth = sess.services.auth_manager.auth().await;
    let mut api_provider = provider.to_api_provider(auth.as_ref().map(CodexAuth::auth_mode))?;
    let config = sess.get_config().await;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
        eprintln!(
            "[rt-debug] handle_start overriding realtime base_url={}",
            api_provider.base_url
        );
    }
    let prompt = config
        .experimental_realtime_ws_backend_prompt
        .clone()
        .unwrap_or(params.prompt);

    let requested_session_id = params
        .session_id
        .or_else(|| Some(sess.conversation_id.to_string()));
    eprintln!(
        "[rt-debug] handle_start effective prompt_len={} requested_session_id={requested_session_id:?}",
        prompt.len()
    );
    let events_rx = match sess
        .conversation
        .start(api_provider, None, prompt, requested_session_id.clone())
        .await
    {
        Ok(events_rx) => events_rx,
        Err(err) => {
            eprintln!("[rt-debug] handle_start conversation.start failed: {err}");
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::Other).await;
            return Ok(());
        }
    };
    eprintln!("[rt-debug] handle_start conversation.start ok");

    sess.send_event_raw(Event {
        id: sub_id.clone(),
        msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            session_id: requested_session_id,
        }),
    })
    .await;
    eprintln!("[rt-debug] handle_start emitted RealtimeConversationStarted");

    let sess_clone = Arc::clone(sess);
    tokio::spawn(async move {
        eprintln!("[rt-debug] handle_start event-forwarder task started");
        let ev = |msg| Event {
            id: sub_id.clone(),
            msg,
        };
        while let Ok(event) = events_rx.recv().await {
            eprintln!("[rt-debug] realtime event recv: {event:?}");
            let maybe_routed_text = match &event {
                RealtimeEvent::ConversationItemAdded(item) => {
                    realtime_text_from_conversation_item(item)
                }
                _ => None,
            };
            if let Some(text) = maybe_routed_text {
                eprintln!("[rt-debug] routing inbound realtime text start: {text:?}");
                sess_clone.route_realtime_text_input(text).await;
                eprintln!("[rt-debug] routing inbound realtime text done");
            }
            eprintln!("[rt-debug] emitting mirrored realtime event");
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationRealtime(
                    RealtimeConversationRealtimeEvent {
                        payload: event.clone(),
                    },
                )))
                .await;
            eprintln!("[rt-debug] mirrored realtime event emitted");
        }
        eprintln!("[rt-debug] handle_start event-forwarder loop ended");
        if let Some(()) = sess_clone.conversation.running_state().await {
            eprintln!("[rt-debug] handle_start emitting transport_closed");
            sess_clone
                .send_event_raw(ev(EventMsg::RealtimeConversationClosed(
                    RealtimeConversationClosedEvent {
                        reason: Some("transport_closed".to_string()),
                    },
                )))
                .await;
        }
        eprintln!("[rt-debug] handle_start event-forwarder task exiting");
    });

    Ok(())
}

pub(crate) async fn handle_audio(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationAudioParams,
) {
    eprintln!("[rt-debug] handle_audio sub_id={sub_id}");
    if let Err(err) = sess.conversation.audio_in(params.frame).await {
        eprintln!("[rt-debug] handle_audio error: {err}");
        send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest).await;
    }
}

fn realtime_text_from_conversation_item(item: &Value) -> Option<String> {
    if item.get("type").and_then(Value::as_str) != Some("message") {
        return None;
    }
    let content = item.get("content")?.as_array()?;
    let text = content
        .iter()
        .filter(|entry| entry.get("type").and_then(Value::as_str) == Some("text"))
        .filter_map(|entry| entry.get("text").and_then(Value::as_str))
        .collect::<String>();
    if text.is_empty() { None } else { Some(text) }
}

pub(crate) async fn handle_text(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationTextParams,
) {
    eprintln!(
        "[rt-debug] handle_text sub_id={sub_id} text={:?}",
        params.text
    );
    if let Err(err) = sess.conversation.text_in(params.text).await {
        eprintln!("[rt-debug] handle_text error: {err}");
        send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest).await;
    }
}

pub(crate) async fn handle_close(sess: &Arc<Session>, sub_id: String) {
    eprintln!("[rt-debug] handle_close sub_id={sub_id}");
    match sess.conversation.shutdown().await {
        Ok(()) => {
            sess.send_event_raw(Event {
                id: sub_id,
                msg: EventMsg::RealtimeConversationClosed(RealtimeConversationClosedEvent {
                    reason: Some("requested".to_string()),
                }),
            })
            .await;
            eprintln!("[rt-debug] handle_close emitted requested close");
        }
        Err(err) => {
            eprintln!("[rt-debug] handle_close error: {err}");
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
        eprintln!("[rt-debug] spawn_realtime_input_task started");
        loop {
            tokio::select! {
                biased;
                text = text_rx.recv() => {
                    match text {
                        Ok(text) => {
                            eprintln!("[rt-debug] spawn_realtime_input_task text_rx recv: {text:?}");
                            if let Err(err) = writer.send_conversation_item_create(text).await {
                                let mapped_error = map_api_error(err);
                                eprintln!("[rt-debug] spawn_realtime_input_task send_conversation_item_create error: {mapped_error}");
                                warn!("failed to send input text: {mapped_error}");
                                break;
                            }
                            eprintln!("[rt-debug] spawn_realtime_input_task send_conversation_item_create ok");
                        }
                        Err(_) => {
                            eprintln!("[rt-debug] spawn_realtime_input_task text_rx closed");
                            break;
                        }
                    }
                }
                event = events.next_event() => {
                    match event {
                        Ok(Some(event)) => {
                            eprintln!("[rt-debug] spawn_realtime_input_task ws event: {event:?}");
                            let should_stop = matches!(&event, RealtimeEvent::Error(_));
                            if events_tx.send(event).await.is_err() {
                                eprintln!("[rt-debug] spawn_realtime_input_task events_tx closed");
                                break;
                            }
                            eprintln!("[rt-debug] spawn_realtime_input_task ws event forwarded");
                            if should_stop {
                                eprintln!("[rt-debug] spawn_realtime_input_task stopping on error event");
                                error!("realtime stream error event received");
                                break;
                            }
                        }
                        Ok(None) => {
                            eprintln!("[rt-debug] spawn_realtime_input_task ws event stream closed");
                            let _ = events_tx
                                .send(RealtimeEvent::Error(
                                    "realtime websocket connection is closed".to_string(),
                                ))
                                .await;
                            break;
                        }
                        Err(err) => {
                            let mapped_error = map_api_error(err);
                            eprintln!("[rt-debug] spawn_realtime_input_task ws error: {mapped_error}");
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
                            eprintln!("[rt-debug] spawn_realtime_input_task audio_rx recv");
                            if let Err(err) = writer.send_audio_frame(frame).await {
                                let mapped_error = map_api_error(err);
                                eprintln!("[rt-debug] spawn_realtime_input_task send_audio_frame error: {mapped_error}");
                                error!("failed to send input audio: {mapped_error}");
                                break;
                            }
                            eprintln!("[rt-debug] spawn_realtime_input_task send_audio_frame ok");
                        }
                        Err(_) => {
                            eprintln!("[rt-debug] spawn_realtime_input_task audio_rx closed");
                            break;
                        }
                    }
                }
            }
        }
        eprintln!("[rt-debug] spawn_realtime_input_task exiting");
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
mod tests {
    use super::realtime_text_from_conversation_item;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn extracts_text_from_message_items_ignoring_role() {
        let assistant = json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "text", "text": "hello"}],
        });
        assert_eq!(
            realtime_text_from_conversation_item(&assistant),
            Some("hello".to_string())
        );

        let user = json!({
            "type": "message",
            "role": "user",
            "content": [{"type": "text", "text": "world"}],
        });
        assert_eq!(
            realtime_text_from_conversation_item(&user),
            Some("world".to_string())
        );
    }

    #[test]
    fn extracts_and_concatenates_text_entries_only() {
        let item = json!({
            "type": "message",
            "content": [
                {"type": "text", "text": "a"},
                {"type": "ignored", "text": "x"},
                {"type": "text", "text": "b"}
            ],
        });
        assert_eq!(
            realtime_text_from_conversation_item(&item),
            Some("ab".to_string())
        );
    }

    #[test]
    fn ignores_non_message_or_missing_text() {
        let non_message = json!({
            "type": "tool_call",
            "content": [{"type": "text", "text": "nope"}],
        });
        assert_eq!(realtime_text_from_conversation_item(&non_message), None);

        let no_text = json!({
            "type": "message",
            "content": [{"type": "other", "value": 1}],
        });
        assert_eq!(realtime_text_from_conversation_item(&no_text), None);
    }
}
