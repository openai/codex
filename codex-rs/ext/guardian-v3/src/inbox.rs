//! Each acting agent's reviewer owns its channel subscriptions and inbox of post IDs.
//! Each committed channel post reaches its current subscribers once per original.
//! Replacement model conversations replay the inbox with their own admission cursor.

use crate::EvidenceCursor;
use crate::EvidenceId;
use crate::EvidenceSnapshot;
use crate::GUARDIAN_BOARD_NAMESPACE;
use crate::delivery::evidence_batch;
use anyhow::ensure;
use codex_guardian_reviewer::ReviewDeadline;
use codex_guardian_reviewer::run_before_review_deadline;
use codex_message_board::AgentMessageBoard;
use codex_message_board::AppendRequest;
use codex_message_board::Page;
use codex_message_board::PageRequest;
use codex_message_board::SubscriptionChange;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result as CodexResult;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::TruncationPolicy;
use std::collections::BTreeSet;
use std::collections::HashMap;
use std::collections::HashSet;
use std::future::Future;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::sync::PoisonError;
use std::sync::Weak;
use tokio::sync::RwLock;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

type Subscribers = HashMap<String, HashMap<Uuid, Weak<ActorInbox>>>;

// A retry of an already committed channel post must not notify later subscribers.
// Only the current original can be retried by the serial housekeeper.
#[derive(Default)]
struct PublishedPosts {
    original: Option<EvidenceId>,
    post_ids: HashSet<Uuid>,
}

/// Routes one root's posts to live reviewers, indexed by channel and inbox identity.
pub struct LiveDelivery {
    pub(super) root: ThreadId,
    // The sole housekeeper advances this after publishing. Resumed intake starts here.
    routed: Mutex<EvidenceCursor>,
    subscribers: Mutex<Subscribers>,
    // Keep membership changes from landing between an append and inbox delivery.
    posting: RwLock<PublishedPosts>,
}

impl LiveDelivery {
    /// Start delivery at the beginning of a fresh root's history.
    pub fn new(root: ThreadId) -> Self {
        Self::from_cursor(EvidenceCursor::start(root))
    }

    /// Initialize a tree at the intake position selected on its first activation.
    pub fn from_cursor(cursor: EvidenceCursor) -> Self {
        Self {
            root: cursor.thread_id(),
            routed: Mutex::new(cursor),
            subscribers: Mutex::default(),
            posting: RwLock::default(),
        }
    }

    pub(super) fn cursor(&self) -> EvidenceCursor {
        self.routed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }

    pub(super) fn acknowledge(
        &self,
        snapshot: &EvidenceSnapshot,
        count: usize,
    ) -> anyhow::Result<()> {
        self.routed
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .acknowledge(snapshot, count)
    }

    pub(super) fn register(self: &Arc<Self>, actor: ThreadId) -> Arc<ActorInbox> {
        Arc::new(ActorInbox {
            id: Uuid::now_v7(),
            delivery: Arc::clone(self),
            actor,
            state: Mutex::new(Some(InboxState::default())),
            length: watch::Sender::new(0),
        })
    }

    fn subscribers(&self) -> MutexGuard<'_, Subscribers> {
        self.subscribers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }

    #[expect(
        clippy::await_holding_invalid_type,
        reason = "serialize each append and inbox delivery with subscription changes"
    )]
    pub(super) async fn append(
        &self,
        board: &dyn AgentMessageBoard,
        request: AppendRequest,
        id: EvidenceId,
        deadline: ReviewDeadline,
        cancellation: &CancellationToken,
    ) -> anyhow::Result<()> {
        let mut published =
            run_before_review_deadline(deadline, Some(cancellation), self.posting.write())
                .await
                .map_err(|reason| anyhow::anyhow!("waiting to append evidence: {reason:?}"))?;
        crate::housekeeper::check_active(deadline, cancellation)?;
        // Finish the accepted write and delivery even during shutdown.
        let post = board.append(self.root, request).await?;
        if published.original != Some(id) {
            published.original = Some(id);
            published.post_ids.clear();
        }
        if published.post_ids.contains(&post.message_id) {
            return Ok(());
        }
        // Release the subscriber lock before touching inboxes. Dropping the last
        // reference to an inbox can unregister it from the same index.
        let inboxes: Vec<_> = self
            .subscribers()
            .get(&post.channel_name)
            .into_iter()
            .flat_map(HashMap::values)
            .filter_map(Weak::upgrade)
            .collect();
        for inbox in inboxes {
            inbox.enqueue(id, post.message_id)?;
        }
        published.post_ids.insert(post.message_id);
        Ok(())
    }
}

