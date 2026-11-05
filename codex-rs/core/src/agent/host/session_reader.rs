//! Read-only host agent session membership for native context construction.

use codex_protocol::AgentPath;
use codex_protocol::ThreadId;
use codex_protocol::error::Result;
use futures::future::BoxFuture;

/// An agent's identity in the selected owner's agent session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AgentRef {
    pub thread_id: ThreadId,
    pub path: AgentPath,
}

/// An agent session view scoped to one runtime, installed before host-owned startup or resume.
///
/// Reads must not register, load, or restore agents. Return an error when membership
/// is unavailable. Implementations may read a local cache or query their coordinator.
/// Core reads child paths for its bounded V2 roster when rendering an environment fragment.
/// If the read fails, Core renders the fragment without a roster.
pub trait AgentSessionReader: Send + Sync {
    /// Returns the root, including self when this runtime is the root.
    fn root(&self) -> BoxFuture<'_, Result<AgentRef>>;

    /// Returns the structural parent; only the root has no parent.
    fn parent(&self) -> BoxFuture<'_, Result<Option<AgentRef>>>;

    /// Returns registered direct children, including unloaded members, excluding
    /// provisional reservations and historical/removed relationships. Return loaded
    /// children first, then alphabetical paths within each group, as native rosters do.
    fn children(&self) -> BoxFuture<'_, Result<Vec<AgentRef>>>;
}
