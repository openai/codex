use crate::state::ActiveTurn;
use crate::state::MailboxDeliveryPhase;
use crate::state::TurnState;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::protocol::InterAgentCommunication;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::watch;

/// Turn-local pending input storage owned by the input queue flow.
#[derive(Default)]
pub(crate) struct TurnInputQueue {
    items: Vec<ResponseInputItem>,
}

/// Session-scoped pending input storage and active-turn mailbox delivery coordination.
pub(crate) struct InputQueue {
    mailbox_tx: watch::Sender<()>,
    mailbox_pending_mails: Mutex<VecDeque<InterAgentCommunication>>,

    idle_pending_input: Mutex<Vec<ResponseInputItem>>,
}

impl InputQueue {
    pub(crate) fn new() -> Self {
        let (mailbox_tx, _) = watch::channel(());
        Self {
            mailbox_tx,
            mailbox_pending_mails: Mutex::new(VecDeque::new()),
            idle_pending_input: Mutex::new(Vec::new()),
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
        self.mailbox_pending_mails
            .lock()
            .await
            .push_back(communication);
        self.mailbox_tx.send_replace(());
    }

    pub(crate) async fn has_pending_mailbox_items(&self) -> bool {
        !self.mailbox_pending_mails.lock().await.is_empty()
    }

    pub(crate) async fn has_trigger_turn_mailbox_items(&self) -> bool {
        self.mailbox_pending_mails
            .lock()
            .await
            .iter()
            .any(|mail| mail.trigger_turn)
    }

    pub(crate) async fn drain_mailbox_input_items(&self) -> Vec<ResponseInputItem> {
        self.mailbox_pending_mails
            .lock()
            .await
            .drain(..)
            .map(|mail| mail.to_response_input_item())
            .collect()
    }

    pub(crate) async fn queue_response_items_for_next_turn(&self, items: Vec<ResponseInputItem>) {
        if items.is_empty() {
            return;
        }

        self.idle_pending_input.lock().await.extend(items);
    }

    pub(crate) async fn take_queued_response_items_for_next_turn(&self) -> Vec<ResponseInputItem> {
        std::mem::take(&mut *self.idle_pending_input.lock().await)
    }

    pub(crate) async fn has_queued_response_items_for_next_turn(&self) -> bool {
        !self.idle_pending_input.lock().await.is_empty()
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

    /// Clear any pending waiters and input buffered for the current turn.
    pub(crate) async fn clear_pending(&self, active_turn: &ActiveTurn) {
        let mut turn_state = active_turn.turn_state.lock().await;
        turn_state.clear_pending_waiters();
        turn_state.pending_input.items.clear();
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
        let mut turn_state = turn_state.lock().await;
        if !turn_state.pending_input.items.is_empty() {
            return;
        }
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
        self.accept_mailbox_delivery_for_turn_state(turn_state.as_ref())
            .await;
    }

    pub(super) async fn accept_mailbox_delivery_for_turn_state(
        &self,
        turn_state: &Mutex<TurnState>,
    ) {
        turn_state
            .lock()
            .await
            .accept_mailbox_delivery_for_current_turn();
    }

    pub(super) async fn push_pending_input_and_accept_mailbox_delivery_for_turn_state(
        &self,
        turn_state: &Mutex<TurnState>,
        input: ResponseInputItem,
    ) {
        let mut turn_state = turn_state.lock().await;
        turn_state.pending_input.items.push(input);
        turn_state.accept_mailbox_delivery_for_current_turn();
    }

    pub(crate) async fn extend_pending_input_for_turn_state(
        &self,
        turn_state: &Mutex<TurnState>,
        input: Vec<ResponseInputItem>,
    ) {
        turn_state.lock().await.pending_input.items.extend(input);
    }

    pub(crate) async fn take_pending_input_for_turn_state(
        &self,
        turn_state: &Mutex<TurnState>,
    ) -> Vec<ResponseInputItem> {
        turn_state.lock().await.pending_input.items.split_off(0)
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn inject_response_items(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
        input: Vec<ResponseInputItem>,
    ) -> Result<(), Vec<ResponseInputItem>> {
        let mut active = active_turn.lock().await;
        match active.as_mut() {
            Some(active_turn) => {
                active_turn
                    .turn_state
                    .lock()
                    .await
                    .pending_input
                    .items
                    .extend(input);
                Ok(())
            }
            None => Err(input),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn prepend_pending_input(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
        mut input: Vec<ResponseInputItem>,
    ) -> Result<(), ()> {
        let mut active = active_turn.lock().await;
        match active.as_mut() {
            Some(active_turn) => {
                let mut turn_state = active_turn.turn_state.lock().await;
                if !input.is_empty() {
                    let pending_input = &mut turn_state.pending_input;
                    input.append(&mut pending_input.items);
                    pending_input.items = input;
                }
                Ok(())
            }
            None => Err(()),
        }
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "active turn checks and turn state updates must remain atomic"
    )]
    pub(crate) async fn get_pending_input(
        &self,
        active_turn: &Mutex<Option<ActiveTurn>>,
    ) -> Vec<ResponseInputItem> {
        let (pending_input, accepts_mailbox_delivery) = {
            let mut active = active_turn.lock().await;
            match active.as_mut() {
                Some(active_turn) => {
                    let mut turn_state = active_turn.turn_state.lock().await;
                    (
                        turn_state.pending_input.items.split_off(0),
                        turn_state.accepts_mailbox_delivery_for_current_turn(),
                    )
                }
                None => (Vec::new(), true),
            }
        };
        if !accepts_mailbox_delivery {
            return pending_input;
        }
        let mailbox_items = self.drain_mailbox_input_items().await;
        if pending_input.is_empty() {
            mailbox_items
        } else if mailbox_items.is_empty() {
            pending_input
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
        let (has_turn_pending_input, accepts_mailbox_delivery) = {
            let active = active_turn.lock().await;
            match active.as_ref() {
                Some(active_turn) => {
                    let turn_state = active_turn.turn_state.lock().await;
                    (
                        !turn_state.pending_input.items.is_empty(),
                        turn_state.accepts_mailbox_delivery_for_current_turn(),
                    )
                }
                None => (false, true),
            }
        };
        if has_turn_pending_input {
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
            input_queue.drain_mailbox_input_items().await,
            vec![
                mail_one.to_response_input_item(),
                mail_two.to_response_input_item()
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
