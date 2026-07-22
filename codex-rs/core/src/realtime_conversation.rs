use crate::client::ModelClient;
use crate::context::ContextualUserFragment;
use crate::context::RealtimeDelegation;
use crate::context::RealtimeDelegationSource;
use crate::realtime_context::build_realtime_startup_context;
use crate::realtime_context::truncate_realtime_text_to_token_budget;
use crate::realtime_prompt::prepare_realtime_backend_prompt;
use crate::session::session::Session;
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
use codex_api::RealtimeContextAppendChannel;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeSessionConfig;
use codex_api::RealtimeSessionMode;
use codex_api::RealtimeWebsocketClient;
use codex_api::RealtimeWebsocketEvents;
use codex_api::RealtimeWebsocketWriter;
use codex_api::build_session_headers;
use codex_api::map_api_error;
use codex_config::config_toml::RealtimeWsMode;
use codex_config::config_toml::RealtimeWsVersion;
use codex_login::CodexAuth;
use codex_login::default_client::add_originator_header;
use codex_login::default_client::default_headers;
use codex_login::read_openai_api_key_from_env;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::auth::AuthMode;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::MessagePhase;
use codex_protocol::protocol::CodexErrorInfo;
use codex_protocol::protocol::CodexResponseHandoffMode;
use codex_protocol::protocol::ConversationAudioParams;
use codex_protocol::protocol::ConversationSpeechParams;
use codex_protocol::protocol::ConversationStartParams;
use codex_protocol::protocol::ConversationStartTransport;
use codex_protocol::protocol::ConversationTextParams;
use codex_protocol::protocol::ConversationTextRole;
use codex_protocol::protocol::ErrorEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeConversationClosedEvent;
use codex_protocol::protocol::RealtimeConversationRealtimeEvent;
use codex_protocol::protocol::RealtimeConversationSdpEvent;
use codex_protocol::protocol::RealtimeConversationStartedEvent;
use codex_protocol::protocol::RealtimeHandoffRequested;
use codex_protocol::protocol::RealtimeOutputModality;
use codex_protocol::protocol::RealtimeTranscriptEntry;
use codex_protocol::protocol::RealtimeVoice;
use codex_protocol::protocol::RealtimeVoicesList;
use codex_utils_output_truncation::approx_bytes_for_tokens;
use codex_utils_string::approx_token_count;
use codex_utils_string::take_bytes_at_char_boundary;
use http::HeaderMap;
use http::HeaderValue;
use http::header::AUTHORIZATION;
use serde_json::json;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use std::time::Duration;
use std::time::Instant;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::warn;

mod bem;

use self::bem::ChannelParser as BemChannelParser;
use self::bem::message_phase as bem_message_phase;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const TEXT_IN_QUEUE_CAPACITY: usize = 64;
const HANDOFF_OUT_QUEUE_CAPACITY: usize = 64;
const OUTPUT_EVENTS_QUEUE_CAPACITY: usize = 256;
const REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET: usize = 5_300;
const REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET: usize = 1_000;
const REALTIME_INITIAL_ITEMS_MAX_COUNT: usize = 128;
const REALTIME_INITIAL_ITEMS_MAX_TOKENS: usize = 8_192;
const HANDOFF_STREAM_FLUSH_INTERVAL: Duration = Duration::from_millis(200);
const HANDOFF_STREAM_TRUNCATION_MARKER: &str = "\n…output truncated…\n";
const AGENT_FINAL_MESSAGE_PREFIX: &str = "\"Agent Final Message\":\n\n";
const STANDALONE_HANDOFF_ID: &str = "codex";
const DEFAULT_REALTIME_MODEL: &str = "gpt-realtime-1.5";
const DEFAULT_FRAMELESS_REALTIME_MODEL: &str = "gpt-live-1-boulder-alpha";
pub(crate) const REALTIME_USER_TEXT_PREFIX: &str = "[USER] ";
pub(crate) const REALTIME_BACKEND_TEXT_PREFIX: &str = "[BACKEND] ";
const REALTIME_V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT: &str =
    "Background agent finished. Use the preceding [BACKEND] messages as the result.";
const REALTIME_V2_STEER_ACKNOWLEDGEMENT: &str =
    "This was sent to steer the previous background agent task.";
const REALTIME_ACTIVE_RESPONSE_ERROR_PREFIX: &str =
    "Conversation already has an active response in progress:";
const REALTIME_SESSION_ENDED_HANDOFF_INSTRUCTION: &str = "The user just ended their realtime session. Here is the remaining handoff/transcript tail. You probably do not have to do anything; acknowledge the handoff unless the transcript itself asks for something.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RealtimeConversationEnd {
    Requested,
    TransportClosed,
    Error,
}

enum RealtimeFanoutTaskStop {
    Await,
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
    output_tx: Sender<RealtimeOutbound>,
    last_output: Arc<Mutex<Option<RealtimeHandoffOutput>>>,
    stream: Arc<Mutex<RealtimeHandoffStreamState>>,
    client_managed_handoffs: bool,
    codex_responses_as_items: bool,
    codex_response_item_prefix: Option<String>,
    codex_response_handoff_mode: CodexResponseHandoffMode,
    codex_response_handoff_channel_prefixes: Arc<BTreeMap<String, Vec<String>>>,
    session_kind: RealtimeSessionKind,
    event_parser: RealtimeEventParser,
}

#[derive(Clone, Debug)]
struct RealtimeHandoffOutput {
    text: String,
    phase: Option<MessagePhase>,
}

#[derive(Debug, Default)]
struct RealtimeHandoffStreamState {
    active_handoff: Option<String>,
    items: HashMap<String, RealtimeStreamedItem>,
}

#[derive(Debug)]
struct RealtimeStreamedItem {
    handoff_id: String,
    phase: Option<MessagePhase>,
    bem_channel_parser: Option<BemChannelParser>,
    prefix_final_message: bool,
    sent_bytes: usize,
    buffered_text: String,
    tail_text: String,
    truncated: bool,
    last_flush_at: Instant,
    flush_scheduled: bool,
}

impl RealtimeStreamedItem {
    fn next_flush_delay(&self) -> Duration {
        HANDOFF_STREAM_FLUSH_INTERVAL.saturating_sub(self.last_flush_at.elapsed())
    }

