//! Classifies pre-cut thread events against a running-thread resume snapshot.

use super::*;
use std::collections::HashMap;

pub(super) struct BufferedThreadEvent {
    pub(super) event: Event,
    pub(super) represented_in_resume_snapshot: bool,
    pub(super) request_live_for_resumed_connection: bool,
}

impl BufferedThreadEvent {
    pub(super) fn from_resume_cut(event: Event) -> Self {
        let represented_in_resume_snapshot =
            is_persisted_rollout_item(&RolloutItem::EventMsg(event.msg.clone()));
        Self {
            event,
            represented_in_resume_snapshot,
            request_live_for_resumed_connection: true,
        }
    }
}

fn buffered_event_creates_pending_server_request(event: &EventMsg) -> bool {
    matches!(
        event,
        EventMsg::ExecApprovalRequest(_)
            | EventMsg::RequestPermissions(_)
            | EventMsg::RequestUserInput(_)
            | EventMsg::DynamicToolCallRequest(_)
            | EventMsg::ElicitationRequest(_)
            | EventMsg::ApplyPatchApprovalRequest(_)
    )
}

fn buffered_event_cancels_pending_server_requests(event: &EventMsg) -> bool {
    matches!(
        event,
        EventMsg::TurnStarted(_) | EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_)
    )
}

/// Marks buffered request events that remain live at the resume snapshot boundary.
///
/// Existing subscribers still receive the full event sequence. The resumed connection starts
/// from the final snapshot, so it must not receive a request that a later buffered turn transition
/// has already invalidated. The return value applies the same projection to requests that were
/// pending before the cut.
pub(super) fn project_buffered_request_liveness(
    buffered_events: &mut [BufferedThreadEvent],
) -> bool {
    let mut later_event_cancels_requests = false;
    for buffered in buffered_events.iter_mut().rev() {
        if buffered_event_creates_pending_server_request(&buffered.event.msg) {
            buffered.request_live_for_resumed_connection = !later_event_cancels_requests;
        }
        if buffered_event_cancels_pending_server_requests(&buffered.event.msg) {
            later_event_cancels_requests = true;
        }
    }
    !later_event_cancels_requests
}

fn turn_in_resume_payload<'a>(
    turn_id: &str,
    thread_turns: &'a [Turn],
    initial_turns_page: Option<&'a codex_app_server_protocol::TurnsPage>,
) -> Option<&'a Turn> {
    let mut turns = thread_turns.iter().chain(
        initial_turns_page
            .into_iter()
            .flat_map(|page| page.data.iter()),
    );
    // The same turn can appear in both response fields. Prefer a full item view so an
    // item-producing event is suppressed only when one of those fields actually carries it.
    turns
        .clone()
        .find(|turn| turn.id == turn_id && matches!(turn.items_view, TurnItemsView::Full))
        .or_else(|| turns.find(|turn| turn.id == turn_id))
}

fn serialized_thread_items(
    left: &ThreadItem,
    right: &ThreadItem,
) -> Option<(serde_json::Value, serde_json::Value)> {
    let left = serde_json::to_value(left).ok()?;
    let right = serde_json::to_value(right).ok()?;
    (left.get("type") == right.get("type")).then_some((left, right))
}

fn thread_items_share_stable_id(left: &ThreadItem, right: &ThreadItem) -> bool {
    let Some((left, right)) = serialized_thread_items(left, right) else {
        return false;
    };
    left.get("id").is_some() && left.get("id") == right.get("id")
}

fn thread_items_match_without_generated_id(left: &ThreadItem, right: &ThreadItem) -> bool {
    let Some((mut left, mut right)) = serialized_thread_items(left, right) else {
        return false;
    };
    // Legacy history reconstruction assigns generated ids to user/agent items while the
    // canonical lifecycle event retains the model item id. Compare their remaining content so a
    // Full resume payload does not get followed by a duplicate item notification.
    let Some(left) = left.as_object_mut() else {
        return false;
    };
    let Some(right) = right.as_object_mut() else {
        return false;
    };
    left.remove("id");
    right.remove("id");
    left == right
}

