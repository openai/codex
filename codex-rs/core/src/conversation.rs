use crate::api_bridge::map_api_error;
use crate::default_client::default_headers;
use crate::error::CodexErr;
use crate::error::Result as CodexResult;
use codex_api::Provider as ApiProvider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeWebsocketClient;
use http::HeaderMap;
use tokio::sync::Mutex;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tracing::warn;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const TEXT_IN_QUEUE_CAPACITY: usize = 64;

pub(crate) struct RealtimeConversationManager {
    state: Mutex<Option<ConversationState>>,
}

#[allow(dead_code)]
struct ConversationState {
    audio_tx: mpsc::Sender<RealtimeAudioFrame>,
    text_tx: mpsc::Sender<String>,
    task: JoinHandle<()>,
}

#[allow(dead_code)]
impl RealtimeConversationManager {
    pub(crate) fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    pub(crate) async fn start(
        &self,
        api_provider: ApiProvider,
        extra_headers: HeaderMap,
        prompt: String,
        session_id: Option<String>,
    ) -> CodexResult<()> {
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
            .connect(session_config, extra_headers, default_headers())
            .await
            .map_err(map_api_error)?;

        let writer = connection.writer();
        let events = connection.events();
        let (audio_tx, mut audio_rx) = mpsc::channel::<RealtimeAudioFrame>(AUDIO_IN_QUEUE_CAPACITY);
        let (text_tx, mut text_rx) = mpsc::channel::<String>(TEXT_IN_QUEUE_CAPACITY);

        let task = tokio::spawn(async move {
            loop {
                tokio::select! {
                    biased;
                    text = text_rx.recv() => {
                        match text {
                            Some(text) => {
                                if let Err(err) = writer.send_conversation_item_create(text).await {
                                    let mapped_error = map_api_error(err);
                                    warn!("failed to send input text: {mapped_error}");
                                    break;
                                }
                            }
                            None => {
                                break;
                            }
                        }
                    }
                    event = events.next_event() => {
                        match event {
                            Ok(Some(RealtimeEvent::SessionCreated { .. }))
                            | Ok(Some(RealtimeEvent::SessionUpdated { .. }))
                            | Ok(Some(RealtimeEvent::AudioOut(_)))
                            | Ok(Some(RealtimeEvent::ConversationItemAdded(_))) => {}
                            Ok(Some(RealtimeEvent::Error(message))) => {
                                error!("realtime stream error: {message}");
                                break;
                            }
                            Ok(None) => {
                                break;
                            }
                            Err(err) => {
                                let mapped_error = map_api_error(err);
                                error!("realtime stream closed: {mapped_error}");
                                break;
                            }
                        }
                    }
                    frame = audio_rx.recv() => {
                        match frame {
                            Some(frame) => {
                                if let Err(err) = writer.send_audio_frame(frame).await {
                                    let mapped_error = map_api_error(err);
                                    error!("failed to send input audio: {mapped_error}");
                                    break;
                                }
                            }
                            None => {
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
        Ok(())
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
            Err(mpsc::error::TrySendError::Full(_)) => {
                warn!("dropping input audio frame due to full queue");
                Ok(())
            }
            Err(mpsc::error::TrySendError::Closed(_)) => Err(CodexErr::InvalidRequest(
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

    pub(crate) async fn shutdown(&self) {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            state.task.abort();
            let _ = state.task.await;
        }
    }
}