    fn output_prefix(&self) -> &'static str {
        if self.prefix_final_message
            && self.sent_bytes == 0
            && !matches!(self.phase, Some(MessagePhase::Commentary))
        {
            AGENT_FINAL_MESSAGE_PREFIX
        } else {
            ""
        }
    }

    fn stream_head_byte_limit(&self) -> usize {
        let output_byte_limit = approx_bytes_for_tokens(REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET);
        output_byte_limit.saturating_sub(HANDOFF_STREAM_TRUNCATION_MARKER.len()) / 2
    }

    fn tail_byte_limit(&self) -> usize {
        approx_bytes_for_tokens(REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET)
            .saturating_sub(self.stream_head_byte_limit())
            .saturating_sub(HANDOFF_STREAM_TRUNCATION_MARKER.len())
    }

    fn streamable_text_bytes(&self) -> usize {
        self.stream_head_byte_limit()
            .saturating_sub(self.sent_bytes)
            .saturating_sub(self.output_prefix().len())
    }

    fn push_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }

        let text = if let Some(parser) = self.bem_channel_parser.as_mut() {
            let Some(text) = parser.push(text) else {
                return;
            };
            self.phase = parser.phase();
            text
        } else {
            text.to_string()
        };
        self.push_output_text(&text);
    }

    fn finish_input(&mut self) {
        let Some(parser) = self.bem_channel_parser.as_mut() else {
            return;
        };
        self.phase = parser.phase();
        let text = parser.finish();
        if self.phase.is_none() && !text.is_empty() {
            warn!("BEM output ended before a recognized channel header was received");
            self.phase = Some(MessagePhase::FinalAnswer);
        }
        self.push_output_text(&text);
    }

    fn push_output_text(&mut self, text: &str) {
        if text.is_empty() {
            return;
        }
        if self.truncated {
            self.tail_text.push_str(text);
            self.tail_text =
                take_last_bytes_at_char_boundary(&self.tail_text, self.tail_byte_limit())
                    .to_string();
            return;
        }

        self.buffered_text.push_str(text);
        let output_byte_limit = approx_bytes_for_tokens(REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET);
        let remaining_text_bytes = output_byte_limit
            .saturating_sub(self.sent_bytes)
            .saturating_sub(self.output_prefix().len());
        if self.buffered_text.len() <= remaining_text_bytes {
            return;
        }

        let head_bytes =
            take_bytes_at_char_boundary(&self.buffered_text, self.streamable_text_bytes()).len();
        self.tail_text =
            take_last_bytes_at_char_boundary(&self.buffered_text, self.tail_byte_limit())
                .to_string();
        self.buffered_text.truncate(head_bytes);
        self.truncated = true;
    }

    fn drain_stream_chunk(&mut self) -> Option<String> {
        let prefix = self.output_prefix();
        let available_text_bytes = self.streamable_text_bytes();
        if self.buffered_text.is_empty() || available_text_bytes == 0 {
            return None;
        }

        let requested_bytes = available_text_bytes.min(self.buffered_text.len());
        let split_at = take_bytes_at_char_boundary(&self.buffered_text, requested_bytes).len();
        if split_at == 0 {
            return None;
        }
        let text = self.buffered_text.drain(..split_at).collect::<String>();
        let text = format!("{prefix}{text}");
        self.sent_bytes += text.len();
        Some(text)
    }

    fn drain_final_chunk(&mut self) -> Option<String> {
        let prefix = self.output_prefix();
        if !self.truncated {
            if self.buffered_text.is_empty() {
                return None;
            }
            let text = self.buffered_text.drain(..).collect::<String>();
            let text = format!("{prefix}{text}");
            self.sent_bytes += text.len();
            return Some(text);
        }

        let head = self.buffered_text.drain(..).collect::<String>();
        let tail = self.tail_text.drain(..).collect::<String>();
        let text = format!("{prefix}{head}{HANDOFF_STREAM_TRUNCATION_MARKER}{tail}");
        self.sent_bytes += text.len();
        Some(text)
    }
}

fn take_last_bytes_at_char_boundary(text: &str, max_bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(max_bytes);
    while start < text.len() && !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}

#[derive(Debug, PartialEq, Eq)]
enum RealtimeOutbound {
    StandaloneHandoff {
        text: String,
        phase: Option<MessagePhase>,
    },
    StandaloneSpeech {
        text: String,
    },
    HandoffUpdate {
        handoff_id: String,
        text: String,
        phase: Option<MessagePhase>,
    },
    HandoffAppend {
        handoff_id: String,
        text: String,
        phase: Option<MessagePhase>,
    },
    CompletedHandoff {
        handoff_id: String,
        text: String,
        phase: Option<MessagePhase>,
    },
    ConversationItem {
        text: String,
        phase: Option<MessagePhase>,
    },
    HandoffCompleteAck {
        handoff_id: String,
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
    text_rx: Receiver<ConversationTextParams>,
    handoff_output_rx: Receiver<RealtimeOutbound>,
    audio_rx: Receiver<RealtimeAudioFrame>,
    events_tx: Sender<RealtimeEvent>,
    handoff_state: RealtimeHandoffState,
    session_kind: RealtimeSessionKind,
    event_parser: RealtimeEventParser,
    flush_transcript_tail_on_session_end: bool,
    transcript_tail_tx: Sender<String>,
    stop_token: CancellationToken,
}

struct RealtimeInputChannels {
    text_rx: Receiver<ConversationTextParams>,
    handoff_output_rx: Receiver<RealtimeOutbound>,
    audio_rx: Receiver<RealtimeAudioFrame>,
}

impl RealtimeHandoffState {
    fn streams_handoff_append(&self) -> bool {
        self.event_parser == RealtimeEventParser::FramelessBidi
            && !self.client_managed_handoffs
            && !self.codex_responses_as_items
    }

    fn routes_handoff_by_bem(&self) -> bool {
        self.event_parser == RealtimeEventParser::FramelessBidi
            && self.codex_response_handoff_mode == CodexResponseHandoffMode::BemTags
    }
}

#[allow(dead_code)]
struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    text_tx: Sender<ConversationTextParams>,
    session_kind: RealtimeSessionKind,
    handoff: RealtimeHandoffState,
    input_task: JoinHandle<()>,
    fanout_task: Option<JoinHandle<()>>,
    realtime_active: Arc<AtomicBool>,
    stop_token: CancellationToken,
}

struct RealtimeStart {
    api_provider: ApiProvider,
    extra_headers: Option<HeaderMap>,
    client_managed_handoffs: bool,
    flush_transcript_tail_on_session_end: bool,
    codex_responses_as_items: bool,
    codex_response_item_prefix: Option<String>,
    codex_response_handoff_mode: CodexResponseHandoffMode,
    codex_response_handoff_channel_prefixes: Option<BTreeMap<String, Vec<String>>>,
    realtime_call_api_provider: Option<ApiProvider>,
    session_config: RealtimeSessionConfig,
    model_client: ModelClient,
    sdp: Option<String>,
}

struct RealtimeStartOutput {
    realtime_active: Arc<AtomicBool>,
    events_rx: Receiver<RealtimeEvent>,
    transcript_tail_rx: Receiver<String>,
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

    pub(crate) async fn is_running_v2(&self) -> bool {
        let state = self.state.lock().await;
        matches!(
            state.as_ref(),
            Some(state)
                if state.realtime_active.load(Ordering::Relaxed)
                    && state.session_kind == RealtimeSessionKind::V2
        )
    }

    async fn start(&self, start: RealtimeStart) -> CodexResult<RealtimeStartOutput> {
        let previous_state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };
        if let Some(state) = previous_state {
            stop_conversation_state(state, RealtimeFanoutTaskStop::Await).await;
        }

