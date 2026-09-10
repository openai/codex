//! Thread event buffering and replay state for the TUI app.
//!
//! This module owns the per-thread event store used when the TUI switches between the main
//! conversation, subagents, and side conversations. It keeps buffered app-server notifications,
//! pending interactive request replay state, active-turn tracking, and saved composer state close
//! together with the replay behavior that consumes them.

use super::*;
use std::borrow::Cow;

#[derive(Debug, Clone)]
pub(super) struct ThreadEventSnapshot {
    pub(super) session: Option<ThreadSessionState>,
    pub(super) delegated_turns: Vec<String>,
    pub(super) turns: Vec<Turn>,
    pub(super) events: Vec<ThreadBufferedEvent>,
    pub(super) active_reasoning_item: Option<codex_app_server_protocol::ItemStartedNotification>,
    pub(super) input_state: Option<ThreadInputState>,
}

#[derive(Debug, Clone)]
pub(super) enum ThreadBufferedEvent {
    Notification(Box<ServerNotification>),
    Request(Box<ServerRequest>),
    HistoryEntryResponse(HistoryLookupResponse),
    FeedbackSubmission(FeedbackThreadEvent),
}

fn is_voice_handoff_item(item: &ThreadItem) -> bool {
    matches!(item, ThreadItem::UserMessage { content, .. }
        if crate::chatwidget::realtime_delegation_input(content).is_some())
}

