//! Tree-owned message boards for training. Agent handles share state; nothing is written to disk.
//! Mutations and recipient selection are atomic. Host callbacks run outside the state lock.
//! Opening a board prunes registry entries whose state has been released.

use crate::ChannelSummary;
use crate::CreateChannelRequest;
use crate::MessageBoardHost;
use crate::PostDestination;
use crate::PostMetadata;
use crate::PostPreview;
use crate::PostRequest;
use crate::SubscriptionChange;
use crate::SubscriptionRequest;
use crate::SubscriptionState;
use crate::SubscriptionTarget;
use caseless::default_case_fold_str;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use futures::StreamExt;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Weak;
use tokio::sync::Mutex;
use uuid::Uuid;

mod queries;

#[cfg(test)]
#[path = "in_memory_tests.rs"]
mod tests;

/// Shares board state across a tree while giving each agent its own host handle.
#[derive(Default)]
pub struct InMemoryMessageBoards {
    states: Mutex<HashMap<SessionId, Weak<Mutex<State>>>>,
}

impl InMemoryMessageBoards {
    pub async fn open(
        &self,
        identity: SessionId,
        host: Arc<dyn MessageBoardHost>,
    ) -> InMemoryAgentMessageBoard {
        let mut states = self.states.lock().await;
        states.retain(|_, state| state.strong_count() > 0);
        let entry = states.entry(identity).or_default();
        let state = entry.upgrade().unwrap_or_else(|| {
            let state = Arc::default();
            *entry = Arc::downgrade(&state);
            state
        });
        InMemoryAgentMessageBoard {
            identity,
            host,
            state,
        }
    }
}

/// A board held in memory for one training rollout.
/// The last handle releases its state; there is no persistence or recovery.
#[derive(Clone)]
pub struct InMemoryAgentMessageBoard {
    identity: SessionId,
    host: Arc<dyn MessageBoardHost>,
    state: Arc<Mutex<State>>,
}

#[derive(Default)]
struct State {
    channels: HashMap<String, Channel>,
    posts: Vec<Post>,
    by_id: HashMap<Uuid, usize>,
    requests: HashMap<(ThreadId, String), usize>,
    subscriptions: BTreeMap<SubscriptionTarget, HashMap<ThreadId, SubscriptionChange>>,
}

struct Channel {
    summary: ChannelSummary,
    search: String,
    posts: Vec<usize>,
    roots: Vec<usize>,
}

struct Post {
    metadata: PostMetadata,
    request: PostRequest,
    search: String,
    n_chars: usize,
    replies: Vec<usize>,
    latest_reply: Option<usize>,
}

impl Post {
    fn preview(&self, max_chars: usize) -> PostPreview {
        PostPreview {
            metadata: self.metadata.clone(),
            text_preview: self.request.text.chars().take(max_chars).collect(),
            n_chars: self.n_chars,
            truncated: self.n_chars > max_chars,
        }
    }
}

impl State {
    fn post_index(&self, id: Uuid) -> Result<usize> {
        self.by_id
            .get(&id)
            .copied()
            .ok_or_else(|| invalid("post not found in this board"))
    }

    fn thread_index(&self, id: Uuid) -> Result<usize> {
        let index = self.post_index(id)?;
        if self.posts[index].metadata.thread_id != id {
            return Err(invalid("thread_id must identify a top-level post"));
        }
        Ok(index)
    }

    fn key(&self, index: usize) -> (i64, usize) {
        (
            self.posts[index].metadata.created_at.timestamp_micros(),
            index,
        )
    }

    fn existing_post(
        &self,
        caller: ThreadId,
        request: &PostRequest,
    ) -> Result<Option<PostMetadata>> {
        self.requests
            .get(&(caller, request.request_id.clone()))
            .map(|index| {
                let post = &self.posts[*index];
                if post.request != *request {
                    return Err(invalid("request ID was already used for a different post"));
                }
                Ok(post.metadata.clone())
            })
            .transpose()
    }

