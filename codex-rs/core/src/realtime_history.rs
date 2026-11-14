//! Records canonical Voice history for every Core host. Presentation rules share
//! transcript boundaries and deduplication state with the history reducer.
//! The reducer selects when its effects are persisted relative to their source event.

mod presentation;

use crate::observation_clock::ObservationClock;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RealtimeEvent;
use codex_protocol::protocol::RealtimeTranscriptDelta;
use codex_protocol::protocol::RealtimeTranscriptDone;
use codex_protocol::realtime::BemItemPresentation;
use codex_protocol::realtime::RealtimeFrameChannels;
use codex_protocol::realtime::RealtimeItem;
use codex_protocol::realtime::RealtimeItemContent;
use codex_protocol::realtime::RealtimeSessionOutcome;
use codex_protocol::realtime::RealtimeTranscriptRole;
use codex_protocol::realtime::RealtimeVoiceTranscript;
use codex_protocol::user_input::UserInput;
use std::collections::HashMap;
use std::collections::HashSet;
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ActiveSegment {
    session_id: String,
    id: String,
    role: RealtimeTranscriptRole,
    text: String,
    observed_at_ms: Option<i64>,
}

#[derive(Debug, Default)]
struct ActiveTranscriptSegments {
    user: Option<ActiveSegment>,
    assistant: Option<ActiveSegment>,
    first_active_role: Option<RealtimeTranscriptRole>,
}

impl ActiveTranscriptSegments {
    fn slot_mut(&mut self, role: RealtimeTranscriptRole) -> &mut Option<ActiveSegment> {
        match role {
            RealtimeTranscriptRole::User => &mut self.user,
            RealtimeTranscriptRole::Assistant => &mut self.assistant,
        }
    }

    fn take(&mut self, role: RealtimeTranscriptRole) -> Option<ActiveSegment> {
        let segment = self.slot_mut(role).take();
        if self.first_active_role == Some(role) {
            self.first_active_role = self
                .user
                .as_ref()
                .or(self.assistant.as_ref())
                .map(|segment| segment.role);
        }
        segment
    }
}

#[derive(Debug)]
struct StreamingAgentMessage {
    item_id: String,
    text: String,
}

#[derive(Debug)]
pub(crate) struct RealtimeTranscriptStream {
    pub(crate) started_item: Option<RealtimeItem>,
    pub(crate) item_id: String,
    pub(crate) delta: String,
}

#[derive(Debug, Default)]
pub(crate) struct RealtimeEventEffects {
    pub(crate) seal_frames: bool,
    pub(crate) order: RealtimeEventOrder,
    pub(crate) items: Vec<RealtimeItem>,
    pub(crate) transcript_stream: Option<RealtimeTranscriptStream>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RealtimeEventOrder {
    BeforeEvent,
    #[default]
    AfterEvent,
}

#[derive(Clone, Copy)]
enum Continuation {
    Continue,
    Finish,
}

/// Retains only live session state; durable history is served by the rollout index.
#[derive(Default)]
pub(crate) struct RealtimeHistoryState {
    pub(crate) observation_clock: ObservationClock,
    /// Resolved once at the first call. Unresolved and unenrolled threads write legacy history.
    pub(crate) frame_enrollment: Option<bool>,
    active_session_id: Option<String>,
    active_segments: ActiveTranscriptSegments,
    streaming_agent_message: Option<StreamingAgentMessage>,
    active_turn_id: Option<String>,
    realtime_session_by_bem_turn: HashMap<String, String>,
    promoted_bem_presentation_keys: HashSet<String>,
    pending_handoffs: VecDeque<String>,
    failed: bool,
}

impl RealtimeHistoryState {
    pub(crate) fn frames_enabled(&self) -> bool {
        self.frame_enrollment == Some(true)
    }

    fn should_seal_user_input(&self, input: &[UserInput]) -> bool {
        self.active_session_id.is_some()
            && [&self.active_segments.user, &self.active_segments.assistant]
                .into_iter()
                .flatten()
                .any(|segment| !segment.text.is_empty())
            && !matches!(input, [UserInput::Text { text, .. }] if {
                let text = text.trim();
                text.starts_with("<realtime_delegation>")
                    && text.ends_with("</realtime_delegation>")
            })
    }

