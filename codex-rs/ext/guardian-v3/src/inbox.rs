//! Deliver committed posts to the actors subscribed when each post was written. An inbox
//! retains only board references; each live or replacement reviewer owns its admission cursor.

use crate::EvidenceId;
use crate::GUARDIAN_BOARD_NAMESPACE;
use crate::delivery::evidence_batch;
use anyhow::ensure;
use codex_message_board::AgentMessageBoard;
use codex_message_board::PageRequest;
use codex_protocol::ThreadId;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use std::collections::HashMap;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use tokio::sync::watch;
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
            .or_insert_with(|| {
                Arc::new(ActorInbox {
                    root: self.root,
                    actor,
                    posts: Mutex::default(),
                    length: watch::Sender::new(0),
                })
            })
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
            let len = posts.len();
            drop(posts);
            inbox.length.send_replace(len);
        }
        Ok(())
    }
}

pub(super) struct ActorInbox {
    root: ThreadId,
    actor: ThreadId,
    posts: Mutex<Vec<(u64, Uuid)>>,
    length: watch::Sender<usize>,
}

impl ActorInbox {
    #[cfg(test)]
    pub(super) fn queued_posts(&self) -> Vec<(u64, Uuid)> {
        self.posts
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(super) fn subscribe(&self) -> watch::Receiver<usize> {
        self.length.subscribe()
    }

    pub(super) fn len(&self) -> usize {
        *self.length.borrow()
    }

    /// Drain a finite prefix. New arrivals can wake the consumer again without holding
    /// the housekeeper's write queue or changing the current operation's target.
    pub(super) async fn deliver<F, Fut>(
        &self,
        actor: ThreadId,
        board: &dyn AgentMessageBoard,
        delivered: &mut usize,
        through: usize,
        policy: TruncationPolicy,
        mut admit: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut(Vec<ResponseItem>) -> Fut,
        Fut: Future<Output = anyhow::Result<()>>,
    {
        ensure!(
            actor == self.actor
                && board.identity() == self.root.into()
                && board.storage_namespace() == Some(GUARDIAN_BOARD_NAMESPACE),
            "Guardian inbox opened with the wrong board or caller"
        );
        ensure!(
            *delivered <= self.len() && through <= self.len(),
            "invalid Guardian inbox cursor"
        );
        let page_size = PageRequest::default().limit.get() as usize;
        while *delivered < through {
            let start = *delivered;
            let end = through.min(start.saturating_add(page_size));
            let positions =
                self.posts.lock().unwrap_or_else(PoisonError::into_inner)[start..end].to_vec();
            let ids: Vec<_> = positions.iter().map(|(_, id)| *id).collect();
            let texts = board.read_post_texts_by_id(actor, &ids).await?;
            ensure!(
                texts.len() == positions.len(),
                "board returned an incomplete evidence page"
            );
            while *delivered < end {
                let pending = positions
                    .iter()
                    .zip(&texts)
                    .skip(*delivered - start)
                    .map(|((ordinal, _), text)| (*ordinal, text.as_str()));
                let (item, count) = evidence_batch(pending, policy)?;
                admit(vec![item]).await?;
                *delivered += count;
            }
        }
        Ok(())
    }
}