    fn insert_channel(&mut self, name: &str, author: AgentPath, now: DateTime<Utc>) -> Result<()> {
        if name.is_empty()
            || name.len() > 128
            || name.trim() != name
            || name.chars().any(char::is_control)
        {
            return Err(invalid(
                "channel names must contain 1–128 bytes without edge whitespace or control characters",
            ));
        }
        if self.channels.contains_key(name) {
            return Err(invalid("channel already exists"));
        }
        self.channels.insert(
            name.to_owned(),
            Channel {
                summary: ChannelSummary {
                    channel_name: name.to_owned(),
                    created_at: now,
                    created_by: author,
                    message_count: 0,
                    last_message_id: None,
                },
                search: default_case_fold_str(name),
                posts: Vec::new(),
                roots: Vec::new(),
            },
        );
        Ok(())
    }

    fn subscribe(&mut self, target: SubscriptionTarget, caller: ThreadId) {
        self.subscriptions
            .entry(target)
            .or_default()
            .entry(caller)
            .or_insert(SubscriptionChange::Subscribe);
    }
}

impl InMemoryAgentMessageBoard {
    pub fn new(identity: SessionId, host: Arc<dyn MessageBoardHost>) -> Self {
        Self {
            identity,
            host,
            state: Arc::default(),
        }
    }

    async fn create_channel(
        &self,
        caller: ThreadId,
        request: CreateChannelRequest,
    ) -> Result<ChannelSummary> {
        let author = self.host.agent_path(caller).await?;
        let now = self.host.current_time(caller).await?;
        let mut state = self.state.lock().await;
        state.insert_channel(&request.channel_name, author, now)?;
        if request.subscription == SubscriptionChange::Subscribe {
            state.subscribe(
                SubscriptionTarget::Channel(request.channel_name.clone()),
                caller,
            );
        }
        Ok(state.channels[&request.channel_name].summary.clone())
    }

    async fn post(&self, caller: ThreadId, request: PostRequest) -> Result<PostMetadata> {
        // Accepted writes and their one-time fanout outlive cancellation of the tool call.
        let board = self.clone();
        tokio::spawn(async move { board.post_inner(caller, request).await })
            .await
            .map_err(|error| CodexErr::Io(std::io::Error::other(error)))?
    }

