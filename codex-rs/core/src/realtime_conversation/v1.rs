use crate::api_bridge::map_api_error;
use crate::default_client::default_headers;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use async_channel::Receiver;
use async_channel::Sender;
use async_channel::TrySendError;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeApiMode;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeWebsocketClient;
use codex_api::endpoint::realtime_websocket::RealtimeWebsocketEvents;
use codex_api::endpoint::realtime_websocket::RealtimeWebsocketWriter;
use http::HeaderMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::error;
use tracing::warn;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const USER_TEXT_IN_QUEUE_CAPACITY: usize = 64;
const HANDOFF_OUT_QUEUE_CAPACITY: usize = 64;
const OUTPUT_EVENTS_QUEUE_CAPACITY: usize = 256;

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

pub(super) struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    user_text_tx: Sender<String>,
    handoff: RealtimeHandoffState,
    task: JoinHandle<()>,
    realtime_active: Arc<AtomicBool>,
}

impl ConversationState {
    pub(super) fn is_running(&self) -> bool {
        self.realtime_active.load(Ordering::Relaxed)
    }

    pub(super) async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
        match self.audio_tx.try_send(frame) {
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

    pub(super) async fn text_in(&self, text: String) -> CodexResult<()> {
        self.user_text_tx
            .send(text)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub(super) async fn handoff_out(&self, output_text: String) -> CodexResult<()> {
        self.handoff.send_output(output_text).await
    }

    pub(super) async fn handoff_complete(&self) -> CodexResult<()> {
        Ok(())
    }

    pub(super) async fn active_handoff_id(&self) -> Option<String> {
        self.handoff.active_handoff.lock().await.clone()
    }

    pub(super) async fn clear_active_handoff(&self) {
        *self.handoff.active_handoff.lock().await = None;
    }

    pub(super) async fn shutdown(self) {
        self.realtime_active.store(false, Ordering::Relaxed);
        self.task.abort();
        let _ = self.task.await;
    }

    pub(super) fn realtime_active(&self) -> Arc<AtomicBool> {
        Arc::clone(&self.realtime_active)
    }
}

pub(super) async fn start(
    api_provider: ApiProvider,
    extra_headers: Option<HeaderMap>,
    prompt: String,
    model: Option<String>,
    session_id: Option<String>,
) -> CodexResult<(ConversationState, Receiver<RealtimeEvent>)> {
    let session_config = RealtimeSessionConfig {
        instructions: prompt,
        model,
        session_id,
        mode: RealtimeApiMode::V1,
    };
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

    Ok((
        ConversationState {
            audio_tx,
            user_text_tx,
            handoff,
            task,
            realtime_active,
        },
        events_rx,
    ))
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
                        Ok(HandoffOutput { handoff_id, output_text }) => {
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

#[cfg(test)]
mod tests {
    use super::HandoffOutput;
    use super::RealtimeHandoffState;
    use async_channel::bounded;
    use pretty_assertions::assert_eq;

    #[tokio::test]
    async fn sends_handoff_outputs_when_handoff_is_active() {
        let (tx, rx) = bounded(2);
        let state = RealtimeHandoffState::new(tx);
        *state.active_handoff.lock().await = Some("handoff_1".to_string());

        state
            .send_output("legacy output".to_string())
            .await
            .expect("send");

        let output = rx.recv().await.expect("recv");
        assert_eq!(
            output,
            HandoffOutput {
                handoff_id: "handoff_1".to_string(),
                output_text: "legacy output".to_string(),
            }
        );
    }

    #[tokio::test]
    async fn ignores_handoff_output_when_handoff_is_not_active() {
        let (tx, rx) = bounded(1);
        let state = RealtimeHandoffState::new(tx);

        state
            .send_output("ignored".to_string())
            .await
            .expect("send");

        assert!(rx.is_empty());
    }
}
