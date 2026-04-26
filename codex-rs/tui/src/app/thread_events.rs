//! Thread event buffering and replay state for the TUI app.
//!
//! This module owns the per-thread event store used when the TUI switches between the main
//! conversation, subagents, and side conversations. It keeps buffered app-server notifications,
//! pending interactive request replay state, active-turn tracking, and saved composer state close
//! together with the replay behavior that consumes them.

use super::*;

#[derive(Debug, Clone)]
pub(super) struct ThreadEventSnapshot {
    pub(super) session: Option<ThreadSessionState>,
    pub(super) turns: Vec<Turn>,
    pub(super) events: Vec<ThreadBufferedEvent>,
    pub(super) input_state: Option<ThreadInputState>,
}

#[derive(Debug, Clone)]
pub(super) enum ThreadBufferedEvent {
    Notification(ServerNotification),
    Request(ServerRequest),
    HistoryEntryResponse(GetHistoryEntryResponseEvent),
    FeedbackSubmission(FeedbackThreadEvent),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FeedbackThreadEvent {
    pub(super) category: FeedbackCategory,
    pub(super) include_logs: bool,
    pub(super) feedback_audience: FeedbackAudience,
    pub(super) result: Result<String, String>,
}

#[derive(Debug)]
pub(super) struct ThreadEventStore {
    pub(super) session: Option<ThreadSessionState>,
    pub(super) turns: Vec<Turn>,
    pub(super) buffer: VecDeque<ThreadBufferedEvent>,
    pub(super) pending_interactive_replay: PendingInteractiveReplayState,
    pub(super) active_turn_id: Option<String>,
    pub(super) input_state: Option<ThreadInputState>,
    pub(super) capacity: usize,
    pub(super) active: bool,
}

impl ThreadEventStore {
    pub(super) fn event_survives_session_refresh(event: &ThreadBufferedEvent) -> bool {
        matches!(
            event,
            ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::Notification(ServerNotification::HookStarted(_))
                | ThreadBufferedEvent::Notification(ServerNotification::HookCompleted(_))
                | ThreadBufferedEvent::FeedbackSubmission(_)
        )
    }

