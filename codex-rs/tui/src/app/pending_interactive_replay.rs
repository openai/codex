use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use std::collections::HashMap;
use std::collections::HashSet;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct ElicitationRequestKey {
    server_name: String,
    request_id: codex_protocol::mcp::RequestId,
}

impl ElicitationRequestKey {
    fn new(server_name: String, request_id: codex_protocol::mcp::RequestId) -> Self {
        Self {
            server_name,
            request_id,
        }
    }
}

#[derive(Debug, Default)]
// Tracks which interactive prompts are still unresolved in the thread-event buffer.
//
// Thread snapshots are replayed when switching threads/agents. Most events should replay
// verbatim, but interactive prompts (approvals, request_user_input, MCP elicitations) must
// only replay if they are still pending. This state is updated from:
// - inbound events (`note_event`)
// - outbound ops that resolve a prompt (`note_outbound_op`)
// - buffer eviction (`note_evicted_event`)
//
// We keep both fast lookup sets (for snapshot filtering by call_id/request key) and
// turn-indexed queues/vectors so `TurnComplete`/`TurnAborted` can clear stale prompts tied
// to a turn. `request_user_input` removal is FIFO because the overlay answers queued prompts
// in FIFO order for a shared `turn_id`.
pub(super) struct PendingInteractiveReplayState {
    exec_approval_call_ids: HashSet<String>,
    exec_approval_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    patch_approval_call_ids: HashSet<String>,
    patch_approval_call_ids_by_turn_id: HashMap<String, Vec<String>>,
    elicitation_requests: HashSet<ElicitationRequestKey>,
    request_user_input_call_ids: HashSet<String>,
    request_user_input_call_ids_by_turn_id: HashMap<String, Vec<String>>,
}

impl PendingInteractiveReplayState {
    pub(super) fn event_can_change_pending_thread_approvals(event: &Event) -> bool {
        matches!(
            &event.msg,
            EventMsg::ExecApprovalRequest(_)
                | EventMsg::ApplyPatchApprovalRequest(_)
                | EventMsg::ElicitationRequest(_)
                | EventMsg::ExecCommandBegin(_)
                | EventMsg::PatchApplyBegin(_)
                | EventMsg::TurnComplete(_)
                | EventMsg::TurnAborted(_)
                | EventMsg::ShutdownComplete
        )
    }

    pub(super) fn op_can_change_state(op: &Op) -> bool {
        matches!(
            op,
            Op::ExecApproval { .. }
                | Op::PatchApproval { .. }
                | Op::ResolveElicitation { .. }
                | Op::UserInputAnswer { .. }
                | Op::Shutdown
        )
    }

    pub(super) fn note_outbound_op(&mut self, op: &Op) {
        match op {
            Op::ExecApproval { id, turn_id, .. } => {
                self.exec_approval_call_ids.remove(id);
                if let Some(turn_id) = turn_id {
                    Self::remove_call_id_from_turn_map_entry(
                        &mut self.exec_approval_call_ids_by_turn_id,
                        turn_id,
                        id,
                    );
                }
            }
            Op::PatchApproval { id, .. } => {
                self.patch_approval_call_ids.remove(id);
                Self::remove_call_id_from_turn_map(
                    &mut self.patch_approval_call_ids_by_turn_id,
                    id,
                );
            }
            Op::ResolveElicitation {
                server_name,
                request_id,
                ..
            } => {
                self.elicitation_requests
                    .remove(&ElicitationRequestKey::new(
                        server_name.clone(),
                        request_id.clone(),
                    ));
            }
            // `Op::UserInputAnswer` identifies the turn, not the prompt call_id. The UI
            // answers queued prompts for the same turn in FIFO order, so remove the oldest
            // queued call_id for that turn.
            Op::UserInputAnswer { id, .. } => {
                let mut remove_turn_entry = false;
                if let Some(call_ids) = self.request_user_input_call_ids_by_turn_id.get_mut(id) {
                    if !call_ids.is_empty() {
                        let call_id = call_ids.remove(0);
                        self.request_user_input_call_ids.remove(&call_id);
                    }
                    if call_ids.is_empty() {
                        remove_turn_entry = true;
                    }
                }
                if remove_turn_entry {
                    self.request_user_input_call_ids_by_turn_id.remove(id);
                }
            }
            Op::Shutdown => self.clear(),
            _ => {}
        }
    }