struct ResumePayloadItemCoverageEntry {
    turn_id: String,
    item: ThreadItem,
    claimed_canonical_id: Option<String>,
}

pub(super) struct ResumePayloadItemCoverage {
    entries: Vec<ResumePayloadItemCoverageEntry>,
}

impl ResumePayloadItemCoverage {
    pub(super) fn new<'a>(
        thread_turns: &'a [Turn],
        initial_turns_page: Option<&'a codex_app_server_protocol::TurnsPage>,
    ) -> Self {
        let mut entries = Vec::new();
        let mut selected_turns: Vec<&Turn> = Vec::new();
        let mut selected_turn_indexes: HashMap<&str, usize> = HashMap::new();
        for turn in thread_turns.iter().chain(
            initial_turns_page
                .into_iter()
                .flat_map(|page| page.data.iter()),
        ) {
            if let Some(index) = selected_turn_indexes.get(turn.id.as_str()).copied() {
                if !matches!(selected_turns[index].items_view, TurnItemsView::Full)
                    && matches!(turn.items_view, TurnItemsView::Full)
                {
                    selected_turns[index] = turn;
                }
            } else {
                selected_turn_indexes.insert(turn.id.as_str(), selected_turns.len());
                selected_turns.push(turn);
            }
        }
        for turn in selected_turns {
            entries.extend(
                turn.items
                    .iter()
                    .cloned()
                    .map(|item| ResumePayloadItemCoverageEntry {
                        turn_id: turn.id.clone(),
                        item,
                        claimed_canonical_id: None,
                    }),
            );
        }
        Self { entries }
    }

    fn consume_canonical_item(
        &mut self,
        turn_id: &str,
        item: &ThreadItem,
        phase: CanonicalItemPhase,
    ) -> bool {
        if let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry.turn_id == turn_id && thread_items_share_stable_id(&entry.item, item)
        }) {
            // A final snapshot dominates ItemStarted. ItemCompleted additionally requires the
            // returned item to carry the completed state/content; an in-progress same-id item is
            // not enough when the durable legacy end fell after the cut.
            let represented = matches!(phase, CanonicalItemPhase::Started)
                || thread_items_match_without_generated_id(&entry.item, item);
            if represented && matches!(phase, CanonicalItemPhase::Completed) {
                let canonical_id = item.id();
                if entry
                    .claimed_canonical_id
                    .as_deref()
                    .is_some_and(|claimed| claimed != canonical_id)
                {
                    return false;
                }
                entry.claimed_canonical_id = Some(canonical_id.to_string());
            }
            return represented;
        }

        if !item_can_have_generated_history_id(item) {
            return false;
        }
        let canonical_id = item.id();
        if let Some(entry) = self.entries.iter().find(|entry| {
            entry.turn_id == turn_id && entry.claimed_canonical_id.as_deref() == Some(canonical_id)
        }) {
            // The completion is processed first (classification runs in reverse), binding the
            // generated history occurrence. Its earlier ItemStarted may reuse that binding even
            // when the start payload was partial. A completion still requires exact final state.
            return matches!(phase, CanonicalItemPhase::Started)
                || thread_items_match_without_generated_id(&entry.item, item);
        }
        // A different canonical id consumes a different generated-id occurrence.
        let Some(entry) = self.entries.iter_mut().find(|entry| {
            entry.turn_id == turn_id
                && entry.claimed_canonical_id.is_none()
                && thread_items_match_without_generated_id(&entry.item, item)
        }) else {
            return false;
        };
        entry.claimed_canonical_id = Some(canonical_id.to_string());
        true
    }

    fn covers_item_delta(&self, turn_id: &str, canonical_item_id: &str) -> bool {
        self.entries.iter().any(|entry| {
            entry.turn_id == turn_id
                && entry.claimed_canonical_id.as_deref() == Some(canonical_item_id)
        })
    }
}

#[derive(Clone, Copy)]
enum CanonicalItemPhase {
    Started,
    Completed,
}

fn item_can_have_generated_history_id(item: &ThreadItem) -> bool {
    matches!(
        item,
        ThreadItem::UserMessage { .. }
            | ThreadItem::HookPrompt { .. }
            | ThreadItem::AgentMessage { .. }
            | ThreadItem::Reasoning { .. }
            | ThreadItem::ContextCompaction { .. }
    )
}

