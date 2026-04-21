use crate::handoff::REALTIME_BACKEND_TEXT_PREFIX;
use crate::handoff::REALTIME_USER_TEXT_PREFIX;
use crate::handoff::prefix_realtime_v2_text;
use anyhow::Context;
use async_channel::Receiver;
use async_channel::RecvError;
use async_channel::Sender;
use async_channel::TrySendError;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use codex_api::ApiError;
use codex_api::RealtimeAudioFrame;
use codex_api::RealtimeEvent;
use codex_api::RealtimeEventParser;
use codex_api::RealtimeWebsocketEvents;
use codex_api::RealtimeWebsocketWriter;
use codex_api::map_api_error;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use serde_json::json;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::atomic::Ordering;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tracing::debug;
use tracing::error;
use tracing::warn;

const AUDIO_IN_QUEUE_CAPACITY: usize = 256;
const USER_TEXT_IN_QUEUE_CAPACITY: usize = 64;
const HANDOFF_OUT_QUEUE_CAPACITY: usize = 64;
const OUTPUT_EVENTS_QUEUE_CAPACITY: usize = 256;
const REALTIME_V2_HANDOFF_COMPLETE_ACKNOWLEDGEMENT: &str =
    "Background agent finished. Use the preceding [BACKEND] messages as the result.";
const REALTIME_V2_STEER_ACKNOWLEDGEMENT: &str =
    "This was sent to steer the previous background agent task.";
const REALTIME_ACTIVE_RESPONSE_ERROR_PREFIX: &str =
    "Conversation already has an active response in progress:";

enum RealtimeFanoutTaskStop {
    Abort,
    Detach,
}

pub struct RealtimeConversationManager {
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

struct ConversationState {
    audio_tx: Sender<RealtimeAudioFrame>,
    user_text_tx: Sender<String>,
    session_kind: RealtimeSessionKind,
    handoff: RealtimeHandoffState,
    input_task: JoinHandle<()>,
    fanout_task: Option<JoinHandle<()>>,
    realtime_active: Arc<AtomicBool>,
}

pub struct RealtimeStart {
    pub writer: RealtimeWebsocketWriter,
    pub events: RealtimeWebsocketEvents,
    pub event_parser: RealtimeEventParser,
    pub sdp: Option<String>,
}

pub struct RealtimeStartOutput {
    pub realtime_active: Arc<AtomicBool>,
    pub events_rx: Receiver<RealtimeEvent>,
    pub sdp: Option<String>,
}

impl Default for RealtimeConversationManager {
    fn default() -> Self {
        Self::new()
    }
}

impl RealtimeConversationManager {
    pub fn new() -> Self {
        Self {
            state: Mutex::new(None),
        }
    }

    pub async fn running_state(&self) -> Option<()> {
        let state = self.state.lock().await;
        state
            .as_ref()
            .and_then(|state| state.realtime_active.load(Ordering::Relaxed).then_some(()))
    }

    pub async fn is_running_v2(&self) -> bool {
        let state = self.state.lock().await;
        matches!(
            state.as_ref(),
            Some(state)
                if state.realtime_active.load(Ordering::Relaxed)
                    && state.session_kind == RealtimeSessionKind::V2
        )
    }

    pub async fn start(&self, start: RealtimeStart) -> CodexResult<RealtimeStartOutput> {
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
            writer,
            events,
            event_parser,
            sdp,
        } = start;
        let session_kind = match event_parser {
            RealtimeEventParser::V1 => RealtimeSessionKind::V1,
            RealtimeEventParser::RealtimeV2 => RealtimeSessionKind::V2,
        };
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
            session_kind,
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

    pub async fn register_fanout_task(
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

    pub async fn finish_if_active(&self, realtime_active: &Arc<AtomicBool>) {
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

    pub async fn audio_in(&self, frame: RealtimeAudioFrame) -> CodexResult<()> {
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

    pub async fn text_in(&self, text: String) -> CodexResult<()> {
        let sender = {
            let guard = self.state.lock().await;
            guard
                .as_ref()
                .map(|state| (state.user_text_tx.clone(), state.session_kind))
        };

        let Some((sender, session_kind)) = sender else {
            return Err(CodexErr::InvalidRequest(
                "conversation is not running".to_string(),
            ));
        };

        let text = if session_kind == RealtimeSessionKind::V2 {
            prefix_realtime_v2_text(text, REALTIME_USER_TEXT_PREFIX)
        } else {
            text
        };
        sender
            .send(text)
            .await
            .map_err(|_| CodexErr::InvalidRequest("conversation is not running".to_string()))?;
        Ok(())
    }

    pub async fn handoff_out(&self, output_text: String) -> CodexResult<()> {
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

        let output_text = if handoff.session_kind == RealtimeSessionKind::V2 {
            prefix_realtime_v2_text(output_text, REALTIME_BACKEND_TEXT_PREFIX)
        } else {
            output_text
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

    pub async fn handoff_complete(&self) -> CodexResult<()> {
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

    pub async fn active_handoff_id(&self) -> Option<String> {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        }?;
        handoff.active_handoff.lock().await.clone()
    }

    pub async fn clear_active_handoff(&self) {
        let handoff = {
            let guard = self.state.lock().await;
            guard.as_ref().map(|state| state.handoff.clone())
        };
        if let Some(handoff) = handoff {
            *handoff.active_handoff.lock().await = None;
            *handoff.last_output_text.lock().await = None;
        }
    }

    pub async fn shutdown(&self) -> CodexResult<()> {
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
                user_text = user_text_rx.recv() => {
                    handle_user_text_input(user_text, &writer, &events_tx).await
                }
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
                user_audio_frame = audio_rx.recv() => {
                    handle_user_audio_input(user_audio_frame, &writer, &events_tx).await
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
                    .send_conversation_function_call_output(handoff_id, output_text)
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
                writer.send_conversation_item_create(output_text).await
            }
            HandoffOutput::FinalUpdate {
                handoff_id,
                output_text: _,
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
            if session_kind == RealtimeSessionKind::V2 {
                update_output_audio_state(output_audio_state, frame);
            }
            false
        }
        RealtimeEvent::InputAudioSpeechStarted(event) => {
            if session_kind == RealtimeSessionKind::V2
                && let Some(output_audio_state) = output_audio_state.take()
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
            false
        }
        RealtimeEvent::ResponseCreated(_) => {
            if session_kind == RealtimeSessionKind::V2 {
                response_create_queue.mark_started();
            }
            false
        }
        RealtimeEvent::ResponseCancelled(_) | RealtimeEvent::ResponseDone(_) => {
            *output_audio_state = None;
            if session_kind == RealtimeSessionKind::V2 {
                response_create_queue
                    .mark_finished(writer, events_tx, "deferred")
                    .await?;
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
                            *handoff_state.last_output_text.lock().await = None;
                            *handoff_state.active_handoff.lock().await =
                                Some(handoff.handoff_id.clone());
                        }
                    }
                }
            }
            false
        }
        RealtimeEvent::NoopRequested(noop) => {
            *output_audio_state = None;

            if session_kind == RealtimeSessionKind::V2
                && let Err(err) = writer
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
            false
        }
        RealtimeEvent::Error(_) => true,
        RealtimeEvent::SessionUpdated { .. }
        | RealtimeEvent::InputTranscriptDelta(_)
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

#[cfg(test)]
#[path = "realtime_conversation_tests.rs"]
mod tests;