    pub(super) fn note_event(&mut self, event: &Event) {
        match &event.msg {
            EventMsg::ExecApprovalRequest(ev) => {
                let approval_id = ev.effective_approval_id();
                self.exec_approval_call_ids.insert(approval_id.clone());
                self.exec_approval_call_ids_by_turn_id
                    .entry(ev.turn_id.clone())
                    .or_default()
                    .push(approval_id);
            }
            EventMsg::ExecCommandBegin(ev) => {
                self.exec_approval_call_ids.remove(&ev.call_id);
                Self::remove_call_id_from_turn_map(
                    &mut self.exec_approval_call_ids_by_turn_id,
                    &ev.call_id,
                );
            }
            EventMsg::ApplyPatchApprovalRequest(ev) => {
                self.patch_approval_call_ids.insert(ev.call_id.clone());
                self.patch_approval_call_ids_by_turn_id
                    .entry(ev.turn_id.clone())
                    .or_default()
                    .push(ev.call_id.clone());
            }
            EventMsg::PatchApplyBegin(ev) => {
                self.patch_approval_call_ids.remove(&ev.call_id);
                Self::remove_call_id_from_turn_map(
                    &mut self.patch_approval_call_ids_by_turn_id,
                    &ev.call_id,
                );
            }
            EventMsg::ElicitationRequest(ev) => {
                self.elicitation_requests.insert(ElicitationRequestKey::new(
                    ev.server_name.clone(),
                    ev.id.clone(),
                ));
            }
            EventMsg::RequestUserInput(ev) => {
                self.request_user_input_call_ids.insert(ev.call_id.clone());
                self.request_user_input_call_ids_by_turn_id
                    .entry(ev.turn_id.clone())
                    .or_default()
                    .push(ev.call_id.clone());
            }
            // A turn ending (normally or aborted/replaced) invalidates any unresolved
            // turn-scoped approvals and request_user_input prompts from that turn.
            EventMsg::TurnComplete(ev) => {
                self.clear_exec_approval_turn(&ev.turn_id);
                self.clear_patch_approval_turn(&ev.turn_id);
                self.clear_request_user_input_turn(&ev.turn_id);
            }
            EventMsg::TurnAborted(ev) => {
                if let Some(turn_id) = &ev.turn_id {
                    self.clear_exec_approval_turn(turn_id);
                    self.clear_patch_approval_turn(turn_id);
                    self.clear_request_user_input_turn(turn_id);
                }
            }
            EventMsg::ShutdownComplete => self.clear(),
            _ => {}
        }
    }

