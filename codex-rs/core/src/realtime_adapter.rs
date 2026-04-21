use crate::compact::content_items_to_text;
use crate::config::Config;
use crate::session::session::Session;
use codex_api::RealtimeEvent;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeWebsocketClient;
use codex_api::map_api_error;
use codex_app_server_protocol::AuthMode;
use codex_config::config_toml::RealtimeWsVersion;
use codex_login::default_client::default_headers;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
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
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeVoice;
use codex_thread_store::ListThreadsParams;
use codex_thread_store::SortDirection;
use codex_thread_store::StoredThread;
use codex_thread_store::ThreadSortKey;
use codex_thread_store::ThreadStore;
use dirs::home_dir;
use std::mem::take;
use std::sync::Arc;
use std::sync::atomic::Ordering;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

pub(crate) use codex_realtime::REALTIME_USER_TEXT_PREFIX;
pub(crate) use codex_realtime::RealtimeConversationManager;
use codex_realtime::RealtimeFeaturesConfig;
use codex_realtime::RealtimeStart;
use codex_realtime::RealtimeStartOutput;
use codex_realtime::build_realtime_session_config as build_session_config;
pub(crate) use codex_realtime::prefix_realtime_v2_text;
use codex_realtime::realtime_api_key;
use codex_realtime::realtime_delegation_from_handoff;
use codex_realtime::realtime_request_headers;
#[cfg(test)]
use codex_realtime::realtime_text_from_handoff_request;
#[cfg(test)]
use codex_realtime::wrap_realtime_delegation_input;

const MAX_RECENT_THREADS: usize = 40;
const REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET: usize = 5_300;

pub(crate) async fn build_realtime_startup_context(
    sess: &Session,
    budget_tokens: usize,
) -> Option<String> {
    let config = sess.get_config().await;
    let history = sess.clone_history().await;
    let current_thread_turns = current_thread_turns(history.raw_items());
    let recent_threads = load_recent_threads(sess).await;
    let context = codex_realtime::RealtimeStartupContext {
        cwd: config.cwd.clone(),
        current_thread_turns,
        recent_threads,
        user_root: home_dir(),
    };
    codex_realtime::build_realtime_startup_context(&context, budget_tokens).await
}

async fn load_recent_threads(sess: &Session) -> Vec<StoredThread> {
    match sess
        .services
        .thread_store
        .list_threads(ListThreadsParams {
            page_size: MAX_RECENT_THREADS,
            cursor: None,
            sort_key: ThreadSortKey::UpdatedAt,
            sort_direction: SortDirection::Desc,
            allowed_sources: Vec::new(),
            model_providers: None,
            archived: false,
            search_term: None,
        })
        .await
    {
        Ok(page) => page.items,
        Err(err) => {
            warn!("failed to load realtime startup threads from thread store: {err}");
            Vec::new()
        }
    }
}

fn current_thread_turns(items: &[ResponseItem]) -> Vec<codex_realtime::RealtimeStartupContextTurn> {
    let mut turns = Vec::new();
    let mut current_user = Vec::new();
    let mut current_assistant = Vec::new();

    for item in items {
        match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                if crate::event_mapping::is_contextual_user_message_content(content) {
                    continue;
                }
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if !current_user.is_empty() || !current_assistant.is_empty() {
                    turns.push(codex_realtime::RealtimeStartupContextTurn {
                        user_messages: take(&mut current_user),
                        assistant_messages: take(&mut current_assistant),
                    });
                }
                current_user.push(text);
            }
            ResponseItem::Message { role, content, .. } if role == "assistant" => {
                let Some(text) = content_items_to_text(content)
                    .map(|text| text.trim().to_string())
                    .filter(|text| !text.is_empty())
                else {
                    continue;
                };
                if current_user.is_empty() && current_assistant.is_empty() {
                    continue;
                }
                current_assistant.push(text);
            }
            _ => {}
        }
    }

    if !current_user.is_empty() || !current_assistant.is_empty() {
        turns.push(codex_realtime::RealtimeStartupContextTurn {
            user_messages: current_user,
            assistant_messages: current_assistant,
        });
    }

    turns
}

#[cfg(test)]
fn build_current_thread_section(items: &[ResponseItem]) -> Option<String> {
    codex_realtime::build_current_thread_section(&current_thread_turns(items))
}