        self.start_inner(start).await
    }

    async fn start_inner(&self, start: RealtimeStart) -> CodexResult<RealtimeStartOutput> {
        let RealtimeStart {
            api_provider,
            extra_headers,
            client_managed_handoffs,
            flush_transcript_tail_on_session_end,
            codex_responses_as_items,
            codex_response_item_prefix,
            codex_response_handoff_mode,
            codex_response_handoff_channel_prefixes,
            realtime_call_api_provider,
            session_config,
            model_client,
            sdp,
        } = start;
        let event_parser = session_config.event_parser;
        let session_kind = match event_parser {
            RealtimeEventParser::V1 | RealtimeEventParser::FramelessBidi => RealtimeSessionKind::V1,
            RealtimeEventParser::RealtimeV2 => RealtimeSessionKind::V2,
        };

        let (audio_tx, audio_rx) =
            async_channel::bounded::<RealtimeAudioFrame>(AUDIO_IN_QUEUE_CAPACITY);
        let (text_tx, text_rx) =
            async_channel::bounded::<ConversationTextParams>(TEXT_IN_QUEUE_CAPACITY);
        let (handoff_output_tx, handoff_output_rx) =
            async_channel::bounded::<RealtimeOutbound>(HANDOFF_OUT_QUEUE_CAPACITY);
        let (events_tx, events_rx) =
            async_channel::bounded::<RealtimeEvent>(OUTPUT_EVENTS_QUEUE_CAPACITY);
        let (transcript_tail_tx, transcript_tail_rx) = async_channel::bounded::<String>(1);

        let realtime_active = Arc::new(AtomicBool::new(true));
        let stop_token = CancellationToken::new();
        let handoff = RealtimeHandoffState {
            output_tx: handoff_output_tx,
            last_output: Arc::new(Mutex::new(None)),
            stream: Arc::new(Mutex::new(RealtimeHandoffStreamState::default())),
            client_managed_handoffs,
            codex_responses_as_items,
            codex_response_item_prefix,
            codex_response_handoff_mode,
            codex_response_handoff_channel_prefixes: Arc::new(
                codex_response_handoff_channel_prefixes.unwrap_or_default(),
            ),
            session_kind,
            event_parser,
        };
        let input_channels = RealtimeInputChannels {
            text_rx,
            handoff_output_rx,
            audio_rx,
        };

        let client = RealtimeWebsocketClient::new(api_provider);
        let (task, sdp) = if let Some(sdp) = sdp {
            let call = model_client
                .create_realtime_call_with_headers(
                    sdp,
                    session_config.clone(),
                    extra_headers.unwrap_or_default(),
                    realtime_call_api_provider,
                )
                .await?;
            let task = spawn_webrtc_sideband_input_task(RealtimeWebrtcSidebandInputTask {
                client,
                session_config,
                call_id: call.call_id,
                sideband_headers: call.sideband_headers,
                input_channels,
                events_tx,
                handoff_state: handoff.clone(),
                session_kind,
                event_parser,
                realtime_active: Arc::clone(&realtime_active),
                flush_transcript_tail_on_session_end,
                transcript_tail_tx,
                stop_token: stop_token.clone(),
            });
            (task, Some(call.sdp))
        } else {
            let connection = client
                .connect(
                    session_config,
                    extra_headers.unwrap_or_default(),
                    default_headers(),
                )
                .await
                .map_err(map_api_error)?;
            let task = spawn_realtime_input_task(RealtimeInputTask {
                writer: connection.writer(),
                events: connection.events(),
                text_rx: input_channels.text_rx,
                handoff_output_rx: input_channels.handoff_output_rx,
                audio_rx: input_channels.audio_rx,
                events_tx,
                handoff_state: handoff.clone(),
                session_kind,
                event_parser,
                flush_transcript_tail_on_session_end,
                transcript_tail_tx,
                stop_token: stop_token.clone(),
            });
            (task, None)
        };

        let mut guard = self.state.lock().await;
        *guard = Some(ConversationState {
            audio_tx,
            text_tx,
            session_kind,
            handoff,
            input_task: task,
            fanout_task: None,
            realtime_active: Arc::clone(&realtime_active),
            stop_token,
        });
        Ok(RealtimeStartOutput {
            realtime_active,
            events_rx,
            transcript_tail_rx,
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

    pub(crate) async fn text_in(&self, mut params: ConversationTextParams) -> CodexResult<()> {
        let sender = {
            let guard = self.state.lock().await;
            guard
                .as_ref()
                .map(|state| (state.text_tx.clone(), state.session_kind))
        };

        let Some((sender, session_kind)) = sender else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };

        if params.role == ConversationTextRole::User {
            params.text =
                prefix_realtime_text(params.text, REALTIME_USER_TEXT_PREFIX, session_kind);
        }
        sender
            .send(params)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub(crate) async fn handoff_out(
        &self,
        output_text: String,
        phase: Option<MessagePhase>,
    ) -> CodexResult<()> {
        let handoff = {
            let guard = self.state.lock().await;
            let Some(state) = guard.as_ref() else {
                return Err(CodexErr::InvalidRequest(
                    "conversation is not running".to_string(),
                ));
            };
            state.handoff.clone()
        };

        if handoff.client_managed_handoffs {
            return Ok(());
        }
        let phase = if handoff.routes_handoff_by_bem() {
            match bem_message_phase(
                &output_text,
                &handoff.codex_response_handoff_channel_prefixes,
            ) {
                Some(phase) => Some(phase),
                None => {
                    warn!("BEM output did not contain a recognized channel header");
                    Some(MessagePhase::FinalAnswer)
                }
            }
        } else {
            phase
        };
        let is_commentary = matches!(phase, Some(MessagePhase::Commentary));
        let active_handoff = handoff.stream.lock().await.active_handoff.clone();
        let output = match active_handoff {
            Some(handoff_id) => {
                let output_text = realtime_backend_output(output_text, handoff.session_kind);
                *handoff.last_output.lock().await = Some(RealtimeHandoffOutput {
                    text: output_text.clone(),
                    phase: phase.clone(),
                });
                if handoff.codex_responses_as_items {
                    RealtimeOutbound::ConversationItem {
                        text: realtime_backend_item(
                            output_text,
                            handoff.codex_response_item_prefix.as_deref(),
                        ),
                        phase,
                    }
                } else if handoff.event_parser == RealtimeEventParser::V1 && is_commentary {
                    RealtimeOutbound::HandoffAppend {
                        handoff_id,
                        text: output_text,
                        phase,
                    }
                } else {
                    RealtimeOutbound::HandoffUpdate {
                        handoff_id,
                        text: output_text,
                        phase,
                    }
                }
            }
            None if output_text.trim().is_empty() => return Ok(()),
            None => {
                let output_text = realtime_backend_output(output_text, handoff.session_kind);
                if handoff.codex_responses_as_items {
                    RealtimeOutbound::ConversationItem {
                        text: realtime_backend_item(
                            output_text,
                            handoff.codex_response_item_prefix.as_deref(),
                        ),
                        phase,
                    }
                } else {
                    RealtimeOutbound::StandaloneHandoff {
                        text: if handoff.event_parser == RealtimeEventParser::V1 && !is_commentary {
                            format!("{AGENT_FINAL_MESSAGE_PREFIX}{output_text}")
                        } else {
                            output_text
                        },
                        phase,
                    }
                }
            }
        };
        handoff
            .output_tx
            .send(output)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub(crate) async fn register_handoff_stream_item(
        &self,
        item_id: String,
        phase: Option<MessagePhase>,
        initial_text: String,
    ) {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        let Some(handoff) = handoff else {
            return;
        };
        if !handoff.streams_handoff_append() {
            return;
        }
        let flush_delay = {
            let mut stream = handoff.stream.lock().await;
            let Some(handoff_id) = stream.active_handoff.clone() else {
                return;
            };
            let mut streamed_item = RealtimeStreamedItem {
                handoff_id,
                phase: if handoff.routes_handoff_by_bem() {
                    None
                } else {
                    phase
                },
                bem_channel_parser: handoff.routes_handoff_by_bem().then(|| {
                    BemChannelParser::new(Arc::clone(
                        &handoff.codex_response_handoff_channel_prefixes,
                    ))
                }),
                prefix_final_message: handoff.event_parser == RealtimeEventParser::V1,
                sent_bytes: 0,
                buffered_text: String::new(),
                tail_text: String::new(),
                truncated: false,
                last_flush_at: Instant::now(),
                flush_scheduled: false,
            };
            streamed_item.push_text(&initial_text);
            let flush_delay = if streamed_item.buffered_text.is_empty() {
                None
            } else {
                streamed_item.flush_scheduled = true;
                Some(streamed_item.next_flush_delay())
            };
            stream.items.insert(item_id.clone(), streamed_item);
            flush_delay
        };
        if let Some(flush_delay) = flush_delay {
            schedule_streamed_handoff_flush(&handoff, item_id, flush_delay);
        }
    }

    pub(crate) async fn stream_handoff_delta(
        &self,
        item_id: &str,
        delta: String,
    ) -> CodexResult<()> {
        if delta.is_empty() {
            return Ok(());
        }
        let handoff = {
            let guard = self.state.lock().await;
            let Some(state) = guard.as_ref() else {
                return Err(CodexErr::InvalidRequest(
                    "conversation is not running".to_string(),
                ));
            };
            state.handoff.clone()
        };
        if !handoff.streams_handoff_append() {
            return Ok(());
        }
        let flush_delay = {
            let mut stream = handoff.stream.lock().await;
            let Some(streamed_item) = stream.items.get_mut(item_id) else {
                return Ok(());
            };
            streamed_item.push_text(&delta);
            if streamed_item.flush_scheduled || streamed_item.streamable_text_bytes() == 0 {
                None
            } else {
                streamed_item.flush_scheduled = true;
                Some(streamed_item.next_flush_delay())
            }
        };
        if let Some(flush_delay) = flush_delay {
            schedule_streamed_handoff_flush(&handoff, item_id.to_string(), flush_delay);
        }
        Ok(())
    }

    pub(crate) async fn finish_handoff_stream_item(&self, item_id: &str) -> bool {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        let Some(handoff) = handoff else {
            return false;
        };
        if !handoff.streams_handoff_append() {
            return false;
        }
        let Some(mut streamed_item) = handoff.stream.lock().await.items.remove(item_id) else {
            return false;
        };
        streamed_item.finish_input();
        let chunk = streamed_item.drain_final_chunk();
        let sent_output = streamed_item.sent_bytes > 0;
        if let Some(text) = chunk {
            let _ = handoff
                .output_tx
                .send(RealtimeOutbound::HandoffAppend {
                    handoff_id: streamed_item.handoff_id,
                    text,
                    phase: streamed_item.phase,
                })
                .await;
        }
        sent_output
    }

    pub(crate) async fn append_speech(&self, text: String) -> CodexResult<()> {
        if text.trim().is_empty() {
            return Ok(());
        }

        let handoff = {
            let guard = self.state.lock().await;
            let Some(state) = guard.as_ref() else {
                return Err(CodexErr::InvalidRequest(
                    "conversation is not running".to_string(),
                ));
            };
            state.handoff.clone()
        };

        handoff
            .output_tx
            .send(RealtimeOutbound::StandaloneSpeech {
                text: realtime_backend_output(text, handoff.session_kind),
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
        if handoff.client_managed_handoffs {
            return Ok(());
        }
        match handoff.session_kind {
            RealtimeSessionKind::V1 => return Ok(()),
            RealtimeSessionKind::V2 => {}
        }

        let Some(handoff_id) = handoff.stream.lock().await.active_handoff.clone() else {
            return Ok(());
        };
        let Some(last_output) = handoff.last_output.lock().await.clone() else {
            return Ok(());
        };

        let output = if handoff.codex_responses_as_items {
            RealtimeOutbound::HandoffCompleteAck { handoff_id }
        } else {
            RealtimeOutbound::CompletedHandoff {
                handoff_id,
                text: last_output.text,
                phase: last_output.phase,
            }
        };

        handoff
            .output_tx
            .send(output)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))
    }

    pub(crate) async fn clear_active_handoff(&self) {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        if let Some(handoff) = handoff {
            {
                let mut stream = handoff.stream.lock().await;
                stream.active_handoff = None;
                stream.items.clear();
            }
            *handoff.last_output.lock().await = None;
        }
    }

    pub(crate) async fn shutdown(&self) -> CodexResult<()> {
        let state = {
            let mut guard = self.state.lock().await;
            guard.take()
        };

        if let Some(state) = state {
            stop_conversation_state(state, RealtimeFanoutTaskStop::Await).await;
        }
        Ok(())
    }
}

async fn stop_conversation_state(
    mut state: ConversationState,
    fanout_task_stop: RealtimeFanoutTaskStop,
) {
    state.realtime_active.store(false, Ordering::Relaxed);
    state.stop_token.cancel();
    let _ = state.input_task.await;

    if let Some(fanout_task) = state.fanout_task.take() {
        match fanout_task_stop {
            RealtimeFanoutTaskStop::Await => {
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
    client_managed_handoffs: bool,
    flush_transcript_tail_on_session_end: bool,
    codex_responses_as_items: bool,
    codex_response_item_prefix: Option<String>,
    codex_response_handoff_mode: CodexResponseHandoffMode,
    codex_response_handoff_channel_prefixes: Option<BTreeMap<String, Vec<String>>>,
    realtime_call_api_provider: Option<ApiProvider>,
    requested_realtime_session_id: Option<String>,
    version: RealtimeWsVersion,
    session_config: RealtimeSessionConfig,
    transport: ConversationStartTransport,
}

#[derive(Clone, Copy)]
pub(crate) enum ConfiguredRealtimeVoice {
    Use,
    Ignore,
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
        .clone()
        .unwrap_or(ConversationStartTransport::Websocket);
    let mut api_provider = provider.to_api_provider(Some(AuthMode::ApiKey))?;
    if let Some(realtime_ws_base_url) = &config.experimental_realtime_ws_base_url {
        api_provider.base_url = realtime_ws_base_url.clone();
    }
    let realtime_call_api_provider =
        if let Some(realtime_call_base_url) = &config.experimental_realtime_webrtc_call_base_url {
            let mut api_provider = provider.to_api_provider(Some(AuthMode::ApiKey))?;
            api_provider.base_url = realtime_call_base_url.clone();
            Some(api_provider)
        } else {
            None
        };
    let version = params.version.unwrap_or(match &transport {
        ConversationStartTransport::Websocket => config.realtime.version,
        ConversationStartTransport::Webrtc { .. } => RealtimeWsVersion::V1,
    });
    if matches!(transport, ConversationStartTransport::Webrtc { .. }) {
        validate_avas_webrtc_start(version, config.realtime.session_type)?;
    }
    let configured_voice = match (&transport, params.version) {
        (ConversationStartTransport::Webrtc { .. }, None) => ConfiguredRealtimeVoice::Ignore,
        (ConversationStartTransport::Webrtc { .. } | ConversationStartTransport::Websocket, _) => {
            ConfiguredRealtimeVoice::Use
        }
    };
    let session_config =
        build_realtime_session_config(sess, &params, version, configured_voice).await?;
    let requested_realtime_session_id = session_config.session_id.clone();
    let event_parser = session_config.event_parser;
    let originator = sess.originator().await;
    let mut extra_headers = match transport {
        ConversationStartTransport::Websocket => {
            let realtime_api_key = realtime_api_key(auth.as_ref(), &provider)?;
            realtime_request_headers(
                requested_realtime_session_id.as_deref(),
                Some(realtime_api_key.as_str()),
                event_parser,
                originator.as_str(),
            )?
        }
        ConversationStartTransport::Webrtc { .. } => {
            realtime_request_headers(
                requested_realtime_session_id.as_deref(),
                /*api_key*/ None,
                event_parser,
                originator.as_str(),
            )?
        }
    }
    .unwrap_or_default();
    extra_headers.extend(build_session_headers(
        Some(sess.session_id().to_string()),
        Some(sess.thread_id().to_string()),
    ));
    Ok(PreparedRealtimeConversationStart {
        api_provider,
        extra_headers: Some(extra_headers),
        client_managed_handoffs: params.client_managed_handoffs,
        flush_transcript_tail_on_session_end: params.flush_transcript_tail_on_session_end,
        codex_responses_as_items: params.codex_responses_as_items,
        codex_response_item_prefix: params.codex_response_item_prefix,
        codex_response_handoff_mode: params.codex_response_handoff_mode,
        codex_response_handoff_channel_prefixes: params.codex_response_handoff_channel_prefixes,
        realtime_call_api_provider,
        requested_realtime_session_id,
        version,
        session_config,
        transport,
    })
}

fn validate_avas_webrtc_start(
    version: RealtimeWsVersion,
    session_type: RealtimeWsMode,
) -> CodexResult<()> {
    if version == RealtimeWsVersion::V2 {
        return Err(CodexErr::InvalidRequest(
            "AVAS realtime calls require realtime v1 or v3".to_string(),
        ));
    }
    if session_type != RealtimeWsMode::Conversational {
        return Err(CodexErr::InvalidRequest(
            "AVAS realtime calls require conversational realtime".to_string(),
        ));
    }
    Ok(())
}

pub(crate) async fn build_realtime_session_config(
    sess: &Arc<Session>,
    params: &ConversationStartParams,
    version: RealtimeWsVersion,
    configured_voice: ConfiguredRealtimeVoice,
) -> CodexResult<RealtimeSessionConfig> {
    let config = sess.get_config().await;
    let prompt = prepare_realtime_backend_prompt(
        params.prompt.clone(),
        config.experimental_realtime_ws_backend_prompt.clone(),
    );
    let startup_context = if params.include_startup_context {
        match config.experimental_realtime_ws_startup_context.clone() {
            Some(startup_context) => startup_context,
            None => {
                build_realtime_startup_context(sess.as_ref(), REALTIME_STARTUP_CONTEXT_TOKEN_BUDGET)
                    .await
                    .unwrap_or_default()
            }
        }
    } else {
        String::new()
    };
    let prompt = match (prompt.is_empty(), startup_context.is_empty()) {
        (true, true) => String::new(),
        (true, false) => startup_context,
        (false, true) => prompt,
        (false, false) => format!("{prompt}\n\n{startup_context}"),
    };
    if version != RealtimeWsVersion::V3 && !params.initial_items.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "initial realtime items require realtime v3".to_string(),
        ));
    }
    if params.initial_items.len() > REALTIME_INITIAL_ITEMS_MAX_COUNT {
        return Err(CodexErr::InvalidRequest(format!(
            "initial realtime items must contain no more than {REALTIME_INITIAL_ITEMS_MAX_COUNT} items"
        )));
    }
    let mut total_initial_item_tokens: usize = 0;
    for item in &params.initial_items {
        let item_tokens = approx_token_count(&item.text);
        if item_tokens > REALTIME_INITIAL_ITEMS_MAX_TOKENS {
            return Err(CodexErr::InvalidRequest(format!(
                "each initial realtime item must not exceed {REALTIME_INITIAL_ITEMS_MAX_TOKENS} estimated tokens"
            )));
        }
        total_initial_item_tokens = total_initial_item_tokens.saturating_add(item_tokens);
    }
    if total_initial_item_tokens > REALTIME_INITIAL_ITEMS_MAX_TOKENS {
        return Err(CodexErr::InvalidRequest(format!(
            "initial realtime items must not exceed {REALTIME_INITIAL_ITEMS_MAX_TOKENS} estimated tokens in total"
        )));
    }
    let model = Some(
        params
            .model
            .clone()
            .or_else(|| config.experimental_realtime_ws_model.clone())
            .unwrap_or_else(|| match version {
                RealtimeWsVersion::V1 | RealtimeWsVersion::V2 => DEFAULT_REALTIME_MODEL.to_string(),
                RealtimeWsVersion::V3 => DEFAULT_FRAMELESS_REALTIME_MODEL.to_string(),
            }),
    );
    let event_parser = match version {
        RealtimeWsVersion::V1 => RealtimeEventParser::V1,
        RealtimeWsVersion::V2 => RealtimeEventParser::RealtimeV2,
        RealtimeWsVersion::V3 => RealtimeEventParser::FramelessBidi,
    };
    if version != RealtimeWsVersion::V2
        && matches!(params.output_modality, RealtimeOutputModality::Text)
    {
        return Err(CodexErr::InvalidRequest(
            "text realtime output modality requires realtime v2".to_string(),
        ));
    }
    let session_mode = match config.realtime.session_type {
        RealtimeWsMode::Conversational => RealtimeSessionMode::Conversational,
        RealtimeWsMode::Transcription => RealtimeSessionMode::Transcription,
    };
    let config_voice = match configured_voice {
        ConfiguredRealtimeVoice::Use => config.realtime.voice,
        ConfiguredRealtimeVoice::Ignore => None,
    };
    let voice = params
        .voice
        .or(config_voice)
        .unwrap_or_else(|| default_realtime_voice(version));
    validate_realtime_voice(version, voice)?;
    Ok(RealtimeSessionConfig {
        instructions: prompt,
        initial_items: params.initial_items.clone(),
        model,
        session_id: Some(
            params
                .realtime_session_id
                .clone()
                .unwrap_or_else(|| sess.thread_id.to_string()),
        ),
        event_parser,
        session_mode,
        output_modality: params.output_modality,
        voice,
    })
}

fn default_realtime_voice(version: RealtimeWsVersion) -> RealtimeVoice {
    let voices = RealtimeVoicesList::builtin();
    match version {
        RealtimeWsVersion::V1 | RealtimeWsVersion::V3 => voices.default_v1,
        RealtimeWsVersion::V2 => voices.default_v2,
    }
}

fn prefix_realtime_text(text: String, prefix: &str, session_kind: RealtimeSessionKind) -> String {
    if session_kind != RealtimeSessionKind::V2 || text.is_empty() || text.starts_with(prefix) {
        return text;
    }
    format!("{prefix}{text}")
}

fn realtime_backend_output(output_text: String, session_kind: RealtimeSessionKind) -> String {
    let output_text = prefix_realtime_text(output_text, REALTIME_BACKEND_TEXT_PREFIX, session_kind);
    truncate_realtime_text_to_token_budget(&output_text, REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET)
}

fn realtime_backend_item(text: String, prefix: Option<&str>) -> String {
    let text = match prefix.filter(|prefix| !prefix.is_empty()) {
        Some(prefix) => format!("{prefix}\n\n{text}"),
        None => text,
    };
    truncate_realtime_text_to_token_budget(&text, REALTIME_ASSISTANT_OUTPUT_TOKEN_BUDGET)
}

fn validate_realtime_voice(version: RealtimeWsVersion, voice: RealtimeVoice) -> CodexResult<()> {
    let voices = RealtimeVoicesList::builtin();
    let allowed = match version {
        RealtimeWsVersion::V1 | RealtimeWsVersion::V3 => &voices.v1,
        RealtimeWsVersion::V2 => &voices.v2,
    };
    if allowed.contains(&voice) {
        return Ok(());
    }

    let version = match version {
        RealtimeWsVersion::V1 => "v1",
        RealtimeWsVersion::V2 => "v2",
        RealtimeWsVersion::V3 => "v3",
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
        client_managed_handoffs,
        flush_transcript_tail_on_session_end,
        codex_responses_as_items,
        codex_response_item_prefix,
        codex_response_handoff_mode,
        codex_response_handoff_channel_prefixes,
        realtime_call_api_provider,
        requested_realtime_session_id,
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
        client_managed_handoffs,
        flush_transcript_tail_on_session_end,
        codex_responses_as_items,
        codex_response_item_prefix,
        codex_response_handoff_mode,
        codex_response_handoff_channel_prefixes,
        realtime_call_api_provider,
        session_config,
        model_client: sess.services.model_client.clone(),
        sdp,
    };
    let start_output = sess.conversation.start(start).await?;

    info!("realtime conversation started");

    sess.send_event_raw(Event {
        id: sub_id.to_string(),
        msg: EventMsg::RealtimeConversationStarted(RealtimeConversationStartedEvent {
            realtime_session_id: requested_realtime_session_id,
            version,
        }),
    })
    .await;

    let RealtimeStartOutput {
        realtime_active,
        events_rx,
        transcript_tail_rx,
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
        // Drain already-parsed events so a queued handoff is routed before the final tail.
        while let Ok(event) = events_rx.recv().await {
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
                    realtime_delegation_from_handoff(handoff)
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
        if let Ok(text) = transcript_tail_rx.recv().await {
            sess_clone.route_realtime_text_input(text).await;
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

fn realtime_transcript_delta_from_handoff(handoff: &RealtimeHandoffRequested) -> Option<String> {
    realtime_transcript_delta(&handoff.active_transcript)
}

fn realtime_transcript_delta(active_transcript: &[RealtimeTranscriptEntry]) -> Option<String> {
    let active_transcript = active_transcript
        .iter()
        .map(|entry| format!("{role}: {text}", role = entry.role, text = entry.text))
        .collect::<Vec<_>>()
        .join("\n");
    (!active_transcript.is_empty()).then_some(active_transcript)
}

fn realtime_text_from_handoff_request(handoff: &RealtimeHandoffRequested) -> Option<String> {
    (!handoff.input_transcript.is_empty())
        .then_some(handoff.input_transcript.clone())
        .or_else(|| realtime_transcript_delta_from_handoff(handoff))
}

fn realtime_delegation_from_handoff(handoff: &RealtimeHandoffRequested) -> Option<String> {
    let input = realtime_text_from_handoff_request(handoff)?;
    Some(wrap_realtime_delegation_input(
        &input,
        realtime_transcript_delta_from_handoff(handoff).as_deref(),
        RealtimeDelegationSource::Handoff,
    ))
}

fn wrap_realtime_delegation_input(
    input: &str,
    transcript_delta: Option<&str>,
    source: RealtimeDelegationSource,
) -> String {
    RealtimeDelegation::new(input, transcript_delta, source).render()
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
    realtime_session_id: Option<&str>,
    api_key: Option<&str>,
    event_parser: RealtimeEventParser,
    originator: &str,
) -> CodexResult<Option<HeaderMap>> {
    let mut headers = HeaderMap::new();

    match event_parser {
        RealtimeEventParser::V1 => {
            headers.insert("openai-alpha", HeaderValue::from_static("quicksilver=v1"));
        }
        RealtimeEventParser::FramelessBidi => {
            headers.insert("openai-alpha", HeaderValue::from_static("quicksilver=v2"));
        }
        RealtimeEventParser::RealtimeV2 => {}
    }

    if let Some(realtime_session_id) = realtime_session_id
        && let Ok(realtime_session_id) = HeaderValue::from_str(realtime_session_id)
    {
        headers.insert("x-session-id", realtime_session_id);
    }

    if let Some(api_key) = api_key {
        let auth_value = HeaderValue::from_str(&format!("Bearer {api_key}")).map_err(|err| {
            CodexErr::InvalidRequest(format!("invalid realtime api key header: {err}"))
        })?;
        headers.insert(AUTHORIZATION, auth_value);
    }

    add_originator_header(&mut headers, originator);

    Ok(Some(headers))
}

pub(crate) async fn handle_text(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationTextParams,
) {
    debug!(text = %params.text, "[realtime-text] appending realtime conversation text input");
    if let Err(err) = sess.conversation.text_in(params).await {
        error!("failed to append realtime text: {err}");
        if sess.conversation.running_state().await.is_some() {
            warn!("realtime text input failed while the session was already ending");
        } else {
            send_conversation_error(sess, sub_id, err.to_string(), CodexErrorInfo::BadRequest)
                .await;
        }
    }
}

pub(crate) async fn handle_speech(
    sess: &Arc<Session>,
    sub_id: String,
    params: ConversationSpeechParams,
) {
    debug!(text = %params.text, "[realtime-text] appending realtime speech");
    if let Err(err) = sess.conversation.append_speech(params.text).await {
        error!("failed to append realtime speech: {err}");
        if sess.conversation.running_state().await.is_some() {
            warn!("realtime speech append failed while the session was already ending");
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
    tokio::spawn(run_realtime_input_task(input))
}

struct RealtimeWebrtcSidebandInputTask {
    client: RealtimeWebsocketClient,
    session_config: RealtimeSessionConfig,
    call_id: String,
    sideband_headers: HeaderMap,
    input_channels: RealtimeInputChannels,
    events_tx: Sender<RealtimeEvent>,
    handoff_state: RealtimeHandoffState,
    session_kind: RealtimeSessionKind,
    event_parser: RealtimeEventParser,
    realtime_active: Arc<AtomicBool>,
    flush_transcript_tail_on_session_end: bool,
    transcript_tail_tx: Sender<String>,
    stop_token: CancellationToken,
}

fn spawn_webrtc_sideband_input_task(input: RealtimeWebrtcSidebandInputTask) -> JoinHandle<()> {
    let RealtimeWebrtcSidebandInputTask {
        client,
        session_config,
        call_id,
        sideband_headers,
        input_channels,
        events_tx,
        handoff_state,
        session_kind,
        event_parser,
        realtime_active,
        flush_transcript_tail_on_session_end,
        transcript_tail_tx,
        stop_token,
    } = input;

    tokio::spawn(async move {
        if !realtime_active.load(Ordering::Relaxed) {
            return;
        }

        let connection = match tokio::select! {
            connection = client.connect_webrtc_sideband(
                session_config,
                &call_id,
                sideband_headers,
                default_headers(),
            ) => connection,
            _ = stop_token.cancelled() => return,
        } {
            Ok(connection) => connection,
            Err(err) => {
                if realtime_active.load(Ordering::Relaxed) {
                    let mapped_error = map_api_error(err);
                    warn!("failed to connect realtime sideband: {mapped_error}");
                    let _ = events_tx
                        .send(RealtimeEvent::Error(mapped_error.to_string()))
                        .await;
                }
                return;
            }
        };

        if !realtime_active.load(Ordering::Relaxed) {
            return;
        }

        run_realtime_input_task(RealtimeInputTask {
            writer: connection.writer(),
            events: connection.events(),
            text_rx: input_channels.text_rx,
            handoff_output_rx: input_channels.handoff_output_rx,
            audio_rx: input_channels.audio_rx,
            events_tx,
            handoff_state,
            session_kind,
            event_parser,
            flush_transcript_tail_on_session_end,
            transcript_tail_tx,
            stop_token,
        })
        .await;
    })
}

async fn run_realtime_input_task(input: RealtimeInputTask) {
    let RealtimeInputTask {
        writer,
        events,
        text_rx,
        handoff_output_rx,
        audio_rx,
        events_tx,
        handoff_state,
        session_kind,
        event_parser,
        flush_transcript_tail_on_session_end,
        transcript_tail_tx,
        stop_token,
    } = input;

    let mut output_audio_state: Option<OutputAudioState> = None;
    let mut response_create_queue = RealtimeResponseCreateQueue::default();

    loop {
        let result = tokio::select! {
            _ = stop_token.cancelled() => break,
            // Text input that should be sent into realtime.
            text = text_rx.recv() => {
                handle_text_input(
                    text,
                    &writer,
                    &events_tx,
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

    if flush_transcript_tail_on_session_end
        && let Some(transcript_delta) =
            realtime_transcript_delta(&events.take_transcript_tail().await)
    {
        let _ = transcript_tail_tx
            .send(wrap_realtime_delegation_input(
                REALTIME_SESSION_ENDED_HANDOFF_INSTRUCTION,
                Some(&transcript_delta),
                RealtimeDelegationSource::TranscriptTailFlush,
            ))
            .await;
    }
}

async fn handle_text_input(
    params: Result<ConversationTextParams, RecvError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
) -> anyhow::Result<()> {
    let params = params.context("text input channel closed")?;

    if let Err(err) = writer
        .send_conversation_item_create(params.text, params.role)
        .await
    {
        let mapped_error = map_api_error(err);
        warn!("failed to send input text: {mapped_error}");
        let _ = events_tx
            .send(RealtimeEvent::Error(mapped_error.to_string()))
            .await;
        return Err(mapped_error.into());
    }
    Ok(())
}

async fn flush_streamed_handoff_item(handoff: &RealtimeHandoffState, item_id: &str) {
    let (handoff_id, text, phase) = {
        let mut stream = handoff.stream.lock().await;
        let Some(streamed_item) = stream.items.get_mut(item_id) else {
            return;
        };
        streamed_item.flush_scheduled = false;
        let Some(text) = streamed_item.drain_stream_chunk() else {
            return;
        };
        streamed_item.last_flush_at = Instant::now();
        (
            streamed_item.handoff_id.clone(),
            text,
            streamed_item.phase.clone(),
        )
    };
    let _ = handoff
        .output_tx
        .send(RealtimeOutbound::HandoffAppend {
            handoff_id,
            text,
            phase,
        })
        .await;
}

fn schedule_streamed_handoff_flush(
    handoff: &RealtimeHandoffState,
    item_id: String,
    flush_delay: Duration,
) {
    let handoff = handoff.clone();
    let _flush_task = tokio::spawn(async move {
        tokio::time::sleep(flush_delay).await;
        flush_streamed_handoff_item(&handoff, &item_id).await;
    });
}

fn v3_output_writer(
    writer: &RealtimeWebsocketWriter,
    phase: Option<&MessagePhase>,
    handoff_mode: CodexResponseHandoffMode,
) -> RealtimeWebsocketWriter {
    let channel = match handoff_mode {
        CodexResponseHandoffMode::Thinking => None,
        CodexResponseHandoffMode::Commentary => Some(RealtimeContextAppendChannel::Commentary),
        CodexResponseHandoffMode::BemTags => match phase {
            Some(MessagePhase::FinalAnswer) => Some(RealtimeContextAppendChannel::Speakable),
            Some(MessagePhase::Commentary) => Some(RealtimeContextAppendChannel::Commentary),
            None => Some(RealtimeContextAppendChannel::Speakable),
        },
    };
    match channel {
        Some(channel) => writer.clone().with_context_append_channel(channel),
        None => writer.clone(),
    }
}

async fn handle_handoff_output(
    handoff_output: Result<RealtimeOutbound, RecvError>,
    writer: &RealtimeWebsocketWriter,
    events_tx: &Sender<RealtimeEvent>,
    handoff_state: &RealtimeHandoffState,
    event_parser: RealtimeEventParser,
    response_create_queue: &mut RealtimeResponseCreateQueue,
) -> anyhow::Result<()> {
    let handoff_output = handoff_output.context("handoff output channel closed")?;
    let result = match event_parser {
        RealtimeEventParser::V1 => match handoff_output {
            RealtimeOutbound::StandaloneHandoff { text, phase: _ } => {
                writer
                    .send_standalone_handoff(STANDALONE_HANDOFF_ID.to_string(), text)
                    .await
            }
            RealtimeOutbound::StandaloneSpeech { text } => {
                writer
                    .send_standalone_handoff(STANDALONE_HANDOFF_ID.to_string(), text)
                    .await
            }
            RealtimeOutbound::HandoffUpdate {
                handoff_id,
                text,
                phase: _,
            }
            | RealtimeOutbound::CompletedHandoff {
                handoff_id,
                text,
                phase: _,
            } => {
                writer
                    .send_conversation_function_call_output(handoff_id, text)
                    .await
            }
            RealtimeOutbound::HandoffAppend {
                handoff_id,
                text,
                phase: _,
            } => {
                writer
                    .send_conversation_handoff_append(handoff_id, text)
                    .await
            }
            RealtimeOutbound::ConversationItem { text, phase: _ } => {
                writer
                    .send_conversation_item_create(text, ConversationTextRole::Developer)
                    .await
            }
            RealtimeOutbound::HandoffCompleteAck { .. } => Ok(()),
        },
        RealtimeEventParser::FramelessBidi => match handoff_output {
            RealtimeOutbound::StandaloneHandoff { text, phase } => {
                v3_output_writer(
                    writer,
                    phase.as_ref(),
                    handoff_state.codex_response_handoff_mode,
                )
                .send_standalone_handoff(STANDALONE_HANDOFF_ID.to_string(), text)
                .await
            }
            RealtimeOutbound::StandaloneSpeech { text } => {
                writer
                    .clone()
                    .with_context_append_channel(RealtimeContextAppendChannel::Speakable)
                    .send_standalone_handoff(STANDALONE_HANDOFF_ID.to_string(), text)
                    .await
            }
            RealtimeOutbound::HandoffUpdate {
                handoff_id,
                text,
                phase,
            } => {
                v3_output_writer(
                    writer,
                    phase.as_ref(),
                    handoff_state.codex_response_handoff_mode,
                )
                .send_conversation_function_call_output(handoff_id, text)
                .await
            }
            RealtimeOutbound::HandoffAppend {
                handoff_id,
                text,
                phase,
            } => {
                v3_output_writer(
                    writer,
                    phase.as_ref(),
                    handoff_state.codex_response_handoff_mode,
                )
                .send_conversation_handoff_append(handoff_id, text)
                .await
            }
            RealtimeOutbound::CompletedHandoff {
                handoff_id,
                text,
                phase,
            } => {
                v3_output_writer(
                    writer,
                    phase.as_ref(),
                    handoff_state.codex_response_handoff_mode,
                )
                .send_conversation_function_call_output(handoff_id, text)
                .await
            }
            RealtimeOutbound::ConversationItem { text, phase } => {
                v3_output_writer(
                    writer,
                    phase.as_ref(),
                    handoff_state.codex_response_handoff_mode,
                )
                .send_conversation_item_create(text, ConversationTextRole::Developer)
                .await
            }
            RealtimeOutbound::HandoffCompleteAck { .. } => Ok(()),
        },
        RealtimeEventParser::RealtimeV2 => match handoff_output {
            RealtimeOutbound::StandaloneHandoff { text, phase: _ } => {
                if let Err(err) = writer
                    .send_conversation_item_create(text, ConversationTextRole::User)
                    .await
                {
                    Err(err)
                } else {
                    return response_create_queue
                        .request_create(writer, events_tx, "standalone handoff")
                        .await;
                }
            }
            RealtimeOutbound::StandaloneSpeech { text } => {
                if let Err(err) = writer
                    .send_conversation_item_create(text, ConversationTextRole::User)
                    .await
                {
                    Err(err)
                } else {
                    return response_create_queue
                        .request_create(writer, events_tx, "standalone handoff")
                        .await;
                }
            }
            RealtimeOutbound::HandoffUpdate {
                handoff_id,
                text,
                phase: _,
            }
            | RealtimeOutbound::HandoffAppend {
                handoff_id,
                text,
                phase: _,
            } => {
                let active_handoff = handoff_state.stream.lock().await.active_handoff.clone();
                match active_handoff {
                    Some(active_handoff) if active_handoff == handoff_id => {}
                    Some(_) | None => {
                        debug!("dropping stale realtime handoff progress update");
                        return Ok(());
                    }
                }
                writer
                    .send_conversation_item_create(text, ConversationTextRole::User)
                    .await
            }
            RealtimeOutbound::CompletedHandoff {
                handoff_id,
                text: _,
                phase: _,
            } => {
                if let Err(err) = writer
                    .send_conversation_function_call_output(
                        handoff_id,
                        REALTIME_V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT.to_string(),
                    )
                    .await
                {
                    Err(err)
                } else {
                    return response_create_queue
                        .request_create(writer, events_tx, "handoff")
                        .await;
                }
            }
            RealtimeOutbound::ConversationItem { text, phase: _ } => {
                writer
                    .send_conversation_item_create(text, ConversationTextRole::Developer)
                    .await
            }
            RealtimeOutbound::HandoffCompleteAck { handoff_id } => {
                writer
                    .send_conversation_function_call_output(handoff_id, String::new())
                    .await
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
                    let mut stream = handoff_state.stream.lock().await;
                    stream.items.clear();
                    stream.active_handoff = Some(handoff.handoff_id.clone());
                }
                RealtimeSessionKind::V2 => {
                    let active_handoff = handoff_state.stream.lock().await.active_handoff.clone();
                    match active_handoff {
                        Some(_) => {
                            if let Err(err) = writer
                                .send_conversation_function_call_output(
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
                            handoff_state.stream.lock().await.active_handoff =
                                Some(handoff.handoff_id.clone());
                        }
                    }
                }
            }
            false
        }
        RealtimeEvent::NoopRequested(noop) => {
            *output_audio_state = None;

            match session_kind {
                RealtimeSessionKind::V1 => {}
                RealtimeSessionKind::V2 => {
                    if let Err(err) = writer
                        .send_conversation_function_call_output(noop.call_id.clone(), String::new())
                        .await
                    {
                        let mapped_error = map_api_error(err);
                        warn!("failed to send realtime noop function output: {mapped_error}");
                        let _ = events_tx
                            .send(RealtimeEvent::Error(mapped_error.to_string()))
                            .await;
                        return Err(mapped_error.into());
                    }
                }
            }
            false
        }
        RealtimeEvent::Error(_) => true,
        RealtimeEvent::SessionUpdated {
            realtime_session_id,
            ..
        } => {
            info!(realtime_session_id = %realtime_session_id, "realtime session updated");
            false
        }
        RealtimeEvent::InputTranscriptDelta(_)
        | RealtimeEvent::InputTranscriptDone(_)
        | RealtimeEvent::OutputTranscriptDelta(_)
        | RealtimeEvent::OutputTranscriptDone(_)
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
