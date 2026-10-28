//! Pairs a parent response with its connection's routing pin. Reviewers retain this snapshot
//! through queueing/retries; the cookie never enters serialized request metadata or history.

use std::sync::Arc;

use codex_http_client::ChatGptRoutingAffinity;

#[derive(Clone, Debug)]
pub(crate) struct ParentResponse {
    pub(crate) id: String,
    pub(crate) routing_affinity: Option<ChatGptRoutingAffinity>,
}

/// Installed for the exclusive lifetime of one review, including its inference retries.
pub(crate) struct PendingReviewRouting(pub(crate) Option<Arc<ParentResponse>>);
