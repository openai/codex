use async_channel::Sender;
use codex_protocol::ConversationId;
use codex_protocol::items::TurnItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ItemStartedEvent;
use tracing::error;

#[derive(Debug)]
pub(crate) struct ItemCollector {
    thread_id: ConversationId,
    turn_id: String,
    tx_event: Sender<Event>,
}

impl ItemCollector {
    pub fn new(
        tx_event: Sender<Event>,
        thread_id: ConversationId,
        turn_id: String,
    ) -> ItemCollector {
        ItemCollector {
            tx_event,
            thread_id,
            turn_id,
        }
    }

    pub async fn started(&self, item: TurnItem) {
        let err = self
            .tx_event
            .send(Event {
                id: self.turn_id.clone(),
                msg: EventMsg::ItemStarted(ItemStartedEvent {
                    thread_id: self.thread_id,
                    turn_id: self.turn_id.clone(),
                    item,
                }),
            })
            .await;
        if let Err(e) = err {
            error!("failed to send item started event: {e}");
        }
    }

    pub async fn completed(&self, item: TurnItem) {
        let err = self
            .tx_event
            .send(Event {
                id: self.turn_id.clone(),
                msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                    thread_id: self.thread_id,
                    turn_id: self.turn_id.clone(),
                    item,
                }),
            })
            .await;
        if let Err(e) = err {
            error!("failed to send item completed event: {e}");
        }
    }

    pub async fn started_completed(&self, item: TurnItem) {
        self.started(item.clone()).await;
        self.completed(item).await;
    }
}