    fn seal_user_input(&mut self, input: &[UserInput]) -> Vec<RealtimeItem> {
        if !self.should_seal_user_input(input) {
            return Vec::new();
        }
        let mut items = Vec::new();
        self.seal_segments(&mut items, Continuation::Continue);
        items
    }

    pub(crate) fn should_observe(&self, event: &EventMsg) -> bool {
        matches!(
            event,
            EventMsg::RealtimeConversationStarted(_)
                | EventMsg::TurnStarted(_)
                | EventMsg::TurnComplete(_)
                | EventMsg::TurnAborted(_)
        ) || (self.active_session_id.is_some()
            && matches!(
                event,
                EventMsg::RealtimeConversationRealtime(_)
                    | EventMsg::RealtimeConversationClosed(_)
                    | EventMsg::TurnStarted(_)
                    | EventMsg::ItemStarted(_)
                    | EventMsg::ItemCompleted(_)
                    | EventMsg::AgentMessageContentDelta(_)
            ))
            || match event {
                EventMsg::ItemStarted(event) => self
                    .realtime_session_by_bem_turn
                    .contains_key(&event.turn_id),
                EventMsg::ItemCompleted(event) => self
                    .realtime_session_by_bem_turn
                    .contains_key(&event.turn_id),
                EventMsg::AgentMessageContentDelta(event) => self
                    .realtime_session_by_bem_turn
                    .contains_key(&event.turn_id),
                _ => false,
            }
    }

