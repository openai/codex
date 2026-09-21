//! Message-board records shared by backends and tool adapters.

use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use serde::Deserialize;
use serde::Serialize;
use serde::Serializer;
use serde::ser::SerializeStruct;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostMetadata {
    pub message_id: Uuid,
    pub channel_name: String,
    pub author: AgentPath,
    pub thread_id: Uuid,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostPreview {
    #[serde(flatten)]
    pub metadata: PostMetadata,
    pub text_preview: String,
    pub n_chars: usize,
    pub truncated: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PostContent {
    #[serde(flatten)]
    pub metadata: PostMetadata,
    pub text: String,
    pub n_chars: usize,
    pub next_offset_chars: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadSummary {
    pub thread_id: Uuid,
    pub root_post: PostPreview,
    pub reply_count: usize,
    pub last_activity_at: DateTime<Utc>,
    pub latest_reply: Option<PostPreview>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChannelSummary {
    pub channel_name: String,
    pub created_at: DateTime<Utc>,
    pub created_by: AgentPath,
    pub message_count: usize,
    pub last_message_id: Option<Uuid>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubscriptionState {
    pub channel_name: String,
    pub thread_id: Option<Uuid>,
    pub target_agent: AgentPath,
    pub enabled: bool,
    pub last_message_id: Option<Uuid>,
}

/// A bounded page. Counts and end-of-list markers are derived at serialization.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Page<T> {
    pub results: Vec<T>,
    pub next_cursor: Option<String>,
}

impl<T: Serialize> Serialize for Page<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut page = serializer.serialize_struct("Page", 4)?;
        page.serialize_field("results", &self.results)?;
        page.serialize_field("n_returned", &self.results.len())?;
        page.serialize_field("has_more", &self.next_cursor.is_some())?;
        page.serialize_field("next_cursor", &self.next_cursor)?;
        page.end()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThreadPage {
    pub root_post: PostPreview,
    #[serde(flatten)]
    pub replies: Page<PostPreview>,
}