    pub(super) fn new(capacity: usize) -> Self {
        Self {
            session: None,
            turns: Vec::new(),
            buffer: VecDeque::new(),
            pending_interactive_replay: PendingInteractiveReplayState::default(),
            active_turn_id: None,
            input_state: None,
            capacity,
            active: false,
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
        self.buffer.retain(Self::event_survives_session_refresh);
    }

    pub(super) fn set_turns(&mut self, turns: Vec<Turn>) {
        self.active_turn_id = turns
            .iter()
            .rev()
            .find(|turn| matches!(turn.status, TurnStatus::InProgress))
            .map(|turn| turn.id.clone());
        self.turns = turns;
    }

    pub(super) fn push_notification(&mut self, notification: ServerNotification) {
        self.pending_interactive_replay
            .note_server_notification(&notification);
        match &notification {
            ServerNotification::TurnStarted(turn) => {
                self.active_turn_id = Some(turn.turn.id.clone());
            }
            ServerNotification::TurnCompleted(turn)
                if self.active_turn_id.as_deref() == Some(turn.turn.id.as_str()) =>
            {
                self.active_turn_id = None;
            }
            ServerNotification::ThreadClosed(_) => {
                self.active_turn_id = None;
            }
            _ => {}
        }
        self.buffer
            .push_back(ThreadBufferedEvent::Notification(notification));
        if self.buffer.len() > self.capacity
            && let Some(removed) = self.buffer.pop_front()
            && let ThreadBufferedEvent::Request(request) = &removed
        {
            self.pending_interactive_replay
                .note_evicted_server_request(request);
        }
    }

    pub(super) fn push_request(&mut self, request: ServerRequest) {
        self.pending_interactive_replay
            .note_server_request(&request);
        self.buffer.push_back(ThreadBufferedEvent::Request(request));
        if self.buffer.len() > self.capacity
            && let Some(removed) = self.buffer.pop_front()
            && let ThreadBufferedEvent::Request(request) = &removed
        {
            self.pending_interactive_replay
                .note_evicted_server_request(request);
        }
    }

    pub(super) fn pending_replay_requests(&self) -> Vec<ServerRequest> {
        self.buffer
            .iter()
            .filter_map(|event| match event {
                ThreadBufferedEvent::Request(request)
                    if self
                        .pending_interactive_replay
                        .should_replay_snapshot_request(request) =>
                {
                    Some(request.clone())
                }
                ThreadBufferedEvent::Request(_)
                | ThreadBufferedEvent::Notification(_)
                | ThreadBufferedEvent::HistoryEntryResponse(_)
                | ThreadBufferedEvent::FeedbackSubmission(_) => None,
            })
            .collect()
    }

    pub(super) fn apply_thread_rollback(&mut self, response: &ThreadRollbackResponse) {
        self.turns = response.thread.turns.clone();
        self.buffer.clear();
        self.pending_interactive_replay = PendingInteractiveReplayState::default();
        self.active_turn_id = None;
    }

    pub(super) fn snapshot(&self) -> ThreadEventSnapshot {
        ThreadEventSnapshot {
            session: self.session.clone(),
            turns: self.turns.clone(),
            // Thread switches replay buffered events into a rebuilt ChatWidget. Only replay
            // interactive prompts that are still pending, or answered approvals/input will reappear.
            events: self
                .buffer
                .iter()
                .filter(|event| match event {
                    ThreadBufferedEvent::Request(request) => self
                        .pending_interactive_replay
                        .should_replay_snapshot_request(request),
                    ThreadBufferedEvent::Notification(_)
                    | ThreadBufferedEvent::HistoryEntryResponse(_)
                    | ThreadBufferedEvent::FeedbackSubmission(_) => true,
                })
                .cloned()
                .collect(),
            input_state: self.input_state.clone(),
        }
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

#[derive(Debug)]
pub(super) struct ThreadEventChannel {
    pub(super) sender: mpsc::Sender<ThreadBufferedEvent>,
    pub(super) receiver: Option<mpsc::Receiver<ThreadBufferedEvent>>,
    pub(super) store: Arc<Mutex<ThreadEventStore>>,
}

impl ThreadEventChannel {
    pub(super) fn new(capacity: usize) -> Self {
        let (sender, receiver) = mpsc::channel(capacity);
        Self {
            sender,
            receiver: Some(receiver),
            store: Arc::new(Mutex::new(ThreadEventStore::new(capacity))),
        }
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::PathBufExt;
    use crate::test_support::test_path_buf;
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
    use codex_app_server_protocol::RequestId as AppServerRequestId;
    use codex_app_server_protocol::TurnCompletedNotification;
    use codex_app_server_protocol::TurnStartedNotification;
    use codex_config::types::ApprovalsReviewer;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::SandboxPolicy;
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
            sandbox_policy: SandboxPolicy::new_read_only_policy(),
            permission_profile: Some(PermissionProfile::from_legacy_sandbox_policy(
                &SandboxPolicy::new_read_only_policy(),
            )),
            cwd: cwd.abs(),
            instruction_source_paths: Vec::new(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            network_proxy: None,
            rollout_path: Some(PathBuf::new()),
        }
    }

    fn test_turn(turn_id: &str, status: TurnStatus, items: Vec<ThreadItem>) -> Turn {
        Turn {
            id: turn_id.to_string(),
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
                thread_id: thread_id.to_string(),
                turn_id: turn_id.to_string(),
                item_id: item_id.to_string(),
                approval_id: approval_id.map(str::to_string),
                reason: Some("needs approval".to_string()),
                network_approval_context: None,
                command: Some("echo hello".to_string()),
                cwd: Some(test_path_buf("/tmp/project").abs()),
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
}
