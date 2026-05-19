use crate::state::ActiveTurn;
use crate::state::MailboxDeliveryPhase;
use crate::state::TurnState;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InterAgentCommunication;
use codex_protocol::user_input::UserInput;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TurnInput {
    UserInput(Vec<UserInput>),
    ResponseInputItem(ResponseInputItem),
}

/// Session-scoped pending input storage and active-turn mailbox delivery coordination.
pub(crate) struct InputQueue {
    mailbox_tx: watch::Sender<()>,
    state: Mutex<InputQueueState>,
}

#[derive(Default)]
struct InputQueueState {
    mailbox_pending_mails: VecDeque<InterAgentCommunication>,
    turn_pending_input: Vec<TurnInput>,
}

impl InputQueue {
    pub(crate) fn new() -> Self {
        let (mailbox_tx, _) = watch::channel(());
        Self {
            mailbox_tx,
            state: Mutex::new(InputQueueState::default()),
        }
    }

    pub(crate) async fn subscribe_mailbox(&self) -> watch::Receiver<()> {
        let mut mailbox_rx = self.mailbox_tx.subscribe();
        if self.has_pending_mailbox_items().await {
            mailbox_rx.mark_changed();
        }
        mailbox_rx
    }

    pub(crate) async fn enqueue_mailbox_communication(
        &self,
        communication: InterAgentCommunication,
    ) {
        self.state
            .lock()
            .await
            .mailbox_pending_mails
            .push_back(communication);
        self.mailbox_tx.send_replace(());
    }

    pub(crate) async fn has_pending_mailbox_items(&self) -> bool {
        !self.state.lock().await.mailbox_pending_mails.is_empty()
    }

    pub(crate) async fn has_trigger_turn_mailbox_items(&self) -> bool {
        self.state
            .lock()
            .await
            .mailbox_pending_mails
            .iter()
            .any(|mail| mail.trigger_turn)
    }

    pub(crate) async fn has_queued_input_for_next_turn(&self) -> bool {
        !self.state.lock().await.turn_pending_input.is_empty()
    }

    pub(crate) async fn turn_state_for_sub_id(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
        sub_id: &str,
    ) -> Option<Arc<Mutex<TurnState>>> {
        let active = active_turn.lock().await;
        active.as_ref().and_then(|active_turn| {
            active_turn
                .tasks
                .contains_key(sub_id)
                .then(|| Arc::clone(&active_turn.turn_state))
        })
    }

    pub(crate) async fn defer_mailbox_delivery_to_next_turn(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
        sub_id: &str,
    ) {
        let turn_state = self.turn_state_for_sub_id(active_turn, sub_id).await;
        let Some(turn_state) = turn_state else {
            return;
        };
        if !self.state.lock().await.turn_pending_input.is_empty() {
            return;
        }
        let mut turn_state = turn_state.lock().await;
        turn_state.set_mailbox_delivery_phase(MailboxDeliveryPhase::NextTurn);
    }

    pub(crate) async fn accept_mailbox_delivery_for_current_turn(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
        sub_id: &str,
    ) {
        let turn_state = self.turn_state_for_sub_id(active_turn, sub_id).await;
        let Some(turn_state) = turn_state else {
            return;
        };
        turn_state
            .lock()
            .await
            .accept_mailbox_delivery_for_current_turn();
    }