fn projected_item_event(event: &Event) -> Option<(String, ThreadItem, CanonicalItemPhase)> {
    if let EventMsg::GuardianAssessment(payload) = &event.msg {
        let (status, phase) = match payload.status {
            codex_protocol::protocol::GuardianAssessmentStatus::InProgress => (
                codex_app_server_protocol::CommandExecutionStatus::InProgress,
                CanonicalItemPhase::Started,
            ),
            codex_protocol::protocol::GuardianAssessmentStatus::Denied
            | codex_protocol::protocol::GuardianAssessmentStatus::Aborted => (
                codex_app_server_protocol::CommandExecutionStatus::Declined,
                CanonicalItemPhase::Completed,
            ),
            codex_protocol::protocol::GuardianAssessmentStatus::TimedOut => (
                codex_app_server_protocol::CommandExecutionStatus::Failed,
                CanonicalItemPhase::Completed,
            ),
            codex_protocol::protocol::GuardianAssessmentStatus::Approved => return None,
        };
        let item = codex_app_server_protocol::build_item_from_guardian_event(payload, status)?;
        let turn_id = if payload.turn_id.is_empty() {
            event.id.clone()
        } else {
            payload.turn_id.clone()
        };
        return Some((turn_id, item, phase));
    }
    if let EventMsg::DynamicToolCallRequest(payload) = &event.msg {
        let turn_id = if payload.turn_id.is_empty() {
            event.id.clone()
        } else {
            payload.turn_id.clone()
        };
        let item = ThreadItem::DynamicToolCall {
            id: payload.call_id.clone(),
            namespace: payload.namespace.clone(),
            tool: payload.tool.clone(),
            arguments: payload.arguments.clone(),
            status: codex_app_server_protocol::DynamicToolCallStatus::InProgress,
            content_items: None,
            success: None,
            duration_ms: None,
        };
        return Some((turn_id, item, CanonicalItemPhase::Started));
    }
    if let Some(notification) = codex_app_server_protocol::try_item_event_to_server_notification(
        event.msg.clone(),
        "",
        &event.id,
    ) {
        return match notification {
            ServerNotification::ItemStarted(notification) => Some((
                notification.turn_id,
                notification.item,
                CanonicalItemPhase::Started,
            )),
            ServerNotification::ItemCompleted(notification) => Some((
                notification.turn_id,
                notification.item,
                CanonicalItemPhase::Completed,
            )),
            _ => None,
        };
    }

    // Patch lifecycle notifications are intentionally hidden from v2 clients, but the final
    // patch item still needs to claim its snapshot occurrence so a preceding patch delta is not
    // replayed after that final state.
    if let EventMsg::PatchApplyEnd(event_msg) = &event.msg {
        let turn_id = if event_msg.turn_id.is_empty() {
            event.id.clone()
        } else {
            event_msg.turn_id.clone()
        };
        return Some((
            turn_id,
            codex_app_server_protocol::build_file_change_end_item(event_msg),
            CanonicalItemPhase::Completed,
        ));
    }
    None
}

fn buffered_item_delta_target(event: &Event) -> Option<(&str, &str)> {
    match &event.msg {
        EventMsg::AgentMessageContentDelta(event) => Some((&event.turn_id, &event.item_id)),
        EventMsg::PlanDelta(event) => Some((&event.turn_id, &event.item_id)),
        EventMsg::ReasoningContentDelta(event) => Some((&event.turn_id, &event.item_id)),
        EventMsg::ReasoningRawContentDelta(event) => Some((&event.turn_id, &event.item_id)),
        EventMsg::AgentReasoningSectionBreak(payload) => Some((&event.id, &payload.item_id)),
        EventMsg::ExecCommandOutputDelta(payload) => Some((&event.id, &payload.call_id)),
        EventMsg::PatchApplyUpdated(payload) => Some((&event.id, &payload.call_id)),
        _ => None,
    }
}

fn item_payload_is_redacted_on_resume(item: &ThreadItem) -> bool {
    matches!(
        item,
        ThreadItem::McpToolCall { .. } | ThreadItem::ImageGeneration { .. }
    )
}

