use codex_protocol::ConversationId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use std::collections::HashMap;
use std::sync::Arc;

use crate::codex::Session;

pub(crate) struct ItemCollector {
    thread_id: ConversationId,
    turn_id: String,
    session: Arc<Session>,
    items: HashMap<String, TurnItem>,
}

impl ItemCollector {
    pub fn new(session: Arc<Session>, thread_id: ConversationId, turn_id: String) -> ItemCollector {
        ItemCollector {
            items: HashMap::new(),
            session,
            thread_id,
            turn_id,
        }
    }

    pub fn started(&mut self, item: TurnItem) {
        self.items.insert(item.id(), item.clone());

        self.session.send_event(Event {
            id: self.turn_id.clone(),
            msg: EventMsg::ItemStarted(ItemStartedEvent {
                thread_id: self.thread_id,
                turn_id: self.turn_id.clone(),
                item,
            }),
        });
    }

    pub fn completed(&mut self, item: TurnItem) {
        self.items.remove(&item.id());

        self.session.send_event(Event {
            id: self.turn_id.clone(),
            msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                thread_id: self.thread_id,
                turn_id: self.turn_id.clone(),
                item,
            }),
        });
    }
}
