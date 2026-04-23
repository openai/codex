use serde::Deserialize;
use serde::Serialize;

/// Lifecycle status attached to a directional thread-spawn edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ThreadSpawnEdgeStatus {
    /// The child thread is still live or resumable as an open spawned agent.
    Open,
    /// The child thread has been closed from the parent/child graph's perspective.
    Closed,
}