pub(super) struct ActorInbox {
    id: Uuid,
    pub(super) delivery: Arc<LiveDelivery>,
    pub(super) actor: ThreadId,
    // None once the owning ActorReviewer has shut down.
    state: Mutex<Option<InboxState>>,
    length: watch::Sender<usize>,
}

#[derive(Default)]
struct InboxState {
    channels: BTreeSet<String>,
    posts: Vec<(u64, Uuid)>,
}

impl Drop for ActorInbox {
    fn drop(&mut self) {
        self.close();
    }
}

fn unsubscribe(subscribers: &mut Subscribers, channel: &str, id: Uuid) {
    if let Some(members) = subscribers.get_mut(channel) {
        members.remove(&id);
        if members.is_empty() {
            subscribers.remove(channel);
        }
    }
}

impl ActorInbox {
    fn state(&self) -> MutexGuard<'_, Option<InboxState>> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn enqueue(&self, id: EvidenceId, post: Uuid) -> anyhow::Result<()> {
        let mut state = self.state();
        let Some(state) = state.as_mut() else {
            return Ok(());
        };
        if let Some(&(last, _)) = state.posts.last() {
            // Another subscribed channel may already have delivered this original.
            if last == id.ordinal {
                return Ok(());
            }
            ensure!(
                last < id.ordinal,
                "Guardian inbox received evidence out of source order"
            );
        }
        state.posts.push((id.ordinal, post));
        self.length.send_replace(state.posts.len());
        Ok(())
    }

    pub(super) fn close(&self) {
        let Some(state) = self.state().take() else {
            return;
        };
        self.length.send_replace(0);
        let mut subscribers = self.delivery.subscribers();
        for channel in state.channels {
            unsubscribe(&mut subscribers, &channel, self.id);
        }
    }

    pub(super) fn subscribed_channels(&self, page: &PageRequest) -> CodexResult<Page<String>> {
        let state = self.state();
        let state = state.as_ref().ok_or(CodexErr::ThreadNotFound(self.actor))?;
        codex_message_board::paginate(page, state.channels.iter().cloned())
    }

    pub(super) async fn set_subscription(
        self: &Arc<Self>,
        channel: String,
        change: SubscriptionChange,
    ) -> CodexResult<()> {
        if change == SubscriptionChange::Subscribe {
            codex_message_board::validate_channel(&channel)?;
        }
        let _guard = self.delivery.posting.read().await;
        let mut state = self.state();
        let state = state.as_mut().ok_or(CodexErr::ThreadNotFound(self.actor))?;
        let mut subscribers = self.delivery.subscribers();
        match change {
            SubscriptionChange::Subscribe => {
                state.channels.insert(channel.clone());
                subscribers
                    .entry(channel)
                    .or_default()
                    .insert(self.id, Arc::downgrade(self));
            }
            SubscriptionChange::Unsubscribe => {
                state.channels.remove(&channel);
                unsubscribe(&mut subscribers, &channel, self.id);
            }
        }
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn queued_posts(&self) -> Vec<(u64, Uuid)> {
        self.state()
            .as_ref()
            .map(|state| state.posts.clone())
            .unwrap_or_default()
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
                && board.identity() == self.delivery.root.into()
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
            let positions = self
                .state()
                .as_ref()
                .ok_or(CodexErr::ThreadNotFound(self.actor))?
                .posts[start..end]
                .to_vec();
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
