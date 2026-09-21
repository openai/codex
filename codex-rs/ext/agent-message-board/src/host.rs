//! Host capabilities needed by a board without depending on codex-core.

use crate::PostMetadata;
use chrono::DateTime;
use chrono::Utc;
use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationDelivery {
    Accepted,
    SkippedInactive,
}

/// Tree-scoped membership, clock and notification access supplied by the host.
///
/// The host remains authoritative for membership even when runtimes are unloaded.
/// Clock reads use the posting agent's configured source, including simulated
/// time. Notification admission is atomic with turn completion: inactive agents
/// are skipped, and no notification may start work or survive into a later turn.
/// Backends may implement these capabilities with local handles or remote RPCs.
pub trait MessageBoardHost: Send + Sync {
    fn agent_path(&self, caller: ThreadId) -> BoxFuture<'_, Result<AgentPath>>;

    fn resolve_agent(&self, path: AgentPath) -> BoxFuture<'_, Result<ThreadId>>;

    fn current_time(&self, caller: ThreadId) -> BoxFuture<'_, Result<DateTime<Utc>>>;

    /// Only metadata is pushed. Full content is retrieved through bounded reads.
    fn notify(
        &self,
        recipient: ThreadId,
        post: PostMetadata,
    ) -> BoxFuture<'_, Result<NotificationDelivery>>;
}
