//! Helpers for deciding which buffered events to replay when switching threads.

use codex_app_server_protocol::CodexErrorInfo;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ServerRequest;
use codex_app_server_protocol::ThreadItem;
use std::collections::HashSet;

use super::ThreadBufferedEvent;
use super::ThreadEventSnapshot;

pub(super) fn snapshot_has_pending_interactive_request(snapshot: &ThreadEventSnapshot) -> bool {
    snapshot.events.iter().any(|event| {
        matches!(
            event,
            ThreadBufferedEvent::Request(request)
                if matches!(
                    request.as_ref(),
                    ServerRequest::CommandExecutionRequestApproval { .. }
                        | ServerRequest::FileChangeRequestApproval { .. }
                        | ServerRequest::McpServerElicitationRequest { .. }
                        | ServerRequest::PermissionsRequestApproval { .. }
                        | ServerRequest::ToolRequestUserInput { .. }
                )
        )
    })
}

/// A later turn resolves an earlier precaution; replay must preserve restored input.
pub(super) fn omit_resolved_misalignment_errors(
    snapshot: &mut ThreadEventSnapshot,
    latest_turn: &str,
) {
    snapshot.events.retain(|event| !matches!(
        event,
        ThreadBufferedEvent::Notification(notification)
            if matches!(notification.as_ref(), ServerNotification::Error(n)
                if n.turn_id != latest_turn
                    && n.error.codex_error_info == Some(CodexErrorInfo::MisalignmentPolicyViolation))
    ));
    let completed_turns = snapshot.events.iter_mut().filter_map(|event| match event {
        ThreadBufferedEvent::Notification(notification) => match notification.as_mut() {
            ServerNotification::TurnCompleted(n) => Some(&mut n.turn),
            _ => None,
        },
        _ => None,
    });
    for turn in snapshot.turns.iter_mut().chain(completed_turns) {
        if turn.id != latest_turn
            && turn.error.as_ref().is_some_and(|error| {
                error.codex_error_info == Some(CodexErrorInfo::MisalignmentPolicyViolation)
            })
        {
            turn.error = None;
        }
    }
}

/// A completed item's full text replaces its earlier streaming deltas during replay.
/// Keep deltas without a later completion so an in-progress or truncated stream still renders.
/// Other events are barriers: streaming text may need to flush before a tool or prompt.
/// This only changes the replay snapshot; the live notification store remains untouched.
pub(super) fn omit_completed_agent_deltas(events: &mut Vec<ThreadBufferedEvent>) {
    let mut completed = HashSet::new();
    events.reverse();
    events.retain(|event| {
        if let ThreadBufferedEvent::Notification(notification) = event {
            match notification.as_ref() {
                ServerNotification::ItemCompleted(notification) => {
                    if let ThreadItem::AgentMessage { id, .. } = &notification.item {
                        completed.insert((
                            notification.thread_id.clone(),
                            notification.turn_id.clone(),
                            id.clone(),
                        ));
                    } else {
                        completed.clear();
                    }
                }
                ServerNotification::AgentMessageDelta(notification) => {
                    return !completed.contains(&(
                        notification.thread_id.clone(),
                        notification.turn_id.clone(),
                        notification.item_id.clone(),
                    ));
                }
                _ => completed.clear(),
            }
        } else {
            completed.clear();
        }
        true
    });
    events.reverse();
}