    pub(crate) async fn inject(&self, input: Vec<TurnInput>) {
        self.state.lock().await.turn_pending_input.extend(input);
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn get_pending_input(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
    ) -> Vec<TurnInput> {
        let accepts_mailbox_delivery = {
            let active = active_turn.lock().await;
            match active.as_ref() {
                Some(active_turn) => {
                    let turn_state = active_turn.turn_state.lock().await;
                    turn_state.accepts_mailbox_delivery_for_current_turn()
                }
                None => true,
            }
        };
        let mut state = self.state.lock().await;
        let pending_input = std::mem::take(&mut state.turn_pending_input);
        if !accepts_mailbox_delivery {
            return pending_input;
        }
        let mailbox_items = state
            .mailbox_pending_mails
            .drain(..)
            .map(|mail| TurnInput::ResponseInputItem(mail.to_response_input_item()));
        if pending_input.is_empty() {
            mailbox_items.collect()
        } else {
            let mut pending_input = pending_input;
            pending_input.extend(mailbox_items);
            pending_input
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state reads must remain atomic"
    )]
    pub(crate) async fn has_pending_input(&self, active_turn: &Mutex<Option<ActiveTurn>>) -> bool {
        let accepts_mailbox_delivery = {
            let active = active_turn.lock().await;
            match active.as_ref() {
                Some(active_turn) => {
                    let turn_state = active_turn.turn_state.lock().await;
                    turn_state.accepts_mailbox_delivery_for_current_turn()
                }
                None => true,
            }
        };
        if !self.state.lock().await.turn_pending_input.is_empty() {
            return true;
        }
        if !accepts_mailbox_delivery {
            return false;
        }
        self.has_pending_mailbox_items().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_protocol::AgentPath;
    use pretty_assertions::assert_eq;

    fn make_mail(
        author: AgentPath,
        recipient: AgentPath,
        content: &str,
        trigger_turn: bool,
    ) -> InterAgentCommunication {
        InterAgentCommunication::new(
            author,
            recipient,
            Vec::new(),
            content.to_string(),
            trigger_turn,
        )
    }

    #[tokio::test]
    async fn input_queue_notifies_mailbox_subscribers() {
        let input_queue = InputQueue::new();
        let mut mailbox_rx = input_queue.subscribe_mailbox().await;

        input_queue
            .enqueue_mailbox_communication(make_mail(
                AgentPath::root(),
                AgentPath::try_from("/root/worker").expect("agent path"),
                "one",
                /*trigger_turn*/ false,
            ))
            .await;
        input_queue
            .enqueue_mailbox_communication(make_mail(
                AgentPath::root(),
                AgentPath::try_from("/root/worker").expect("agent path"),
                "two",
                /*trigger_turn*/ false,
            ))
            .await;

        mailbox_rx.changed().await.expect("mailbox update");
    }

    #[tokio::test]
    async fn input_queue_drains_mailbox_in_delivery_order() {
        let input_queue = InputQueue::new();
        let mail_one = make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "one",
            /*trigger_turn*/ false,
        );
        let mail_two = make_mail(
            AgentPath::try_from("/root/worker").expect("agent path"),
            AgentPath::root(),
            "two",
            /*trigger_turn*/ false,
        );

        input_queue
            .enqueue_mailbox_communication(mail_one.clone())
            .await;
        input_queue
            .enqueue_mailbox_communication(mail_two.clone())
            .await;

        assert_eq!(
            input_queue.get_pending_input(&Mutex::new(None)).await,
            vec![
                TurnInput::ResponseInputItem(mail_one.to_response_input_item()),
                TurnInput::ResponseInputItem(mail_two.to_response_input_item())
            ]
        );
        assert!(!input_queue.has_pending_mailbox_items().await);
    }

    #[tokio::test]
    async fn input_queue_tracks_pending_trigger_turn_mail() {
        let input_queue = InputQueue::new();

        input_queue
            .enqueue_mailbox_communication(make_mail(
                AgentPath::root(),
                AgentPath::try_from("/root/worker").expect("agent path"),
                "queued",
                /*trigger_turn*/ false,
            ))
            .await;
        assert!(!input_queue.has_trigger_turn_mailbox_items().await);

        input_queue
            .enqueue_mailbox_communication(make_mail(
                AgentPath::root(),
                AgentPath::try_from("/root/worker").expect("agent path"),
                "wake",
                /*trigger_turn*/ true,
            ))
            .await;
        assert!(input_queue.has_trigger_turn_mailbox_items().await);
    }
}
