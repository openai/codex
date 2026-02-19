use crate::CodexAuth;
use crate::api_bridge::auth_provider_from_auth;
use crate::api_bridge::map_api_error;
use crate::codex::Session;
use crate::default_client::default_headers;
use codex_api::AuthProvider;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeWebsocketClient;
use codex_api::endpoint::realtime_websocket::RealtimeWebsocketWriter;
use codex_api::error::ApiError;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::ConversationAudioOutEvent;
use codex_protocol::protocol::ConversationItemAddedEvent;
use codex_protocol::protocol::ConversationStartedEvent;
use codex_protocol::protocol::ConversationStoppedEvent;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use http::HeaderMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::warn;

pub(crate) struct RealtimeConversationManager {
    runtime: Mutex<Option<ConversationRuntime>>,
}

#[allow(dead_code)]
struct ConversationRuntime {
    sub_id: String,
    writer: RealtimeWebsocketWriter,
    events_task: JoinHandle<()>,
}

#[allow(dead_code)]
impl RealtimeConversationManager {
    pub(crate) fn new() -> Self {
        Self {
            runtime: Mutex::new(None),
        }
    }

    pub(crate) async fn start(
        &self,
        sess: &Arc<Session>,
        sub_id: String,
        api_url: Option<String>,
        backend_prompt: Option<String>,
        session_id: Option<String>,
    ) {
        self.start_conversation(sess, sub_id, api_url, backend_prompt, session_id)
            .await;
    }

    pub(crate) async fn audio_in(&self, sess: &Session, sub_id: &str, frame: RealtimeAudioFrame) {
        self.with_writer(sess, sub_id, |writer| async move {
            writer.send_audio_frame(frame).await
        })
        .await;
    }

    pub(crate) async fn text_in(&self, sess: &Session, sub_id: &str, text: String) {
        self.with_writer(sess, sub_id, |writer| async move {
            writer.send_conversation_item_create(text).await
        })
        .await;
    }

    pub(crate) async fn stop(&self, sess: &Session, sub_id: &str) {
        self.shutdown(sess, sub_id).await;
    }

    pub(crate) async fn shutdown(&self, sess: &Session, sub_id: &str) {
        if let Some(runtime) = self.take_runtime().await {
            stop_conversation_runtime(runtime).await;
            sess.send_event_raw(Event {
                id: sub_id.to_string(),
                msg: EventMsg::ConversationStopped(ConversationStoppedEvent),
            })
            .await;
        }
    }

