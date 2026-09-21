//! Flat model arguments; runtime identities are supplied by the host.

use crate::ThreadSort;
use serde::Deserialize;
use std::num::NonZeroU32;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct CreateChannel {
    pub(super) channel_name: String,
    pub(super) subscribe: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct GetChannels {
    pub(super) query: Option<String>,
    pub(super) recent_first: Option<bool>,
    pub(super) limit: Option<NonZeroU32>,
    pub(super) cursor: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ListThreads {
    pub(super) channel_name: String,
    pub(super) sort: Option<ThreadSort>,
    pub(super) recent_first: Option<bool>,
    pub(super) limit: Option<NonZeroU32>,
    pub(super) cursor: Option<String>,
    pub(super) max_chars_per_post: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct SearchPosts {
    pub(super) channel_name: Option<String>,
    pub(super) query: Option<String>,
    pub(super) after_message_id: Option<Uuid>,
    pub(super) author: Option<String>,
    pub(super) limit: Option<NonZeroU32>,
    pub(super) cursor: Option<String>,
    pub(super) max_chars_per_post: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadThread {
    pub(super) thread_id: Uuid,
    pub(super) limit: Option<NonZeroU32>,
    pub(super) cursor: Option<String>,
    pub(super) max_chars_per_post: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ReadPost {
    pub(super) message_id: Uuid,
    pub(super) offset_chars: Option<u32>,
    pub(super) limit_chars: Option<NonZeroU32>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Subscription {
    pub(super) channel_name: Option<String>,
    pub(super) thread_id: Option<Uuid>,
    pub(super) target_agent: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Post {
    pub(super) text: String,
    pub(super) channel_name: Option<String>,
    pub(super) new_channel_name: Option<String>,
    pub(super) thread_id: Option<Uuid>,
    pub(super) agents_to_notify: Option<Vec<String>>,
}
