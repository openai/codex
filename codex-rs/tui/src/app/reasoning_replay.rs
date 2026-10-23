//! Restores the active reasoning stream without treating its snapshot as a completed item.
//!
//! Buffered turn events must replay before the active item is restored. Its first retained
//! event, or the end of replay when all its events were evicted, establishes that boundary.

use super::ThreadBufferedEvent;
use super::ThreadEventSnapshot;
use crate::chatwidget::ChatWidget;
use codex_app_server_protocol::ItemStartedNotification;
use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItemPayload;
use codex_app_server_protocol::TurnStatus;

pub(super) struct ReasoningReplay {
    started: Option<ItemStartedNotification>,
    parts: Option<(Vec<String>, Vec<String>)>,
}

impl ReasoningReplay {
    pub(super) fn new(snapshot: &mut ThreadEventSnapshot) -> Self {
        let started = snapshot.active_reasoning_item.take();
        let parts = started.as_ref().and_then(|started| {
            let turn = snapshot
                .turns
                .iter_mut()
                .find(|turn| turn.id == started.turn_id && turn.status == TurnStatus::InProgress)?;
            let index = turn.items.iter().position(
                |item| matches!(&item.payload, ThreadItemPayload::Reasoning { id, .. } if id == started.item.id()),
            )?;
            match turn.items.remove(index).payload {
                ThreadItemPayload::Reasoning {
                    summary, content, ..
                } => Some((summary, content)),
                _ => unreachable!("matched a reasoning item"),
            }
        });
        Self { started, parts }
    }

    pub(super) fn before_event(&mut self, event: &ThreadBufferedEvent, chat: &mut ChatWidget) {
        if self.started.as_ref().is_some_and(|started| {
            matches!(event, ThreadBufferedEvent::Notification(notification) if match notification.as_ref() {
                ServerNotification::ItemStarted(item) => {
                    item.turn_id == started.turn_id && item.item.id() == started.item.id()
                }
                ServerNotification::ReasoningSummaryTextDelta(delta) => {
                    delta.turn_id == started.turn_id && delta.item_id == started.item.id()
                }
                ServerNotification::ReasoningTextDelta(delta) => {
                    delta.turn_id == started.turn_id && delta.item_id == started.item.id()
                }
                ServerNotification::ReasoningSummaryPartAdded(part) => {
                    part.turn_id == started.turn_id && part.item_id == started.item.id()
                }
                _ => false,
            })
        }) {
            self.restore(chat);
        }
    }

    pub(super) fn restore(&mut self, chat: &mut ChatWidget) {
        if let Some(started) = self.started.take() {
            chat.restore_active_reasoning_item(started, self.parts.take());
        }
    }
}