    pub(super) fn note_evicted_event(&mut self, event: &Event) {
        match &event.msg {
            EventMsg::ExecApprovalRequest(ev) => {
                let approval_id = ev.effective_approval_id();
                self.exec_approval_call_ids.remove(&approval_id);
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.exec_approval_call_ids_by_turn_id,
                    &ev.turn_id,
                    &approval_id,
                );
            }
            EventMsg::ApplyPatchApprovalRequest(ev) => {
                self.patch_approval_call_ids.remove(&ev.call_id);
                Self::remove_call_id_from_turn_map_entry(
                    &mut self.patch_approval_call_ids_by_turn_id,
                    &ev.turn_id,
                    &ev.call_id,
                );
            }
            EventMsg::ElicitationRequest(ev) => {
                self.elicitation_requests
                    .remove(&ElicitationRequestKey::new(
                        ev.server_name.clone(),
                        ev.id.clone(),
                    ));
            }
            EventMsg::RequestUserInput(ev) => {
                self.request_user_input_call_ids.remove(&ev.call_id);
                let mut remove_turn_entry = false;
                if let Some(call_ids) = self
                    .request_user_input_call_ids_by_turn_id
                    .get_mut(&ev.turn_id)
                {
                    call_ids.retain(|call_id| call_id != &ev.call_id);
                    if call_ids.is_empty() {
                        remove_turn_entry = true;
                    }
                }
                if remove_turn_entry {
                    self.request_user_input_call_ids_by_turn_id
                        .remove(&ev.turn_id);
                }
            }
            _ => {}
        }
    }

    pub(super) fn should_replay_snapshot_event(&self, event: &Event) -> bool {
        match &event.msg {
            EventMsg::ExecApprovalRequest(ev) => self
                .exec_approval_call_ids
                .contains(&ev.effective_approval_id()),
            EventMsg::ApplyPatchApprovalRequest(ev) => {
                self.patch_approval_call_ids.contains(&ev.call_id)
            }
            EventMsg::ElicitationRequest(ev) => {
                self.elicitation_requests
                    .contains(&ElicitationRequestKey::new(
                        ev.server_name.clone(),
                        ev.id.clone(),
                    ))
            }
            EventMsg::RequestUserInput(ev) => {
                self.request_user_input_call_ids.contains(&ev.call_id)
            }
            _ => true,
        }
    }

    pub(super) fn has_pending_thread_approvals(&self) -> bool {
        !self.exec_approval_call_ids.is_empty()
            || !self.patch_approval_call_ids.is_empty()
            || !self.elicitation_requests.is_empty()
    }

    fn clear_request_user_input_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.request_user_input_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.request_user_input_call_ids.remove(&call_id);
            }
        }
    }

    fn clear_exec_approval_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.exec_approval_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.exec_approval_call_ids.remove(&call_id);
            }
        }
    }

    fn clear_patch_approval_turn(&mut self, turn_id: &str) {
        if let Some(call_ids) = self.patch_approval_call_ids_by_turn_id.remove(turn_id) {
            for call_id in call_ids {
                self.patch_approval_call_ids.remove(&call_id);
            }
        }
    }

    fn remove_call_id_from_turn_map(
        call_ids_by_turn_id: &mut HashMap<String, Vec<String>>,
        call_id: &str,
    ) {
        call_ids_by_turn_id.retain(|_, call_ids| {
            call_ids.retain(|queued_call_id| queued_call_id != call_id);
            !call_ids.is_empty()
        });
    }

    fn remove_call_id_from_turn_map_entry(
        call_ids_by_turn_id: &mut HashMap<String, Vec<String>>,
        turn_id: &str,
        call_id: &str,
    ) {
        let mut remove_turn_entry = false;
        if let Some(call_ids) = call_ids_by_turn_id.get_mut(turn_id) {
            call_ids.retain(|queued_call_id| queued_call_id != call_id);
            if call_ids.is_empty() {
                remove_turn_entry = true;
            }
        }
        if remove_turn_entry {
            call_ids_by_turn_id.remove(turn_id);
        }
    }

    fn clear(&mut self) {
        self.exec_approval_call_ids.clear();
        self.exec_approval_call_ids_by_turn_id.clear();
        self.patch_approval_call_ids.clear();
        self.patch_approval_call_ids_by_turn_id.clear();
        self.elicitation_requests.clear();
        self.request_user_input_call_ids.clear();
        self.request_user_input_call_ids_by_turn_id.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::super::ThreadEventStore;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::EventMsg;
    use codex_protocol::protocol::Op;
    use codex_protocol::protocol::TurnAbortReason;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use std::path::PathBuf;

    #[test]
    fn thread_event_snapshot_keeps_pending_request_user_input() {
        let mut store = ThreadEventStore::new(8);
        let request = Event {
            id: "ev-1".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        };

        store.push_event(request);

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first().map(|event| &event.msg),
            Some(EventMsg::RequestUserInput(_))
        ));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_request_user_input_after_user_answer() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: codex_protocol::request_user_input::RequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved request_user_input prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_exec_approval_after_outbound_approval_id() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ExecApprovalRequest(
                codex_protocol::protocol::ExecApprovalRequestEvent {
                    call_id: "call-1".to_string(),
                    approval_id: Some("approval-1".to_string()),
                    turn_id: "turn-1".to_string(),
                    command: vec!["echo".to_string(), "hi".to_string()],
                    cwd: PathBuf::from("/tmp"),
                    reason: None,
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
                    available_decisions: None,
                    parsed_cmd: Vec::new(),
                },
            ),
        });

        store.note_outbound_op(&Op::ExecApproval {
            id: "approval-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: codex_protocol::protocol::ReviewDecision::Approved,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved exec approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_answered_request_user_input_for_multi_prompt_turn() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: codex_protocol::request_user_input::RequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        store.push_event(Event {
            id: "ev-2".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-2".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first().map(|event| &event.msg),
            Some(EventMsg::RequestUserInput(ev)) if ev.call_id == "call-2"
        ));
    }

    #[test]
    fn thread_event_snapshot_keeps_newer_request_user_input_pending_when_same_turn_has_queue() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });
        store.push_event(Event {
            id: "ev-2".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-2".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });

        store.note_outbound_op(&Op::UserInputAnswer {
            id: "turn-1".to_string(),
            response: codex_protocol::request_user_input::RequestUserInputResponse {
                answers: HashMap::new(),
            },
        });

        let snapshot = store.snapshot();
        assert_eq!(snapshot.events.len(), 1);
        assert!(matches!(
            snapshot.events.first().map(|event| &event.msg),
            Some(EventMsg::RequestUserInput(ev)) if ev.call_id == "call-2"
        ));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_patch_approval_after_outbound_approval() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ApplyPatchApprovalRequest(
                codex_protocol::protocol::ApplyPatchApprovalRequestEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    changes: HashMap::new(),
                    reason: None,
                    grant_root: None,
                },
            ),
        });

        store.note_outbound_op(&Op::PatchApproval {
            id: "call-1".to_string(),
            decision: codex_protocol::protocol::ReviewDecision::Approved,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved patch approval prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_snapshot_drops_pending_approvals_when_turn_aborts() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ExecApprovalRequest(
                codex_protocol::protocol::ExecApprovalRequestEvent {
                    call_id: "exec-call-1".to_string(),
                    approval_id: Some("approval-1".to_string()),
                    turn_id: "turn-1".to_string(),
                    command: vec!["echo".to_string(), "hi".to_string()],
                    cwd: PathBuf::from("/tmp"),
                    reason: None,
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
                    available_decisions: None,
                    parsed_cmd: Vec::new(),
                },
            ),
        });
        store.push_event(Event {
            id: "ev-2".to_string(),
            msg: EventMsg::ApplyPatchApprovalRequest(
                codex_protocol::protocol::ApplyPatchApprovalRequestEvent {
                    call_id: "patch-call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    changes: HashMap::new(),
                    reason: None,
                    grant_root: None,
                },
            ),
        });
        store.push_event(Event {
            id: "ev-3".to_string(),
            msg: EventMsg::TurnAborted(codex_protocol::protocol::TurnAbortedEvent {
                turn_id: Some("turn-1".to_string()),
                reason: TurnAbortReason::Replaced,
            }),
        });

        let snapshot = store.snapshot();
        assert!(snapshot.events.iter().all(|event| {
            !matches!(
                &event.msg,
                EventMsg::ExecApprovalRequest(_) | EventMsg::ApplyPatchApprovalRequest(_)
            )
        }));
    }

    #[test]
    fn thread_event_snapshot_drops_resolved_elicitation_after_outbound_resolution() {
        let mut store = ThreadEventStore::new(8);
        let request_id = codex_protocol::mcp::RequestId::String("request-1".to_string());
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ElicitationRequest(codex_protocol::approvals::ElicitationRequestEvent {
                server_name: "server-1".to_string(),
                id: request_id.clone(),
                message: "Please confirm".to_string(),
            }),
        });

        store.note_outbound_op(&Op::ResolveElicitation {
            server_name: "server-1".to_string(),
            request_id,
            decision: codex_protocol::approvals::ElicitationAction::Accept,
        });

        let snapshot = store.snapshot();
        assert!(
            snapshot.events.is_empty(),
            "resolved elicitation prompt should not replay on thread switch"
        );
    }

    #[test]
    fn thread_event_store_reports_pending_thread_approvals() {
        let mut store = ThreadEventStore::new(8);
        assert_eq!(store.has_pending_thread_approvals(), false);

        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::ExecApprovalRequest(
                codex_protocol::protocol::ExecApprovalRequestEvent {
                    call_id: "call-1".to_string(),
                    approval_id: None,
                    turn_id: "turn-1".to_string(),
                    command: vec!["echo".to_string(), "hi".to_string()],
                    cwd: PathBuf::from("/tmp"),
                    reason: None,
                    network_approval_context: None,
                    proposed_execpolicy_amendment: None,
                    proposed_network_policy_amendments: None,
                    additional_permissions: None,
                    available_decisions: None,
                    parsed_cmd: Vec::new(),
                },
            ),
        });

        assert_eq!(store.has_pending_thread_approvals(), true);

        store.note_outbound_op(&Op::ExecApproval {
            id: "call-1".to_string(),
            turn_id: Some("turn-1".to_string()),
            decision: codex_protocol::protocol::ReviewDecision::Approved,
        });

        assert_eq!(store.has_pending_thread_approvals(), false);
    }

    #[test]
    fn request_user_input_does_not_count_as_pending_thread_approval() {
        let mut store = ThreadEventStore::new(8);
        store.push_event(Event {
            id: "ev-1".to_string(),
            msg: EventMsg::RequestUserInput(
                codex_protocol::request_user_input::RequestUserInputEvent {
                    call_id: "call-1".to_string(),
                    turn_id: "turn-1".to_string(),
                    questions: Vec::new(),
                },
            ),
        });

        assert_eq!(store.has_pending_thread_approvals(), false);
    }
}