pub(super) fn buffered_event_is_represented_in_resume_payload(
    buffered: &BufferedThreadEvent,
    thread_turns: &[Turn],
    initial_turns_page: Option<&codex_app_server_protocol::TurnsPage>,
    item_coverage: &mut ResumePayloadItemCoverage,
    resume_payload_mode: ResumePayloadMode,
) -> bool {
    // A raw hook prompt has two independently routed projections: the typed HookPrompt can be
    // represented in the snapshot, while the experimental raw notification never is. This
    // classifier owns only the typed side; buffered dispatch handles the raw channel separately.
    if let EventMsg::RawResponseItem(event) = &buffered.event.msg {
        if let Some(item) = hook_prompt_thread_item(&event.item) {
            return item_coverage.consume_canonical_item(
                &buffered.event.id,
                &item,
                CanonicalItemPhase::Completed,
            );
        }
        // Other raw response items have no typed projection.
        return true;
    }
    if let Some((turn_id, item, phase)) = projected_item_event(&buffered.event) {
        // Remote resume redaction removes image items and scrubs MCP payloads. Never follow that
        // response with the original unredacted canonical event, even when pagination omitted it.
        if resume_payload_mode.is_redacted() && item_payload_is_redacted_on_resume(&item) {
            return true;
        }
        return item_coverage.consume_canonical_item(&turn_id, &item, phase);
    }
    if let Some((turn_id, item_id)) = buffered_item_delta_target(&buffered.event) {
        // Classification runs in reverse, so a represented completion has already bound the
        // canonical id to the final snapshot occurrence. Only then is it safe to drop deltas;
        // omitted/partial items keep their stream updates.
        return item_coverage.covers_item_delta(turn_id, item_id);
    }
    if !buffered.represented_in_resume_snapshot {
        return false;
    }

    // Usage and goal state are not stored in a Turn. Replaying buffered TokenCount preserves both
    // its usage and rate-limit notifications; the separate restored-usage replay is skipped below
    // when this event already supplied it. Goal updates likewise use their notification/snapshot
    // channel rather than turn coverage.
    if matches!(
        buffered.event.msg,
        EventMsg::TokenCount(_) | EventMsg::ThreadGoalUpdated(_)
    ) {
        return false;
    }

    let turn_id = match &buffered.event.msg {
        EventMsg::TurnStarted(event) => event.turn_id.as_str(),
        EventMsg::TurnComplete(event) => event.turn_id.as_str(),
        EventMsg::TurnAborted(event) => event
            .turn_id
            .as_deref()
            .unwrap_or(buffered.event.id.as_str()),
        _ => buffered.event.id.as_str(),
    };
    let Some(turn) = turn_in_resume_payload(turn_id, thread_turns, initial_turns_page) else {
        return false;
    };

    match &buffered.event.msg {
        // Turn metadata is present even in summary/not-loaded pages.
        EventMsg::TurnStarted(_) => true,
        EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
            !matches!(turn.status, TurnStatus::InProgress)
        }
        // Other persisted turn events materialize as items. A summary or not-loaded page may
        // omit that item, so only a full view proves the notification is represented.
        _ => matches!(turn.items_view, TurnItemsView::Full),
    }
}

pub(super) fn should_replay_reconciled_token_usage(
    buffered_events: &[BufferedThreadEvent],
    reconciled: Option<&codex_protocol::protocol::TokenUsageInfo>,
    reconciled_turn_id: Option<&str>,
) -> bool {
    let latest_buffered = buffered_events.iter().rev().find_map(|buffered| {
        let EventMsg::TokenCount(event) = &buffered.event.msg else {
            return None;
        };
        event
            .info
            .as_ref()
            .map(|info| (info, buffered.event.id.as_str()))
    });
    match (latest_buffered, reconciled) {
        (_, None) => false,
        (None, Some(_)) => true,
        (Some((buffered_info, buffered_turn_id)), Some(reconciled_info)) => {
            buffered_info != reconciled_info || Some(buffered_turn_id) != reconciled_turn_id
        }
    }
}
