use codex_protocol::protocol::InterAgentCommunication;
use std::collections::VecDeque;
use std::sync::atomic::AtomicU64;
use std::sync::atomic::Ordering;
use tokio::sync::mpsc;
use tokio::sync::watch;

#[cfg(test)]
use codex_protocol::AgentPath;

pub(crate) struct Mailbox {
    tx: mpsc::UnboundedSender<InterAgentCommunication>,
    next_seq: AtomicU64,
    seq_tx: watch::Sender<u64>,
}

pub(crate) struct MailboxReceiver {
    rx: mpsc::UnboundedReceiver<InterAgentCommunication>,
    pending_mails: VecDeque<InterAgentCommunication>,
}

impl Mailbox {
    pub(crate) fn new() -> (Self, MailboxReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();
        let (seq_tx, _) = watch::channel(0);
        (
            Self {
                tx,
                next_seq: AtomicU64::new(0),
                seq_tx,
            },
            MailboxReceiver {
                rx,
                pending_mails: VecDeque::new(),
            },
        )
    }

    pub(crate) fn subscribe(&self) -> watch::Receiver<u64> {
        self.seq_tx.subscribe()
    }

    pub(crate) fn send(&self, communication: InterAgentCommunication) -> u64 {
        let seq = self.next_seq.fetch_add(1, Ordering::Relaxed) + 1;
        let _ = self.tx.send(communication);
        self.seq_tx.send_replace(seq);
        seq
    }
}

impl MailboxReceiver {
    fn sync_pending_mails(&mut self) {
        while let Ok(mail) = self.rx.try_recv() {
            self.pending_mails.push_back(mail);
        }
    }

    pub(crate) fn has_pending(&mut self) -> bool {
        self.sync_pending_mails();
        !self.pending_mails.is_empty()
    }

    pub(crate) fn has_pending_trigger_turn(&mut self) -> bool {
        self.sync_pending_mails();
        self.pending_mails.iter().any(|mail| mail.trigger_turn)
    }

    pub(crate) fn has_pending_queue_only(&mut self) -> bool {
        self.sync_pending_mails();
        self.pending_mails.iter().any(|mail| !mail.trigger_turn)
    }

    pub(crate) fn drain(&mut self) -> Vec<InterAgentCommunication> {
        self.sync_pending_mails();
        self.pending_mails.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
    async fn mailbox_assigns_monotonic_sequence_numbers() {
        let (mailbox, _receiver) = Mailbox::new();
        let mut seq_rx = mailbox.subscribe();

        let seq_a = mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "one",
            /*trigger_turn*/ false,
        ));
        let seq_b = mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "two",
            /*trigger_turn*/ false,
        ));

        seq_rx.changed().await.expect("first seq update");
        assert_eq!(*seq_rx.borrow(), seq_b);
        assert_eq!(seq_a, 1);
        assert_eq!(seq_b, 2);
    }

    #[tokio::test]
    async fn mailbox_drains_in_delivery_order() {
        let (mailbox, mut receiver) = Mailbox::new();
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

        mailbox.send(mail_one.clone());
        mailbox.send(mail_two.clone());

        assert_eq!(receiver.drain(), vec![mail_one, mail_two]);
        assert!(!receiver.has_pending());
    }

    #[tokio::test]
    async fn mailbox_tracks_pending_trigger_turn_mail() {
        let (mailbox, mut receiver) = Mailbox::new();

        mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "queued",
            /*trigger_turn*/ false,
        ));
        assert!(!receiver.has_pending_trigger_turn());

        mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "wake",
            /*trigger_turn*/ true,
        ));
        assert!(receiver.has_pending_trigger_turn());
    }

    #[tokio::test]
    async fn mailbox_tracks_pending_queue_only_mail() {
        let (mailbox, mut receiver) = Mailbox::new();

        mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "wake",
            /*trigger_turn*/ true,
        ));
        assert!(!receiver.has_pending_queue_only());

        mailbox.send(make_mail(
            AgentPath::root(),
            AgentPath::try_from("/root/worker").expect("agent path"),
            "queued",
            /*trigger_turn*/ false,
        ));
        assert!(receiver.has_pending_queue_only());
    }
}
