use crate::api_bridge::map_api_error;
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
use http::HeaderMap;
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

        let task = tokio::spawn(async move {
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
                            Err(_) => {
                                break;
                            }
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
                            Err(_) => {
                                break;
                            }
                        }
                    }
                }
            }
        });

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
