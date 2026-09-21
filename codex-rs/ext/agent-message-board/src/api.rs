//! Operations on one board. Requests describe intent independently of tool schemas.

use crate::ChannelSummary;
use crate::Page;
use crate::PostContent;
use crate::PostMetadata;
use crate::PostPreview;
use crate::SubscriptionState;
use crate::ThreadPage;
use crate::ThreadSummary;
use codex_protocol::AgentPath;
use codex_protocol::SessionId;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;
use serde::Deserialize;
use serde::Serialize;
use std::num::NonZeroU32;
use uuid::Uuid;

// Local and remote implementations must remain usable through the same handle.
const _: Option<&dyn AgentMessageBoard> = None;

/// Stores discussions and subscriptions for one persistent agent tree.
///
/// Every operation validates caller membership. IDs are scoped to this board;
/// an agent runtime's ThreadId is distinct from a discussion's root post UUID.
/// Mutation success acknowledges acceptance, not that recipients read a post.
/// Implementations enforce hard input/output limits and return backend failures
/// as errors. They own atomic subscription changes, posting and recipient
/// selection. Notification delivery must neither wake finalized agents nor
/// leave notifications for a later turn.
///
/// Boxed Send futures support an Arc<dyn AgentMessageBoard>, like AgentControl.
pub trait AgentMessageBoard: Send + Sync {
    fn identity(&self) -> SessionId;

    fn create_channel(
        &self,
        caller: ThreadId,
        request: CreateChannelRequest,
    ) -> BoxFuture<'_, Result<ChannelSummary>>;

    fn list_channels(
        &self,
        caller: ThreadId,
        query: ChannelQuery,
    ) -> BoxFuture<'_, Result<Page<ChannelSummary>>>;

    /// Uses the caller's configured clock; clock failures must not create a post.
    /// The request ID identifies a logical call across retries. A retry with
    /// different input is an error; a successful retry returns the same metadata.
    fn post(&self, caller: ThreadId, request: PostRequest) -> BoxFuture<'_, Result<PostMetadata>>;

    fn list_threads(
        &self,
        caller: ThreadId,
        query: ThreadQuery,
    ) -> BoxFuture<'_, Result<Page<ThreadSummary>>>;

    fn search_posts(
        &self,
        caller: ThreadId,
        query: PostQuery,
    ) -> BoxFuture<'_, Result<Page<PostPreview>>>;

    fn read_thread(
        &self,
        caller: ThreadId,
        request: ReadThreadRequest,
    ) -> BoxFuture<'_, Result<ThreadPage>>;

    fn read_post(
        &self,
        caller: ThreadId,
        request: ReadPostRequest,
    ) -> BoxFuture<'_, Result<PostContent>>;

    /// Channel subscriptions concern new roots; thread subscriptions concern
    /// replies. Changing one does not change the other. Any member may change
    /// another member's subscription. Posting subscribes its author to the thread.
    fn set_subscription(
        &self,
        caller: ThreadId,
        request: SubscriptionRequest,
    ) -> BoxFuture<'_, Result<SubscriptionState>>;
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRequest {
    pub cursor: Option<String>,
    pub limit: NonZeroU32,
}

impl Default for PageRequest {
    fn default() -> Self {
        Self {
            cursor: None,
            limit: NonZeroU32::new(20).unwrap_or(NonZeroU32::MIN),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortDirection {
    NewestFirst,
    OldestFirst,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SubscriptionChange {
    Subscribe,
    Unsubscribe,
}

#[derive(Clone, Debug)]
pub struct CreateChannelRequest {
    pub channel_name: String,
    pub subscription: SubscriptionChange,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelQuery {
    pub query: Option<String>,
    pub direction: SortDirection,
    pub page: PageRequest,
}

/// Exactly one destination; discussion threads use their root post's ID.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PostDestination {
    Channel(String),
    NewChannel(String),
    Thread(Uuid),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostRequest {
    /// Host-generated tool invocation identity, never a model argument.
    pub request_id: String,
    pub destination: PostDestination,
    pub text: String,
    /// Resolved paths in this board's tree; duplicates notify only once.
    pub agents_to_notify: Vec<AgentPath>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThreadSort {
    Created,
    Activity,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadQuery {
    pub channel_name: String,
    pub sort: ThreadSort,
    pub direction: SortDirection,
    pub page: PageRequest,
    pub max_chars_per_post: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostQuery {
    pub channel_name: Option<String>,
    pub query: Option<String>,
    pub after_message_id: Option<Uuid>,
    pub author: Option<AgentPath>,
    pub page: PageRequest,
    pub max_chars_per_post: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReadThreadRequest {
    pub thread_id: Uuid,
    pub page: PageRequest,
    pub max_chars_per_post: NonZeroU32,
}

#[derive(Clone, Debug)]
pub struct ReadPostRequest {
    pub message_id: Uuid,
    /// Offsets and lengths count Unicode scalar values, not UTF-8 bytes.
    pub offset_chars: u32,
    pub limit_chars: NonZeroU32,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum SubscriptionTarget {
    Channel(String),
    Thread(Uuid),
}

#[derive(Clone, Debug)]
pub struct SubscriptionRequest {
    pub target: SubscriptionTarget,
    /// None changes the caller's subscription.
    pub target_agent: Option<AgentPath>,
    pub change: SubscriptionChange,
}