    async fn post_inner(&self, caller: ThreadId, request: PostRequest) -> Result<PostMetadata> {
        if request.text.is_empty()
            || request.text.len() > 64 * 1024
            || request.request_id.is_empty()
            || request.request_id.len() > 512
            || request.agents_to_notify.len() > 256
        {
            return Err(invalid(
                "post text, request ID or recipient count exceeds the board limits",
            ));
        }
        let author = self.host.agent_path(caller).await?;
        if let Some(post) = self.state.lock().await.existing_post(caller, &request)? {
            return Ok(post);
        }
        let mut recipients = HashSet::new();
        for path in &request.agents_to_notify {
            recipients.insert(self.host.resolve_agent(path.clone()).await?);
        }
        let now = self.host.current_time(caller).await?;
        let (metadata, notice) = {
            let mut state = self.state.lock().await;
            if let Some(post) = state.existing_post(caller, &request)? {
                return Ok(post);
            }
            let id = Uuid::now_v7();
            let (channel_name, root, target) = match &request.destination {
                PostDestination::Channel(name) => {
                    if !state.channels.contains_key(name) {
                        return Err(invalid("channel not found in this board"));
                    }
                    (name.clone(), id, SubscriptionTarget::Channel(name.clone()))
                }
                PostDestination::NewChannel(name) => {
                    state.insert_channel(name, author.clone(), now)?;
                    state.subscribe(SubscriptionTarget::Channel(name.clone()), caller);
                    (name.clone(), id, SubscriptionTarget::Channel(name.clone()))
                }
                PostDestination::Thread(root) => {
                    let index = state.thread_index(*root)?;
                    (
                        state.posts[index].metadata.channel_name.clone(),
                        *root,
                        SubscriptionTarget::Thread(*root),
                    )
                }
            };
            if let Some(subscribers) = state.subscriptions.get(&target) {
                recipients.extend(subscribers.iter().filter_map(|(agent, change)| {
                    (*change == SubscriptionChange::Subscribe).then_some(*agent)
                }));
            }
            recipients.remove(&caller);
            let metadata = PostMetadata {
                message_id: id,
                channel_name: channel_name.clone(),
                author,
                thread_id: root,
                created_at: now,
            };
            let index = state.posts.len();
            state
                .requests
                .insert((caller, request.request_id.clone()), index);
            state.by_id.insert(id, index);
            state.posts.push(Post {
                metadata: metadata.clone(),
                search: default_case_fold_str(&request.text),
                n_chars: request.text.chars().count(),
                request,
                replies: Vec::new(),
                latest_reply: None,
            });
            let last = state.channels[&channel_name].summary.last_message_id;
            let newest = last.is_none_or(|last| state.key(index) > state.key(state.by_id[&last]));
            #[expect(
                clippy::expect_used,
                reason = "destination was checked or inserted under this lock"
            )]
            let channel = state
                .channels
                .get_mut(&channel_name)
                .expect("validated channel");
            channel.posts.push(index);
            channel.summary.message_count += 1;
            if newest {
                channel.summary.last_message_id = Some(id);
            }
            if root == id {
                channel.roots.push(index);
            } else {
                let root_index = state.by_id[&root];
                let newest = state.posts[root_index]
                    .latest_reply
                    .is_none_or(|last| state.key(index) > state.key(last));
                let thread = &mut state.posts[root_index];
                thread.replies.push(index);
                if newest {
                    thread.latest_reply = Some(index);
                }
            }
            state.subscribe(SubscriptionTarget::Thread(root), caller);
            (metadata, state.posts[index].preview(/*max_chars*/ 150))
        };
        futures::stream::iter(recipients).for_each_concurrent(/*limit*/ 16, |recipient| {
            let notice = notice.clone();
            async move {
                if let Err(error) = self.host.notify(recipient, notice).await {
                    tracing::warn!(%recipient, %error, "Failed to deliver message-board notification");
                }
            }
        }).await;
        Ok(metadata)
    }

    async fn set_subscription(
        &self,
        caller: ThreadId,
        request: SubscriptionRequest,
    ) -> Result<SubscriptionState> {
        let caller_path = self.host.agent_path(caller).await?;
        let target_path = request.target_agent.unwrap_or(caller_path);
        let target_agent = self.host.resolve_agent(target_path.clone()).await?;
        let mut state = self.state.lock().await;
        let (channel_name, thread_id, last_message_id) = match &request.target {
            SubscriptionTarget::Channel(name) => {
                let channel = state
                    .channels
                    .get(name)
                    .ok_or_else(|| invalid("channel not found in this board"))?;
                (name.clone(), None, channel.summary.last_message_id)
            }
            SubscriptionTarget::Thread(root) => {
                let index = state.thread_index(*root)?;
                let post = &state.posts[index];
                let last = post
                    .latest_reply
                    .filter(|last| state.key(*last) > state.key(index))
                    .unwrap_or(index);
                (
                    post.metadata.channel_name.clone(),
                    Some(*root),
                    Some(state.posts[last].metadata.message_id),
                )
            }
        };
        state
            .subscriptions
            .entry(request.target)
            .or_default()
            .insert(target_agent, request.change);
        Ok(SubscriptionState {
            channel_name,
            thread_id,
            target_agent: target_path,
            enabled: request.change == SubscriptionChange::Subscribe,
            last_message_id,
        })
    }
}

fn invalid(message: impl Into<String>) -> CodexErr {
    CodexErr::InvalidRequest(message.into())
}
