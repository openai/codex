//! Record one committed board post per original for each subscribed Guardian, in source order.

use crate::EvidenceId;
use anyhow::ensure;
use codex_protocol::ThreadId;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use uuid::Uuid;

/// Subscriber inboxes for originals routed by one root thread's housekeeper.
pub struct LiveDelivery {
    pub(super) root: ThreadId,
    actors: Mutex<HashMap<ThreadId, Arc<ActorInbox>>>,
}

impl LiveDelivery {
    pub fn new(root: ThreadId) -> Self {
        Self {
            root,
            actors: Mutex::default(),
        }
    }

    /// A persisted subscription can receive a post before the actor's reviewer starts.
    pub(super) fn inbox(&self, actor: ThreadId) -> Arc<ActorInbox> {
        self.actors
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(actor)
            .or_default()
            .clone()
    }

    /// Single root writer: all destinations for this original have committed. Each
    /// recipient occurs once even when several matching channels got the same original.
    pub(super) fn publish(
        &self,
        id: EvidenceId,
        recipients: HashMap<ThreadId, Uuid>,
    ) -> anyhow::Result<()> {
        for (actor, post) in recipients {
            let inbox = self.inbox(actor);
            let mut posts = inbox.posts.lock().unwrap_or_else(PoisonError::into_inner);
            ensure!(
                posts.last().is_none_or(|(last, _)| *last < id.ordinal),
                "Guardian inbox received evidence out of source order"
            );
            posts.push((id.ordinal, post));
        }
        Ok(())
    }
}

#[derive(Default)]
pub(super) struct ActorInbox {
    posts: Mutex<Vec<(u64, Uuid)>>,
}

#[cfg(test)]
impl ActorInbox {
    pub(super) fn queued_posts(&self) -> Vec<(u64, Uuid)> {
        self.posts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(super) fn len(&self) -> usize {
        self.posts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len()
    }
}
