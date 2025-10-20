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

    pub async fn completed(&self, item: TurnItem, emit_raw_agent_reasoning: bool) {
        let err = self
            .tx_event
            .send(Event {
                id: self.turn_id.clone(),
                msg: EventMsg::ItemCompleted(ItemCompletedEvent {
                    thread_id: self.thread_id,
                    turn_id: self.turn_id.clone(),
                    item: item.clone(),
                }),
            })
            .await;
        if let Err(e) = err {
            error!("failed to send item completed event: {e}");
        }

        self.trigger_legacy_events(item, emit_raw_agent_reasoning)
            .await;
    }

    pub async fn started_completed(&self, item: TurnItem, emit_raw_agent_reasoning: bool) {
        self.started(item.clone()).await;
        self.completed(item, emit_raw_agent_reasoning).await;
    }

    async fn trigger_legacy_events(&self, item: TurnItem, emit_raw_agent_reasoning: bool) {
        for event in item.legacy_events(emit_raw_agent_reasoning) {
            if let Err(e) = self
                .tx_event
                .send(Event {
                    id: self.turn_id.clone(),
                    msg: event.clone(),
                })
                .await
            {
                error!("failed to send legacy event: {e}");
            }
        }
    }
}