    async fn start_conversation(
        &self,
        sess: &Arc<Session>,
        sub_id: String,
        api_url: Option<String>,
        backend_prompt: Option<String>,
        session_id: Option<String>,
    ) {
        if let Some(runtime) = self.take_runtime().await {
            stop_conversation_runtime(runtime).await;
        }

        let turn_context = sess.new_default_turn_with_sub_id(sub_id.clone()).await;
        let provider = turn_context.provider.clone();
        let auth = sess.services.auth_manager.auth().await;

        let api_provider = match provider.to_api_provider(auth.as_ref().map(CodexAuth::auth_mode)) {
            Ok(provider) => provider,
            Err(err) => {
                emit_conversation_error(
                    sess,
                    &sub_id,
                    format!("failed to build provider: {err}"),
                    Some(CodexErrorInfo::Other),
                )
                .await;
                return;
            }
        };

        let auth_provider = match auth_provider_from_auth(auth, &provider) {
            Ok(auth_provider) => auth_provider,
            Err(err) => {
                emit_conversation_error(
                    sess,
                    &sub_id,
                    format!("failed to resolve auth: {err}"),
                    Some(CodexErrorInfo::Other),
                )
                .await;
                return;
            }
        };

        let mut extra_headers = HeaderMap::new();
        if let Some(token) = auth_provider.bearer_token()
            && let Ok(header) = http::HeaderValue::from_str(&format!("Bearer {token}"))
        {
            extra_headers.insert(http::header::AUTHORIZATION, header);
        }
        if let Some(account_id) = auth_provider.account_id()
            && let Ok(header) = http::HeaderValue::from_str(&account_id)
        {
            extra_headers.insert("ChatGPT-Account-ID", header);
        }

        let prompt = match backend_prompt {
            Some(prompt) => prompt,
            None => sess.get_base_instructions().await.text,
        };

        let config = RealtimeSessionConfig {
            api_url: api_url.unwrap_or_else(|| api_provider.base_url.clone()),
            prompt,
            session_id,
        };

        let client = RealtimeWebsocketClient::new(api_provider);
        let connection = match client
            .connect(config, extra_headers, default_headers())
            .await
        {
            Ok(connection) => connection,
            Err(err) => {
                let mapped_error = map_api_error(err);
                emit_conversation_error(
                    sess,
                    &sub_id,
                    format!("failed to open realtime conversation: {mapped_error}"),
                    Some(CodexErrorInfo::Other),
                )
                .await;
                return;
            }
        };

        let writer = connection.writer();
        let events = connection.events();
        let manager = Arc::clone(&sess.conversation);
        let sess_clone = Arc::clone(sess);
        let sub_id_clone = sub_id.clone();

        let events_task = tokio::spawn(async move {
            loop {
                match events.next_event().await {
                    Ok(Some(event)) => match event {
                        RealtimeEvent::SessionCreated { session_id } => {
                            sess_clone
                                .send_event_raw(Event {
                                    id: sub_id_clone.clone(),
                                    msg: EventMsg::ConversationStarted(ConversationStartedEvent {
                                        session_id,
                                    }),
                                })
                                .await;
                        }
                        RealtimeEvent::SessionUpdated { .. } => {}
                        RealtimeEvent::AudioOut(frame) => {
                            sess_clone
                                .send_event_raw(Event {
                                    id: sub_id_clone.clone(),
                                    msg: EventMsg::ConversationAudioOut(
                                        ConversationAudioOutEvent {
                                            frame:
                                                codex_protocol::protocol::ConversationAudioFrame {
                                                    data: frame.data,
                                                    sample_rate: frame.sample_rate,
                                                    num_channels: frame.num_channels,
                                                    samples_per_channel: frame.samples_per_channel,
                                                },
                                        },
                                    ),
                                })
                                .await;
                        }
                        RealtimeEvent::ConversationItemAdded(item) => {
                            sess_clone
                                .send_event_raw(Event {
                                    id: sub_id_clone.clone(),
                                    msg: EventMsg::ConversationItemAdded(
                                        ConversationItemAddedEvent { item },
                                    ),
                                })
                                .await;
                        }
                        RealtimeEvent::Error(message) => {
                            emit_conversation_error(
                                &sess_clone,
                                &sub_id_clone,
                                format!("realtime stream error: {message}"),
                                Some(CodexErrorInfo::Other),
                            )
                            .await;
                        }
                    },
                    Ok(None) => break,
                    Err(err) => {
                        let mapped_error = map_api_error(err);
                        emit_conversation_error(
                            &sess_clone,
                            &sub_id_clone,
                            format!("realtime stream closed: {mapped_error}"),
                            Some(CodexErrorInfo::Other),
                        )
                        .await;
                        break;
                    }
                }
            }

            if manager.clear_runtime_if_sub_id(&sub_id_clone).await {
                sess_clone
                    .send_event_raw(Event {
                        id: sub_id_clone,
                        msg: EventMsg::ConversationStopped(ConversationStoppedEvent),
                    })
                    .await;
            }
        });

        self.set_runtime(ConversationRuntime {
            sub_id,
            writer,
            events_task,
        })
        .await;
    }

    async fn writer(&self) -> Option<RealtimeWebsocketWriter> {
        let guard = self.runtime.lock().await;
        guard.as_ref().map(|runtime| runtime.writer.clone())
    }

    async fn set_runtime(&self, runtime: ConversationRuntime) {
        let mut guard = self.runtime.lock().await;
        *guard = Some(runtime);
    }

    async fn take_runtime(&self) -> Option<ConversationRuntime> {
        let mut guard = self.runtime.lock().await;
        guard.take()
    }

    async fn clear_runtime_if_sub_id(&self, sub_id: &str) -> bool {
        let mut guard = self.runtime.lock().await;
        if guard
            .as_ref()
            .is_some_and(|runtime| runtime.sub_id == sub_id)
        {
            let _ = guard.take();
            true
        } else {
            false
        }
    }

    async fn with_writer<F, Fut>(&self, sess: &Session, sub_id: &str, f: F)
    where
        F: FnOnce(RealtimeWebsocketWriter) -> Fut,
        Fut: std::future::Future<Output = Result<(), ApiError>>,
    {
        let Some(writer) = self.writer().await else {
            emit_conversation_error(
                sess,
                sub_id,
                "conversation is not running",
                Some(CodexErrorInfo::BadRequest),
            )
            .await;
            return;
        };

        if let Err(err) = f(writer).await {
            let mapped_error = map_api_error(err);
            emit_conversation_error(
                sess,
                sub_id,
                format!("failed to send conversation input: {mapped_error}"),
                Some(CodexErrorInfo::Other),
            )
            .await;
        }
    }
}

async fn emit_conversation_error(
    sess: &Session,
    sub_id: &str,
    message: impl Into<String>,
    codex_error_info: Option<CodexErrorInfo>,
) {
    sess.send_event_raw(Event {
        id: sub_id.to_string(),
        msg: EventMsg::Error(ErrorEvent {
            message: message.into(),
            codex_error_info,
        }),
    })
    .await;
}

async fn stop_conversation_runtime(runtime: ConversationRuntime) {
    let ConversationRuntime {
        writer,
        events_task,
        ..
    } = runtime;
    if let Err(err) = writer.close().await {
        let mapped_error = map_api_error(err);
        warn!(error = %mapped_error, "failed to close realtime websocket writer");
    }
    events_task.abort();
    let _ = events_task.await;
}