    pub(crate) fn observe(&mut self, event: &EventMsg) -> RealtimeEventEffects {
        match event {
            EventMsg::TurnStarted(event) => self.active_turn_id = Some(event.turn_id.clone()),
            EventMsg::TurnComplete(event)
                if self.active_turn_id.as_deref() == Some(event.turn_id.as_str()) =>
            {
                self.active_turn_id = None;
            }
            EventMsg::TurnAborted(event)
                if event.turn_id.is_none() || event.turn_id == self.active_turn_id =>
            {
                self.active_turn_id = None;
            }
            _ => {}
        }
        if !self.should_observe(event) {
            return RealtimeEventEffects::default();
        }
        let mut order = RealtimeEventOrder::AfterEvent;
        let mut items = Vec::new();
        let mut transcript_stream = None;
        match event {
            EventMsg::RealtimeConversationStarted(event) => {
                let session_id = event
                    .realtime_session_id
                    .clone()
                    .unwrap_or_else(|| Uuid::now_v7().to_string());
                if self.active_session_id.as_deref() != Some(session_id.as_str()) {
                    self.seal_segments(&mut items, Continuation::Finish);
                    self.active_session_id = Some(session_id.clone());
                    self.failed = false;
                    items.push(RealtimeItem {
                        // The call ID lets a reader seek directly to its opening position.
                        id: if self.frames_enabled() {
                            format!("realtime-session-start:{session_id}")
                        } else {
                            Uuid::now_v7().to_string()
                        },
                        realtime_session_id: session_id.clone(),
                        observed_at_ms: self
                            .frames_enabled()
                            .then(|| self.observation_clock.now_ms()),
                        content: RealtimeItemContent::RealtimeSessionStarted,
                    });
                }
                if let Some(turn_id) = &self.active_turn_id {
                    self.realtime_session_by_bem_turn
                        .insert(turn_id.to_string(), session_id);
                }
            }
            EventMsg::RealtimeConversationRealtime(event) => match &event.payload {
                RealtimeEvent::InputTranscriptDelta(delta) => {
                    transcript_stream = self.add_delta(RealtimeTranscriptRole::User, delta);
                }
                RealtimeEvent::OutputTranscriptDelta(delta) => {
                    transcript_stream = self.add_delta(RealtimeTranscriptRole::Assistant, delta);
                }
                RealtimeEvent::InputTranscriptDone(done) => {
                    transcript_stream =
                        self.finish_segment(&mut items, RealtimeTranscriptRole::User, done);
                }
                RealtimeEvent::OutputTranscriptDone(done) => {
                    transcript_stream =
                        self.finish_segment(&mut items, RealtimeTranscriptRole::Assistant, done);
                }
                RealtimeEvent::HandoffRequested(_) => {
                    if let Some(session_id) = &self.active_session_id {
                        self.pending_handoffs.push_back(session_id.clone());
                    }
                }
                RealtimeEvent::Error(_) => self.failed = true,
                _ => {}
            },
            EventMsg::TurnStarted(event) => {
                if let Some(session_id) = self
                    .pending_handoffs
                    .pop_front()
                    .or_else(|| self.active_session_id.clone())
                {
                    self.realtime_session_by_bem_turn
                        .entry(event.turn_id.clone())
                        .or_insert(session_id);
                }
            }
            EventMsg::ItemStarted(event) => {
                if let TurnItem::UserMessage(item) = &event.item {
                    order = RealtimeEventOrder::BeforeEvent;
                    items.extend(self.seal_user_input(&item.content));
                }
                self.observe_item(
                    &mut items,
                    &event.turn_id,
                    &event.item,
                    /*completed*/ false,
                );
            }
            EventMsg::ItemCompleted(event) => {
                if let TurnItem::UserMessage(item) = &event.item {
                    order = RealtimeEventOrder::BeforeEvent;
                    items.extend(self.seal_user_input(&item.content));
                }
                self.observe_item(
                    &mut items,
                    &event.turn_id,
                    &event.item,
                    /*completed*/ true,
                );
                if self
                    .streaming_agent_message
                    .as_ref()
                    .is_some_and(|message| message.item_id == event.item.id())
                {
                    self.streaming_agent_message = None;
                }
            }
            EventMsg::AgentMessageContentDelta(event) => {
                let message =
                    self.streaming_agent_message
                        .get_or_insert_with(|| StreamingAgentMessage {
                            item_id: event.item_id.clone(),
                            text: String::new(),
                        });
                if message.item_id != event.item_id {
                    message.item_id.clone_from(&event.item_id);
                    message.text.clear();
                }
                message.text.push_str(&event.delta);
                let text = message.text.clone();
                self.observe_assistant_message(&mut items, &event.turn_id, &event.item_id, &text);
            }
            EventMsg::RealtimeConversationClosed(_) => {
                if let Some(session_id) = self.active_session_id.take() {
                    self.seal_segments(&mut items, Continuation::Finish);
                    items.push(RealtimeItem {
                        id: Uuid::now_v7().to_string(),
                        realtime_session_id: session_id,
                        observed_at_ms: self
                            .frames_enabled()
                            .then(|| self.observation_clock.now_ms()),
                        content: RealtimeItemContent::RealtimeSessionClosed {
                            outcome: if self.failed {
                                RealtimeSessionOutcome::Failed
                            } else {
                                RealtimeSessionOutcome::Ended
                            },
                        },
                    });
                    self.failed = false;
                }
            }
            _ => {}
        }
        RealtimeEventEffects {
            seal_frames: self.frames_enabled(),
            order,
            items,
            transcript_stream,
        }
    }

    fn add_promotion(
        &mut self,
        items: &mut Vec<RealtimeItem>,
        turn_id: &str,
        item_id: &str,
        presentation: BemItemPresentation,
    ) {
        let Some(realtime_session_id) = self.realtime_session_by_bem_turn.get(turn_id).cloned()
        else {
            return;
        };
        let presentation_key = match &presentation {
            BemItemPresentation::WholeItem => format!("{item_id}:whole-item"),
            BemItemPresentation::InlineMarkdown => format!("{item_id}:inline-markdown"),
            BemItemPresentation::InlineVisualization { index } => {
                format!("{item_id}:inline-visualization:{index}")
            }
        };
        if !self.promoted_bem_presentation_keys.insert(presentation_key) {
            return;
        }
        self.seal_segments(items, Continuation::Continue);
        items.push(RealtimeItem {
            id: Uuid::now_v7().to_string(),
            realtime_session_id,
            observed_at_ms: self
                .frames_enabled()
                .then(|| self.observation_clock.now_ms()),
            content: RealtimeItemContent::BemItemPromoted {
                turn_id: turn_id.to_string(),
                item_id: item_id.to_string(),
                presentation,
            },
        });
    }

