//! Retains queue-only mail while local sessions are unloaded.
//! Eviction transfers mail before releasing the recipient's residency guard.

use crate::agent_communication::PENDING_MAILBOX_MESSAGES;
use codex_diagnostics::GaugeGuard;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::protocol::InterAgentCommunication;
use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::PoisonError;
use tokio::sync::watch;

pub(super) struct Mailboxes(Mutex<Option<HashMap<ThreadId, Mailbox>>>);

impl Default for Mailboxes {
    fn default() -> Self {
        Self(Mutex::new(Some(HashMap::new())))
    }
}

#[derive(Default)]
struct Mailbox {
    pending: Vec<StoredMail>,
    state: watch::Sender<bool>,
}

struct StoredMail {
    id: Option<String>,
    communication: InterAgentCommunication,
    _diagnostics_guard: GaugeGuard,
}

impl Mailboxes {
    pub(super) fn enqueue(
        &self,
        agent: ThreadId,
        id: Option<String>,
        messages: Vec<InterAgentCommunication>,
    ) -> Result<()> {
        let mut mailboxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let mailbox = mailboxes
            .as_mut()
            .ok_or_else(|| CodexErr::InvalidRequest("agent runtime is shutting down".into()))?
            .entry(agent)
            .or_default();
        mailbox
            .pending
            .extend(messages.into_iter().map(|communication| StoredMail {
                id: id.clone(),
                communication,
                _diagnostics_guard: PENDING_MAILBOX_MESSAGES.track(),
            }));
        mailbox.state.send_replace(!mailbox.pending.is_empty());
        Ok(())
    }

    pub(super) fn take(&self, agent: ThreadId) -> Vec<InterAgentCommunication> {
        let mut mailboxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(mailbox) = mailboxes
            .as_mut()
            .and_then(|mailboxes| mailboxes.get_mut(&agent))
        else {
            return Vec::new();
        };
        let pending = std::mem::take(&mut mailbox.pending);
        mailbox.state.send_if_modified(std::mem::take);
        pending
            .into_iter()
            .map(|mail| {
                // Mail transferred during eviction was already received by the old session.
                if let Some(id) = mail.id {
                    crate::agent_communication::emit_agent_communication_receive(&id);
                }
                mail.communication
            })
            .collect()
    }

    pub(super) fn watch(&self, agent: ThreadId) -> watch::Receiver<bool> {
        let mut mailboxes = self.0.lock().unwrap_or_else(PoisonError::into_inner);
        match mailboxes.as_mut() {
            Some(mailboxes) => {
                // Private delegates and cancelled startups can stop without explicit removal.
                // Retain unread mail and live subscriptions, but reclaim abandoned empty entries.
                mailboxes.retain(|_, mailbox| {
                    !mailbox.pending.is_empty() || mailbox.state.receiver_count() != 0
                });
                mailboxes.entry(agent).or_default().state.subscribe()
            }
            None => watch::channel(/*init*/ false).1,
        }
    }

    pub(super) fn remove(&self, agent: ThreadId) {
        if let Some(mailboxes) = self
            .0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .as_mut()
        {
            mailboxes.remove(&agent);
        }
    }

    pub(super) fn close(&self) {
        if let Some(mailboxes) = self.0.lock().unwrap_or_else(PoisonError::into_inner).take() {
            for mailbox in mailboxes.values() {
                mailbox.state.send_replace(false);
            }
        }
    }
}
