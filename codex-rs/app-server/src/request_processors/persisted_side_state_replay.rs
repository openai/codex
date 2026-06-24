//! Reconstructs typed lifecycle notifications that are durable outside turn items.
//!
//! Resume callers own response ordering. This module keeps rollout extraction pure and sends the
//! reconstructed notifications only to the connection whose state is being restored.

use std::sync::Arc;

use codex_app_server_protocol::HookCompletedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadGoalUpdatedNotification;
use codex_app_server_protocol::guardian_auto_approval_review_notification;
use codex_protocol::ThreadId;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::ThreadGoalStatus;
use codex_rollout::is_persisted_rollout_item;

use crate::outgoing_message::ConnectionId;
use crate::outgoing_message::OutgoingMessageSender;

/// Extracts ordered typed notifications for persisted state that is not represented by turn items.
pub(super) fn persisted_side_state_notifications(
    thread_id: ThreadId,
    rollout_items: &[RolloutItem],
    include_goal_updates: bool,
) -> Vec<ServerNotification> {
    let mut latest_turn_id = "";
    let mut notifications = Vec::new();

    for rollout_item in rollout_items {
        if !is_persisted_rollout_item(rollout_item) {
            continue;
        }
        let RolloutItem::EventMsg(event) = rollout_item else {
            continue;
        };
        if let EventMsg::TurnStarted(started) = event {
            latest_turn_id = started.turn_id.as_str();
            continue;
        }
        if let EventMsg::GuardianAssessment(assessment) = event {
            notifications.push(guardian_auto_approval_review_notification(
                &thread_id,
                latest_turn_id,
                assessment,
            ));
            continue;
        }
        if let EventMsg::HookCompleted(completed) = event {
            notifications.push(ServerNotification::HookCompleted(
                HookCompletedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: completed.turn_id.clone(),
                    run: completed.run.clone().into(),
                },
            ));
            continue;
        }
        if include_goal_updates
            && let EventMsg::ThreadGoalUpdated(updated) = event
            && updated.goal.status == ThreadGoalStatus::Complete
            && let Some(turn_id) = updated.turn_id.as_ref()
        {
            notifications.push(ServerNotification::ThreadGoalUpdated(
                ThreadGoalUpdatedNotification {
                    thread_id: thread_id.to_string(),
                    turn_id: Some(turn_id.clone()),
                    goal: updated.goal.clone().into(),
                },
            ));
        }
    }

    notifications
}

/// Sends reconstructed persisted state only to the connection completing resume.
pub(super) async fn send_persisted_side_state_to_connection(
    outgoing: &Arc<OutgoingMessageSender>,
    connection_id: ConnectionId,
    thread_id: ThreadId,
    rollout_items: &[RolloutItem],
    include_goal_updates: bool,
) {
    for notification in
        persisted_side_state_notifications(thread_id, rollout_items, include_goal_updates)
    {
        outgoing
            .send_server_notification_to_connections(&[connection_id], notification)
            .await;
    }
}