#[cfg(test)]
pub(crate) use codex_realtime::CURRENT_THREAD_SECTION_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::NOTES_SECTION_TOKEN_BUDGET;
pub(crate) use codex_realtime::REALTIME_TURN_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::RECENT_WORK_SECTION_TOKEN_BUDGET;
#[cfg(test)]
pub(crate) use codex_realtime::STARTUP_CONTEXT_HEADER;
#[cfg(test)]
pub(crate) use codex_realtime::WORKSPACE_SECTION_TOKEN_BUDGET;
#[cfg(test)]
use codex_realtime::build_recent_work_section;
#[cfg(test)]
use codex_realtime::build_workspace_section_with_user_root;
#[cfg(test)]
use codex_realtime::format_section;
#[cfg(test)]
use codex_realtime::format_startup_context_blob;
pub(crate) use codex_realtime::truncate_realtime_text_to_token_budget;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RealtimeConversationEnd {
    Requested,
    TransportClosed,
    Error,
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
    requested_session_id: Option<String>,
    version: RealtimeWsVersion,
    start: RealtimeStart,
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
    let realtime_config = realtime_features_config(config.as_ref());
    let version = realtime_config.session.version;

    let mut api_provider = provider.to_api_provider(Some(AuthMode::ApiKey))?;
    if let Some(realtime_ws_base_url) = &realtime_config.websocket_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }

    let session_config = build_realtime_session_config(
        sess,
        &realtime_config,
        params.prompt,
        params.session_id,
        params.output_modality,
        params.voice,
    )
    .await?;
    let requested_session_id = session_config.session_id.clone();
    let event_parser = session_config.event_parser;
    let client = RealtimeWebsocketClient::new(api_provider);

    let start = match transport {
        ConversationStartTransport::Websocket => {
            let api_key = realtime_api_key(auth.as_ref(), &provider)?;
            let extra_headers =
                realtime_request_headers(requested_session_id.as_deref(), Some(api_key.as_str()))?
                    .unwrap_or_default();
            let connection = client
                .connect(session_config, extra_headers, default_headers())
                .await
                .map_err(map_api_error)?;
            RealtimeStart {
                writer: connection.writer(),
                events: connection.events(),
                event_parser,
                sdp: None,
            }
        }
        ConversationStartTransport::Webrtc { sdp } => {
            let extra_headers =
                realtime_request_headers(requested_session_id.as_deref(), /*api_key*/ None)?
                    .unwrap_or_default();
            let call = sess
                .services
                .model_client
                .create_realtime_call_with_headers(sdp, session_config.clone(), extra_headers)
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
            RealtimeStart {
                writer: connection.writer(),
                events: connection.events(),
                event_parser,
                sdp: Some(call.sdp),
            }
        }
    };

    Ok(PreparedRealtimeConversationStart {
        requested_session_id,
        version,
        start,
    })
}

pub(crate) async fn build_realtime_session_config(
    sess: &Arc<Session>,
    config: &RealtimeFeaturesConfig,
    prompt: Option<Option<String>>,
    session_id: Option<String>,
    output_modality: RealtimeOutputModality,
    voice: Option<RealtimeVoice>,
) -> CodexResult<RealtimeSessionConfig> {
    let startup_context = match &config.websocket_startup_context {
        Some(startup_context) => startup_context.clone(),
        None => {
            build_realtime_startup_context(sess.as_ref(), REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET)
                .await
                .unwrap_or_default()
        }
    };
    build_session_config(
        config,
        prompt,
        Some(session_id.unwrap_or_else(|| sess.conversation_id.to_string())),
        output_modality,
        voice,
        startup_context,
    )
}

fn realtime_features_config(config: &Config) -> RealtimeFeaturesConfig {
    RealtimeFeaturesConfig {
        audio: config.realtime_audio.clone(),
        session: config.realtime.clone(),
        websocket_base_url: config.experimental_realtime_ws_base_url.clone(),
        websocket_model: config.experimental_realtime_ws_model.clone(),
        websocket_backend_prompt: config.experimental_realtime_ws_backend_prompt.clone(),
        websocket_startup_context: config.experimental_realtime_ws_startup_context.clone(),
        start_instructions: config.experimental_realtime_start_instructions.clone(),
    }
}

async fn handle_start_inner(
    sess: &Arc<Session>,
    sub_id: &str,
    prepared_start: PreparedRealtimeConversationStart,
) -> CodexResult<()> {
    let PreparedRealtimeConversationStart {
        requested_session_id,
        version,
        start,
    } = prepared_start;
    info!("starting realtime conversation");
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
                    info!(event = ?event, "received realtime conversation event");
                }
            }
            if let RealtimeEvent::Error(_) = &event {
                end = RealtimeConversationEnd::Error;
            }
            if let Some(text) = match &event {
                RealtimeEvent::HandoffRequested(handoff) => {
                    realtime_delegation_from_handoff(handoff)
                }
                _ => None,
            } {
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
mod conversation_tests;

#[cfg(test)]
#[path = "realtime_context_tests.rs"]
mod context_tests;