    fn add_delta(
        &mut self,
        role: RealtimeTranscriptRole,
        delta: &RealtimeTranscriptDelta,
    ) -> Option<RealtimeTranscriptStream> {
        let session_id = self.active_session_id.clone()?;
        self.active_segments.first_active_role.get_or_insert(role);
        let frames_enabled = self.frames_enabled();
        let segment = self
            .active_segments
            .slot_mut(role)
            .get_or_insert_with(|| ActiveSegment {
                session_id,
                id: Uuid::now_v7().to_string(),
                role,
                text: String::new(),
                observed_at_ms: None,
            });
        let observed_at_ms = frames_enabled.then(|| {
            *segment
                .observed_at_ms
                .get_or_insert_with(|| self.observation_clock.now_ms())
        });
        let started_item = segment.text.is_empty().then(|| RealtimeItem {
            id: segment.id.clone(),
            realtime_session_id: segment.session_id.clone(),
            observed_at_ms,
            content: speech(frames_enabled, segment.role, String::new()),
        });
        segment.text.push_str(&delta.delta);
        Some(RealtimeTranscriptStream {
            started_item,
            item_id: segment.id.clone(),
            delta: delta.delta.clone(),
        })
    }

    fn finish_segment(
        &mut self,
        items: &mut Vec<RealtimeItem>,
        role: RealtimeTranscriptRole,
        done: &RealtimeTranscriptDone,
    ) -> Option<RealtimeTranscriptStream> {
        let Some(segment) = self.active_segments.take(role) else {
            if done.text.is_empty() {
                return None;
            }
            let stream = self.add_delta(
                role,
                &RealtimeTranscriptDelta {
                    delta: done.text.clone(),
                },
            );
            if let Some(segment) = self.active_segments.take(role) {
                self.seal_active_segment(items, segment, Continuation::Finish);
            }
            return stream;
        };
        // A split leaves an empty continuation; the upstream final may repeat
        // text that was already committed before the split.
        self.seal_active_segment(items, segment, Continuation::Finish);
        None
    }

    fn seal_segments(&mut self, items: &mut Vec<RealtimeItem>, continuation: Continuation) {
        let mut segments = std::mem::take(&mut self.active_segments);
        let roles = match segments.first_active_role {
            Some(RealtimeTranscriptRole::Assistant) => [
                RealtimeTranscriptRole::Assistant,
                RealtimeTranscriptRole::User,
            ],
            Some(RealtimeTranscriptRole::User) | None => [
                RealtimeTranscriptRole::User,
                RealtimeTranscriptRole::Assistant,
            ],
        };
        for role in roles {
            if let Some(segment) = segments.take(role) {
                self.seal_active_segment(items, segment, continuation);
            }
        }
    }

    fn seal_active_segment(
        &mut self,
        items: &mut Vec<RealtimeItem>,
        segment: ActiveSegment,
        continuation: Continuation,
    ) {
        if !segment.text.is_empty() {
            items.push(RealtimeItem {
                id: segment.id,
                realtime_session_id: segment.session_id.clone(),
                observed_at_ms: segment.observed_at_ms,
                content: speech(self.frames_enabled(), segment.role, segment.text),
            });
        }
        if matches!(continuation, Continuation::Continue) {
            self.active_segments
                .first_active_role
                .get_or_insert(segment.role);
            *self.active_segments.slot_mut(segment.role) = Some(ActiveSegment {
                session_id: segment.session_id,
                id: Uuid::now_v7().to_string(),
                role: segment.role,
                text: String::new(),
                observed_at_ms: None,
            });
        }
    }
}

fn speech(frames_enabled: bool, role: RealtimeTranscriptRole, text: String) -> RealtimeItemContent {
    if frames_enabled {
        RealtimeItemContent::Frame {
            channels: RealtimeFrameChannels {
                voice_transcript: RealtimeVoiceTranscript { role, text },
            },
        }
    } else {
        RealtimeItemContent::TranscriptSegment { role, text }
    }
}

#[cfg(test)]
#[path = "realtime_history_tests.rs"]
mod tests;