fn hide_private_items_after_voice_handoff(items: &mut Vec<ThreadItem>, delegated: bool) {
    let has_marker = items.iter().any(is_voice_handoff_item);
    // An evicted marker leaves no reliable order, so hide private items
    // conservatively. A present marker lets earlier typed output survive.
    let mut voice_started = delegated && !has_marker;
    items.retain(|item| {
        if is_voice_handoff_item(item) {
            voice_started = true;
        }
        !voice_started || !crate::chatwidget::is_private_realtime_agent_item(item)
    });
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FeedbackThreadEvent {
    pub(super) category: FeedbackCategory,
    pub(super) include_logs: bool,
    pub(super) feedback_audience: FeedbackAudience,
    pub(super) result: Result<String, String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ThreadEventAttachment {
    Live,
    ReplayOnly,
    ExternalWriter,
}

#[derive(Debug)]
pub(super) struct ThreadEventStore {
    pub(super) session: Option<ThreadSessionState>,
    pub(super) turns: Vec<Turn>,
    pub(super) buffer: VecDeque<ThreadBufferedEvent>,
    pub(super) pending_interactive_replay: PendingInteractiveReplayState,
    pub(super) active_turn_id: Option<String>,
    // Retain the active item even if its start falls out of the bounded replay buffer.
    pub(super) active_reasoning_item: Option<codex_app_server_protocol::ItemStartedNotification>,
    // Lifecycle identity must survive bounded replay-buffer eviction.
    pub(super) latest_turn_id: Option<String>,
    pub(super) pending_interrupt_turn_id: Option<String>,
    pub(super) input_state: Option<ThreadInputState>,
    pub(super) capacity: usize,
    pub(super) active: bool,
    pub(super) buffered_agent_message_delta_bytes: usize,
    delegated_turns: VecDeque<String>,
    recap_progress: recap::RecapProgress,
}

impl ThreadEventStore {
    pub(super) fn event_survives_session_refresh(event: &mut ThreadBufferedEvent) -> bool {
        if let ThreadBufferedEvent::Notification(notification) = event
            && let ServerNotification::ItemCompleted(notification) = notification.as_mut()
            && let ThreadItem::AgentMessage {
                questions: Some(_),
                text,
                ..
            } = &mut notification.item
        {
            // Refreshed turns contain the text; retain only the live question state.
            text.clear();
            return true;
        }
        match event {
            ThreadBufferedEvent::Request(_) | ThreadBufferedEvent::FeedbackSubmission(_) => true,
            ThreadBufferedEvent::Notification(notification) => matches!(
                notification.as_ref(),
                ServerNotification::HookStarted(_)
                    | ServerNotification::HookCompleted(_)
                    | ServerNotification::McpServerStatusUpdated(_)
            ),
            ThreadBufferedEvent::HistoryEntryResponse(_) => false,
        }
    }

    pub(super) fn new(capacity: usize) -> Self {
        Self {
            session: None,
            turns: Vec::new(),
            buffer: VecDeque::new(),
            pending_interactive_replay: PendingInteractiveReplayState::default(),
            active_turn_id: None,
            active_reasoning_item: None,
            latest_turn_id: None,
            pending_interrupt_turn_id: None,
            input_state: None,
            capacity,
            active: false,
            buffered_agent_message_delta_bytes: 0,
            delegated_turns: VecDeque::new(),
            recap_progress: recap::RecapProgress::default(),
        }
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new_with_session(
        capacity: usize,
        session: ThreadSessionState,
        turns: Vec<Turn>,
    ) -> Self {
        let mut store = Self::new(capacity);
        store.session = Some(session);
        store.set_turns(turns);
        store
    }

    pub(super) fn set_session(&mut self, session: ThreadSessionState, turns: Vec<Turn>) {
        self.session = Some(session);
        self.set_turns(turns);
    }

    pub(super) fn rebase_buffer_after_session_refresh(&mut self) {
        self.buffer.retain_mut(Self::event_survives_session_refresh);
        self.buffered_agent_message_delta_bytes = 0;
    }

    pub(super) fn set_turns(&mut self, turns: Vec<Turn>) {
        if self.active_reasoning_item.as_ref().is_some_and(|started| {
            turns.iter().any(|turn| {
                (turn.id == started.turn_id && turn.status != TurnStatus::InProgress)
                    || (turn.id != started.turn_id && turn.status == TurnStatus::InProgress)
            })
        }) {
            self.active_reasoning_item = None;
        }
        self.recap_progress
            .merge(recap::RecapProgress::from_turns(&turns));
        self.active_turn_id = turns
            .iter()
            .rev()
            .find(|turn| matches!(turn.status, TurnStatus::InProgress))
            .map(|turn| turn.id.clone());
        self.latest_turn_id = turns.last().map(|turn| turn.id.clone());
        self.turns = turns;
    }

    pub(super) fn set_active_turn_id(&mut self, turn_id: String) {
        self.latest_turn_id = Some(turn_id.clone());
        self.active_turn_id = Some(turn_id);
    }

    pub(super) fn push_notification(&mut self, notification: ServerNotification) {
        self.push_notification_inner(Cow::Owned(notification));
    }

    pub(super) fn push_notification_ref(&mut self, notification: &ServerNotification) {
        self.push_notification_inner(Cow::Borrowed(notification));
    }

    fn push_notification_inner(&mut self, notification: Cow<'_, ServerNotification>) {
        let user_item = match notification.as_ref() {
            ServerNotification::ItemStarted(n) => Some((&n.turn_id, &n.item)),
            ServerNotification::ItemCompleted(n) => Some((&n.turn_id, &n.item)),
            _ => None,
        };
        if let Some((turn_id, ThreadItem::UserMessage { content, .. })) = user_item
            && turn_id.len() <= 512
            && crate::chatwidget::realtime_delegation_input(content).is_some()
            && !self.delegated_turns.contains(turn_id)
        {
            self.delegated_turns.push_back(turn_id.clone());
        }
        self.pending_interactive_replay
            .note_server_notification(notification.as_ref());
        match notification.as_ref() {
            ServerNotification::TurnStarted(turn) => {
                self.set_active_turn_id(turn.turn.id.clone());
                if self
                    .active_reasoning_item
                    .as_ref()
                    .is_some_and(|started| started.turn_id != turn.turn.id)
                {
                    self.active_reasoning_item = None;
                }
            }
            ServerNotification::TurnCompleted(turn) => {
                if self
                    .active_reasoning_item
                    .as_ref()
                    .is_some_and(|started| started.turn_id == turn.turn.id)
                {
                    self.active_reasoning_item = None;
                }
                if self.active_turn_id.is_none() {
                    self.latest_turn_id = Some(turn.turn.id.clone());
                }
                if matches!(turn.turn.status, TurnStatus::Completed) {
                    self.recap_progress.completed_turns += 1;
                }
                if self.active_turn_id.as_deref() == Some(turn.turn.id.as_str()) {
                    self.active_turn_id = None;
                }
                if self.pending_interrupt_turn_id.as_deref() == Some(turn.turn.id.as_str()) {
                    self.pending_interrupt_turn_id = None;
                }
            }
            ServerNotification::Error(n)
                if self.active_turn_id.is_none()
                    && !n.will_retry
                    && n.error.codex_error_info
                        == Some(AppServerCodexErrorInfo::MisalignmentPolicyViolation) =>
            {
                self.latest_turn_id = Some(n.turn_id.clone());
            }
            ServerNotification::ThreadClosed(_) => {
                self.active_turn_id = None;
                self.active_reasoning_item = None;
                self.pending_interrupt_turn_id = None;
            }
            ServerNotification::ItemStarted(
                started @ codex_app_server_protocol::ItemStartedNotification {
                    item: ThreadItem::Reasoning { id, .. },
                    ..
                },
            ) if !self.delegated_turns.contains(&started.turn_id)
                && !self.turns.iter().any(|turn| {
                    turn.id == started.turn_id && turn.items.iter().any(is_voice_handoff_item)
                }) =>
            {
                self.active_reasoning_item =
                    Some(codex_app_server_protocol::ItemStartedNotification {
                        thread_id: started.thread_id.clone(),
                        turn_id: started.turn_id.clone(),
                        item: ThreadItem::Reasoning {
                            id: id.clone(),
                            summary: Vec::new(),
                            content: Vec::new(),
                        },
                        started_at_ms: started.started_at_ms,
                    });
            }
            ServerNotification::ItemCompleted(completed)
                if matches!(&completed.item, ThreadItem::Reasoning { .. })
                    && self.active_reasoning_item.as_ref().is_some_and(|started| {
                        started.turn_id == completed.turn_id
                            && started.item.id() == completed.item.id()
                    }) =>
            {
                self.active_reasoning_item = None;
            }
            _ => {}
        }

        // These notifications are either handled before routing or ignored by ChatWidget on
        // replay. In particular, raw response items and realtime audio can carry large payloads,
        // so cloning them into every thread's replay buffer only retains data the TUI cannot use.
        if matches!(
            notification.as_ref(),
            ServerNotification::RawResponseItemCompleted(_)
                | ServerNotification::FileChangePatchUpdated(_)
                | ServerNotification::ServerRequestResolved(_)
                | ServerNotification::McpToolCallProgress(_)
                | ServerNotification::ThreadAttachmentUpdated(_)
                | ServerNotification::ThreadRealtimeItemAdded(_)
                | ServerNotification::ThreadRealtimeOutputAudioDelta(_)
                | ServerNotification::ThreadRealtimeSdp(_)
                | ServerNotification::ThreadRealtimeTranscriptDelta(_)
                | ServerNotification::ThreadRealtimeTranscriptDone(_)
                | ServerNotification::CommandExecOutputDelta(_)
                | ServerNotification::ProcessOutputDelta(_)
                | ServerNotification::ProcessExited(_)
        ) {
            return;
        }

        self.push_replay_notification(notification);
        self.trim_delegated_turns();
    }

    fn trim_delegated_turns(&mut self) {
        while self.delegated_turns.len() > self.capacity {
            // Keep an evicted user marker while this turn still has buffered output.
            let stale = self.delegated_turns.iter().position(|turn_id| {
                !self.buffer.iter().any(|event| {
                    matches!(event, ThreadBufferedEvent::Notification(n) if match n.as_ref() {
                        ServerNotification::ItemCompleted(item) => &item.turn_id == turn_id,
                        ServerNotification::AgentMessageDelta(delta) => &delta.turn_id == turn_id,
                        ServerNotification::ReasoningSummaryTextDelta(delta) => &delta.turn_id == turn_id,
                        ServerNotification::ReasoningTextDelta(delta) => &delta.turn_id == turn_id,
                        ServerNotification::ReasoningSummaryPartAdded(part) => &part.turn_id == turn_id,
                        ServerNotification::TurnCompleted(turn) => &turn.turn.id == turn_id,
                        _ => false,
                    })
                })
            });
            self.delegated_turns.remove(stale.unwrap_or(0));
        }
    }

    pub(super) fn push_request(&mut self, request: ServerRequest) {
        self.pending_interactive_replay
            .note_server_request(&request);
        self.push_buffered_event(ThreadBufferedEvent::Request(Box::new(request)));
    }

    pub(super) fn pending_replay_requests(&self) -> Vec<ServerRequest> {
        self.buffer
            .iter()
            .filter_map(|event| match event {
                ThreadBufferedEvent::Request(request)
                    if self
                        .pending_interactive_replay
                        .should_replay_snapshot_request(request.as_ref()) =>
                {
                    Some(request.as_ref().clone())
                }
                ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::Notification(_)
                | ThreadBufferedEvent::HistoryEntryResponse(_)
                | ThreadBufferedEvent::FeedbackSubmission(_) => None,
            })
            .collect()
    }

    pub(super) fn file_change_changes(
        &self,
        turn_id: &str,
        item_id: &str,
    ) -> Option<Vec<codex_app_server_protocol::FileUpdateChange>> {
        file_change_changes(self.buffer.iter(), &self.turns, turn_id, item_id)
    }

    pub(super) fn snapshot(&self) -> ThreadEventSnapshot {
        let mut snapshot = ThreadEventSnapshot {
            session: self.session.clone(),
            delegated_turns: self.delegated_turns.iter().cloned().collect(),
            turns: self.turns.clone(),
            // Thread switches replay buffered events into a rebuilt ChatWidget. Only replay
            // interactive prompts that are still pending, or answered approvals/input will reappear.
            events: self
                .buffer
                .iter()
                .filter(|event| match event {
                    ThreadBufferedEvent::Request(request) => self
                        .pending_interactive_replay
                        .should_replay_snapshot_request(request.as_ref()),
                    ThreadBufferedEvent::Notification(_)
                    | ThreadBufferedEvent::HistoryEntryResponse(_)
                    | ThreadBufferedEvent::FeedbackSubmission(_) => true,
                })
                .cloned()
                .collect(),
            active_reasoning_item: self.active_reasoning_item.clone(),
            input_state: self.input_state.clone(),
        };
        if let Some(latest_turn_id) = &self.latest_turn_id {
            replay_filter::omit_resolved_misalignment_errors(&mut snapshot, latest_turn_id);
        }
        let delegated = snapshot
            .turns
            .iter()
            .filter(|turn| {
                turn.items.iter().any(|item| {
                    matches!(item, ThreadItem::UserMessage { content, .. }
                    if crate::chatwidget::realtime_delegation_input(content).is_some())
                })
            })
            .map(|turn| turn.id.clone())
            .chain(self.delegated_turns.iter().cloned())
            .collect::<std::collections::HashSet<_>>();
        for turn in &mut snapshot.turns {
            hide_private_items_after_voice_handoff(&mut turn.items, delegated.contains(&turn.id));
        }
        let buffered_markers = snapshot
            .events
            .iter()
            .filter_map(|event| match event {
                ThreadBufferedEvent::Notification(notification) => match notification.as_ref() {
                    ServerNotification::ItemStarted(n) if is_voice_handoff_item(&n.item) => {
                        Some(n.turn_id.clone())
                    }
                    ServerNotification::ItemCompleted(n) if is_voice_handoff_item(&n.item) => {
                        Some(n.turn_id.clone())
                    }
                    _ => None,
                },
                _ => None,
            })
            .collect::<std::collections::HashSet<_>>();
        let mut voice_started = delegated
            .difference(&buffered_markers)
            .cloned()
            .collect::<std::collections::HashSet<_>>();
        let mut typed_items = self
            .active_reasoning_item
            .iter()
            .map(|started| (started.turn_id.clone(), started.item.id().to_string()))
            .collect::<std::collections::HashSet<_>>();
        snapshot.events.retain_mut(|event| {
            let ThreadBufferedEvent::Notification(notification) = event else {
                return true;
            };
            match notification.as_ref() {
                ServerNotification::ItemStarted(n) if is_voice_handoff_item(&n.item) => {
                    voice_started.insert(n.turn_id.clone());
                }
                ServerNotification::ItemCompleted(n) if is_voice_handoff_item(&n.item) => {
                    voice_started.insert(n.turn_id.clone());
                }
                ServerNotification::ItemStarted(n) => {
                    if let ThreadItem::AgentMessage { id, .. } | ThreadItem::Reasoning { id, .. } =
                        &n.item
                    {
                        let key = (n.turn_id.clone(), id.clone());
                        if voice_started.contains(&n.turn_id) {
                            typed_items.remove(&key);
                        } else {
                            typed_items.insert(key);
                        }
                    }
                }
                _ => {}
            }
            match notification.as_mut() {
                ServerNotification::AgentMessageDelta(n) => {
                    !voice_started.contains(&n.turn_id)
                        || typed_items.contains(&(n.turn_id.clone(), n.item_id.clone()))
                }
                ServerNotification::ReasoningSummaryTextDelta(n) => {
                    !voice_started.contains(&n.turn_id)
                        || typed_items.contains(&(n.turn_id.clone(), n.item_id.clone()))
                }
                ServerNotification::ReasoningTextDelta(n) => {
                    !voice_started.contains(&n.turn_id)
                        || typed_items.contains(&(n.turn_id.clone(), n.item_id.clone()))
                }
                ServerNotification::ReasoningSummaryPartAdded(n) => {
                    !voice_started.contains(&n.turn_id)
                        || typed_items.contains(&(n.turn_id.clone(), n.item_id.clone()))
                }
                ServerNotification::ItemCompleted(n) if voice_started.contains(&n.turn_id) => {
                    matches!(&n.item, ThreadItem::AgentMessage { id, .. } | ThreadItem::Reasoning { id, .. }
                        if typed_items.contains(&(n.turn_id.clone(), id.clone())))
                        || !crate::chatwidget::is_private_realtime_agent_item(&n.item)
                }
                ServerNotification::TurnCompleted(n) if delegated.contains(&n.turn.id) => {
                    hide_private_items_after_voice_handoff(
                        &mut n.turn.items,
                        /*delegated*/ true,
                    );
                    true
                }
                _ => true,
            }
        });
        snapshot
    }

    pub(super) fn recap_progress(&self) -> recap::RecapProgress {
        self.recap_progress
    }

    pub(super) fn merge_recap_progress(&mut self, progress: recap::RecapProgress) {
        self.recap_progress.merge(progress);
    }

    pub(super) fn note_outbound_op<T>(&mut self, op: T)
    where
        T: Into<AppCommand>,
    {
        self.pending_interactive_replay.note_outbound_op(op);
    }

    pub(super) fn op_can_change_pending_replay_state<T>(op: T) -> bool
    where
        T: Into<AppCommand>,
    {
        PendingInteractiveReplayState::op_can_change_state(op)
    }

    pub(super) fn has_pending_thread_approvals(&self) -> bool {
        self.pending_interactive_replay
            .has_pending_thread_approvals()
    }

    pub(super) fn side_parent_pending_status(&self) -> Option<SideParentStatus> {
        if self
            .pending_interactive_replay
            .has_pending_thread_user_input()
        {
            Some(SideParentStatus::NeedsInput)
        } else if self
            .pending_interactive_replay
            .has_pending_thread_approvals()
        {
            Some(SideParentStatus::NeedsApproval)
        } else {
            None
        }
    }

    pub(super) fn active_turn_id(&self) -> Option<&str> {
        self.active_turn_id.as_deref()
    }

    pub(super) fn clear_active_turn_id(&mut self) {
        self.active_turn_id = None;
    }
}

fn turn_id_matches(request_turn_id: &str, candidate_turn_id: &str) -> bool {
    request_turn_id.is_empty() || request_turn_id == candidate_turn_id
}

pub(super) fn file_change_changes<'a>(
    events: impl DoubleEndedIterator<Item = &'a ThreadBufferedEvent>,
    turns: &'a [Turn],
    turn_id: &str,
    item_id: &str,
) -> Option<Vec<codex_app_server_protocol::FileUpdateChange>> {
    let event_items = events.rev().filter_map(|event| {
        let ThreadBufferedEvent::Notification(notification) = event else {
            return None;
        };
        let (candidate_turn_id, item) = match notification.as_ref() {
            ServerNotification::ItemStarted(notification) => {
                (&notification.turn_id, &notification.item)
            }
            ServerNotification::ItemCompleted(notification) => {
                (&notification.turn_id, &notification.item)
            }
            _ => return None,
        };
        turn_id_matches(turn_id, candidate_turn_id).then_some(item)
    });
    event_items
        .chain(
            turns
                .iter()
                .rev()
                .filter(|turn| turn_id_matches(turn_id, &turn.id))
                .flat_map(|turn| turn.items.iter().rev()),
        )
        .find_map(|item| file_change_item_changes(item, item_id))
}

fn file_change_item_changes(
    item: &ThreadItem,
    item_id: &str,
) -> Option<Vec<codex_app_server_protocol::FileUpdateChange>> {
    match item {
        ThreadItem::FileChange { id, changes, .. } if id == item_id => Some(changes.clone()),
        _ => None,
    }
}

#[derive(Debug)]
pub(super) struct ThreadEventChannel {
    pub(super) sender: mpsc::Sender<ThreadBufferedEvent>,
    pub(super) receiver: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    pub(super) store: Arc<Mutex<ThreadEventStore>>,
    attachment: ThreadEventAttachment,
}

impl ThreadEventChannel {
    pub(super) fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new(capacity))),
            attachment: ThreadEventAttachment::Live,
        }
    }

    pub(super) fn mark_replay_only(&mut self) {
        self.attachment = ThreadEventAttachment::ReplayOnly;
    }

    pub(super) fn mark_external_writer(&mut self) {
        self.attachment = ThreadEventAttachment::ExternalWriter;
    }

    pub(super) fn attachment(&self) -> ThreadEventAttachment {
        self.attachment
    }

    #[cfg_attr(not(test), allow(dead_code))]
    pub(super) fn new_with_session(
        capacity: usize,
        session: ThreadSessionState,
        turns: Vec<Turn>,
    ) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new_with_session(
                capacity, session, turns,
            ))),
            attachment: ThreadEventAttachment::Live,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
    use codex_app_server_protocol::AskForApproval;
    use codex_app_server_protocol::CommandExecutionRequestApprovalParams;
    use codex_app_server_protocol::HookCompletedNotification;
    use codex_app_server_protocol::HookEventName as AppServerHookEventName;
    use codex_app_server_protocol::HookExecutionMode as AppServerHookExecutionMode;
    use codex_app_server_protocol::HookHandlerType as AppServerHookHandlerType;
    use codex_app_server_protocol::HookOutputEntry as AppServerHookOutputEntry;
    use codex_app_server_protocol::HookOutputEntryKind as AppServerHookOutputEntryKind;
    use codex_app_server_protocol::HookRunStatus as AppServerHookRunStatus;
    use codex_app_server_protocol::HookRunSummary as AppServerHookRunSummary;
    use codex_app_server_protocol::HookScope as AppServerHookScope;
    use codex_app_server_protocol::HookStartedNotification;
    use codex_app_server_protocol::ItemCompletedNotification;
    use codex_app_server_protocol::ItemStartedNotification;
    use codex_app_server_protocol::McpToolCallProgressNotification;
    use codex_app_server_protocol::ReasoningSummaryTextDeltaNotification;
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::ThreadAttachmentOperation;
    use codex_app_server_protocol::ThreadAttachmentUpdatedNotification;
    use codex_app_server_protocol::ThreadRealtimeAudioChunk;
    use codex_app_server_protocol::ThreadRealtimeOutputAudioDeltaNotification;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnStartedNotification;
    use codex_app_server_protocol::UserInput;
    use codex_config::types::ApprovalsReviewer;
    use codex_protocol::models::PermissionProfile;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn test_thread_session(thread_id: ThreadId, cwd: PathBuf) -> ThreadSessionState {
        ThreadSessionState {
            thread_id,
            forked_from_id: None,
            fork_parent_title: None,
            thread_name: None,
            model: "gpt-test".to_string(),
            model_provider_id: "test-provider".to_string(),
            service_tier: None,
            approval_policy: AskForApproval::Never,
            approvals_reviewer: ApprovalsReviewer::User,
            permission_profile: PermissionProfile::read_only(),
            active_permission_profile: None,
            cwd: cwd.abs(),
            runtime_workspace_roots: Vec::new(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            collaboration_mode: None,
            personality: None,
            message_history: None,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }
    }

    fn test_turn(turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
        Turn {
            id: turn_id.to_string(),
            items_view: codex_app_server_protocol::TurnItemsView::Full,
            items,
            status,
            error: None,
            started_at: None,
            completed_at: None,
            duration_ms: None,
        }
    }

    fn turn_started_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::TurnStarted(TurnStartedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                started_at: Some(0),
                ..test_turn(turn_id, TurnStatus::InProgress, Vec::new())
            },
        })
    }

    fn turn_completed_notification(
        thread_id: ThreadId,
        turn_id: &str,
        status: TurnStatus,
    ) -> ServerNotification {
        ServerNotification::TurnCompleted(TurnCompletedNotification {
            thread_id: thread_id.to_string(),
            turn: Turn {
                completed_at: Some(0),
                duration_ms: Some(1),
                ..test_turn(turn_id, status, Vec::new())
            },
        })
    }

    fn hook_started_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::HookStarted(HookStartedNotification {
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            run: AppServerHookRunSummary {
                id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
                event_name: AppServerHookEventName::UserPromptSubmit,
                handler_type: AppServerHookHandlerType::Command,
                execution_mode: AppServerHookExecutionMode::Sync,
                scope: AppServerHookScope::Turn,
                source_path: test_path_buf("/tmp/hooks.json").abs(),
                source: codex_app_server_protocol::HookSource::User,
                display_order: 0,
                status: AppServerHookRunStatus::Running,
                status_message: Some("checking go-workflow input policy".to_string()),
                started_at: 1,
                completed_at: None,
                duration_ms: None,
                entries: Vec::new(),
            },
        })
    }

    fn hook_completed_notification(thread_id: ThreadId, turn_id: &str) -> ServerNotification {
        ServerNotification::HookCompleted(HookCompletedNotification {
            thread_id: thread_id.to_string(),
            turn_id: Some(turn_id.to_string()),
            run: AppServerHookRunSummary {
                id: "user-prompt-submit:0:/tmp/hooks.json".to_string(),
                event_name: AppServerHookEventName::UserPromptSubmit,
                handler_type: AppServerHookHandlerType::Command,
                execution_mode: AppServerHookExecutionMode::Sync,
                scope: AppServerHookScope::Turn,
                source_path: test_path_buf("/tmp/hooks.json").abs(),
                source: codex_app_server_protocol::HookSource::User,
                display_order: 0,
                status: AppServerHookRunStatus::Stopped,
                status_message: Some("checking go-workflow input policy".to_string()),
                started_at: 1,
                completed_at: Some(11),
                duration_ms: Some(10),
                entries: vec![
                    AppServerHookOutputEntry {
                        kind: AppServerHookOutputEntryKind::Warning,
                        text: "go-workflow must start from PlanMode".to_string(),
                    },
                    AppServerHookOutputEntry {
                        kind: AppServerHookOutputEntryKind::Stop,
                        text: "prompt blocked".to_string(),
                    },
                ],
            },
        })
    }

    fn exec_approval_request(
        thread_id: ThreadId,
        turn_id: &str,
        item_id: &str,
        approval_id: Option<&str>,
    ) -> ServerRequest {
        ServerRequest::CommandExecutionRequestApproval {
            request_id: AppServerRequestId::Integer(1),
            params: CommandExecutionRequestApprovalParams {
                kind: Default::default(),
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
                item_id: item_id.to_string(),
                started_at_ms: 0,
                approval_id: approval_id.map(str::to_string),
                environment_id: None,
                reason: Some("needs approval".to_string()),
                network_approval_context: None,
                command: Some("echo hello".to_string()),
                cwd: Some(test_path_buf("/tmp/project").abs().into()),
                command_actions: None,
                additional_permissions: None,
                proposed_execpolicy_amendment: None,
                proposed_network_policy_amendments: None,
                available_decisions: None,
            },
        }
    }

    #[test]
    fn thread_event_store_tracks_active_turn_lifecycle() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        assert_eq!(store.active_turn_id(), None);

        let thread_id = ThreadId::new();
        store.push_notification(turn_started_notification(thread_id, "turn-1"));
        assert_eq!(store.active_turn_id(), Some("turn-1"));

        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-2",
            TurnStatus::Completed,
        ));
        assert_eq!(store.active_turn_id(), Some("turn-1"));

        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-1",
            TurnStatus::Interrupted,
        ));
        assert_eq!(store.active_turn_id(), None);
    }

    #[test]
    fn thread_event_store_preserves_recap_progress_across_replay() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.set_turns(vec![
            test_turn("turn-1", TurnStatus::Completed, Vec::new()),
            test_turn("turn-2", TurnStatus::Failed, Vec::new()),
        ]);
        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-3",
            TurnStatus::Completed,
        ));
        store.push_notification(turn_completed_notification(
            thread_id,
            "turn-4",
            TurnStatus::Interrupted,
        ));
        store.merge_recap_progress(recap::RecapProgress {
            completed_turns: 2,
            last_recapped_turn_count: Some(2),
        });

        assert_eq!(
            store.recap_progress(),
            recap::RecapProgress {
                completed_turns: 2,
                last_recapped_turn_count: Some(2),
            }
        );
    }

    #[test]
    fn thread_event_store_restores_active_turn_from_snapshot_turns() {
        let thread_id = ThreadId::new();
        let session = test_thread_session(thread_id, test_path_buf("/tmp/project"));
        let turns = vec![
            test_turn("turn-1", TurnStatus::Completed, Vec::new()),
            test_turn("turn-2", TurnStatus::InProgress, Vec::new()),
        ];

        let store =
            ThreadEventStore::new_with_session(/*capacity*/ 8, session.clone(), turns.clone());
        assert_eq!(store.active_turn_id(), Some("turn-2"));

        let mut refreshed_store = ThreadEventStore::new(/*capacity*/ 8);
        refreshed_store.set_session(session, turns);
        assert_eq!(refreshed_store.active_turn_id(), Some("turn-2"));
    }

    #[test]
    fn thread_event_store_clear_active_turn_id_resets_cached_turn() {
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        let thread_id = ThreadId::new();
        store.push_notification(turn_started_notification(thread_id, "turn-1"));

        store.clear_active_turn_id();

        assert_eq!(store.active_turn_id(), None);
    }

    #[test]
    fn thread_event_store_skips_large_replay_irrelevant_notifications() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 2);
        store.push_notification(turn_started_notification(thread_id, "turn-1"));
        store.push_request(exec_approval_request(
            thread_id,
            "turn-1",
            "command-approval",
            /*approval_id*/ None,
        ));
        let large_payload = "x".repeat(1024 * 1024);

        for _ in 0..32 {
            store.push_notification_ref(&ServerNotification::ThreadAttachmentUpdated(
                ThreadAttachmentUpdatedNotification {
                    thread_id: thread_id.to_string(),
                    attachment_type: "pullRequest".to_string(),
                    identity_key: "openai/codex#1".to_string(),
                    attachment_id: "attachment-1".to_string(),
                    operation: ThreadAttachmentOperation::Created,
                },
            ));
            store.push_notification_ref(&ServerNotification::McpToolCallProgress(
                McpToolCallProgressNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: "turn-1".to_string(),
                    item_id: "mcp-1".to_string(),
                    message: large_payload.clone(),
                },
            ));
            store.push_notification_ref(&ServerNotification::ThreadRealtimeOutputAudioDelta(
                ThreadRealtimeOutputAudioDeltaNotification {
                    thread_id: thread_id.to_string(),
                    audio: ThreadRealtimeAudioChunk {
                        data: large_payload.clone(),
                        sample_rate: 24_000,
                        num_channels: 1,
                        samples_per_channel: None,
                        item_id: None,
                    },
                },
            ));
        }

        assert_eq!(store.buffer.len(), 2);
        assert!(store.has_pending_thread_approvals());
        assert_eq!(store.active_turn_id(), Some("turn-1"));
    }

    #[test]
    fn thread_event_store_rebase_preserves_resolved_request_state() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_request(exec_approval_request(
            thread_id,
            "turn-approval",
            "call-approval",
            /*approval_id*/ None,
        ));
        store.push_notification(ServerNotification::ServerRequestResolved(
            codex_app_server_protocol::ServerRequestResolvedNotification {
                request_id: AppServerRequestId::Integer(1),
                thread_id: thread_id.to_string(),
            },
        ));

        store.rebase_buffer_after_session_refresh();

        let snapshot = store.snapshot();
        assert!(snapshot.events.is_empty());
        assert_eq!(store.has_pending_thread_approvals(), false);
    }

    #[test]
    fn refresh_retains_live_questions_without_replaying_their_text() {
        let mut expected = serde_json::json!({
            "method": "item/completed", "params": {
                "threadId": "thread", "turnId": "turn", "completedAtMs": 0,
                "item": {
                    "type": "agentMessage", "id": "question", "text": "already in the snapshot",
                    "phase": null, "memoryCitation": null, "delivery": null,
                    "questions": [{"title": "Which way?", "options": null}]
                }
            }
        });
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_notification(serde_json::from_value(expected.clone()).unwrap());
        store.rebase_buffer_after_session_refresh();
        expected["params"]["item"]["text"] = serde_json::json!("");
        let ThreadBufferedEvent::Notification(actual) = &store.snapshot().events[0] else {
            panic!("missing live question");
        };
        assert_eq!(serde_json::to_value(actual).unwrap(), expected);
    }

    #[test]
    fn thread_event_store_rebase_preserves_hook_notifications() {
        let thread_id = ThreadId::new();
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_notification(hook_started_notification(thread_id, "turn-hook"));
        store.push_notification(hook_completed_notification(thread_id, "turn-hook"));

        store.rebase_buffer_after_session_refresh();

        let snapshot = store.snapshot();
        let hook_notifications = snapshot
            .events
            .into_iter()
            .map(|event| match event {
                ThreadBufferedEvent::Notification(notification) => {
                    serde_json::to_value(notification).expect("hook notification should serialize")
                }
                other => panic!("expected buffered hook notification, saw: {other:?}"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            hook_notifications,
            vec![
                serde_json::to_value(hook_started_notification(thread_id, "turn-hook"))
                    .expect("hook notification should serialize"),
                serde_json::to_value(hook_completed_notification(thread_id, "turn-hook"))
                    .expect("hook notification should serialize"),
            ]
        );
    }

    #[test]
    fn thread_event_store_rebase_preserves_mcp_startup_notifications() {
        let thread_id = ThreadId::new();
        let notification = ServerNotification::McpServerStatusUpdated(
            codex_app_server_protocol::McpServerStatusUpdatedNotification {
                thread_id: Some(thread_id.to_string()),
                name: "sentry".to_string(),
                status: codex_app_server_protocol::McpServerStartupState::Failed,
                error: Some("sentry is not logged in".to_string()),
                failure_reason: None,
            },
        );
        let mut store = ThreadEventStore::new(/*capacity*/ 8);
        store.push_notification_ref(&notification);

        store.rebase_buffer_after_session_refresh();

        let snapshot = store.snapshot();
        let actual = match snapshot.events.as_slice() {
            [ThreadBufferedEvent::Notification(actual)] => actual,
            other => panic!("expected one buffered MCP notification, saw: {other:?}"),
        };
        assert_eq!(
            serde_json::to_value(actual).expect("MCP notification should serialize"),
            serde_json::to_value(notification).expect("MCP notification should serialize"),
        );
    }

    #[test]
    fn buffered_voice_handoff_retains_only_preexisting_typed_reasoning() {
        let thread_id = ThreadId::new().to_string();
        let turn_id = "mixed-reasoning-turn".to_string();
        let mut store = ThreadEventStore::new(/*capacity*/ 16);
        let typed = ThreadItem::Reasoning {
            id: "typed-reasoning".into(),
            summary: Vec::new(),
            content: Vec::new(),
        };
        store.push_notification(ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            item: typed,
            started_at_ms: 0,
        }));
        store.push_notification(ServerNotification::ItemStarted(ItemStartedNotification {
            thread_id: thread_id.clone(),
            turn_id: turn_id.clone(),
            item: ThreadItem::UserMessage {
                id: "voice-marker".into(),
                client_id: None,
                content: vec![UserInput::Text {
                    text:
                        "<realtime_delegation><input>spoken follow-up</input></realtime_delegation>"
                            .into(),
                    text_elements: Vec::new(),
                }],
            },
            started_at_ms: 0,
        }));
        for (id, text) in [
            ("typed-reasoning", "Typed tail"),
            ("private-reasoning", "Private voice reasoning"),
        ] {
            if id == "private-reasoning" {
                store.push_notification(ServerNotification::ItemStarted(ItemStartedNotification {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item: ThreadItem::Reasoning {
                        id: id.into(),
                        summary: Vec::new(),
                        content: Vec::new(),
                    },
                    started_at_ms: 0,
                }));
            }
            store.push_notification(ServerNotification::ReasoningSummaryTextDelta(
                ReasoningSummaryTextDeltaNotification {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item_id: id.into(),
                    delta: text.into(),
                    summary_index: 0,
                },
            ));
            store.push_notification(ServerNotification::ItemCompleted(
                ItemCompletedNotification {
                    thread_id: thread_id.clone(),
                    turn_id: turn_id.clone(),
                    item: ThreadItem::Reasoning {
                        id: id.into(),
                        summary: vec![text.into()],
                        content: Vec::new(),
                    },
                    completed_at_ms: 0,
                },
            ));
        }

        let retained = store
            .snapshot()
            .events
            .into_iter()
            .filter_map(|event| match event {
                ThreadBufferedEvent::Notification(notification) => match *notification {
                    ServerNotification::ReasoningSummaryTextDelta(n) => Some(n.delta),
                    ServerNotification::ItemCompleted(n) => match n.item {
                        ThreadItem::Reasoning { summary, .. } => Some(summary.join("")),
                        _ => None,
                    },
                    _ => None,
                },
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(retained, vec!["Typed tail", "Typed tail"]);
    }
}
