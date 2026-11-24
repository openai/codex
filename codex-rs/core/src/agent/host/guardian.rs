//! Host-supplied root authorization evidence for Guardian reviews of detached agents.

use crate::GuardianRootSnapshot;
use codex_protocol::error::Result;
use futures::future::BoxFuture;

/// Reads root authorization evidence for this host-owned child runtime.
///
/// Install this provider before startup or resume through thread extensions. Each read must
/// capture the root runtime ID, history, authorization revisions, completeness and request
/// candidates from the same trusted snapshot. Return an error if the root cannot be read.
/// Repeated reads may observe new revisions or a replacement root runtime.
///
/// Freshness is best effort. A root revocation can affect the child's review only once
/// Guardian observes the updated evidence. If review preparation and validation both read
/// the same older revision, Guardian may still approve the action. A later update does not
/// retract an approval already returned.
pub trait AgentGuardianContext: Send + Sync {
    fn root_snapshot(&self) -> BoxFuture<'_, Result<GuardianRootSnapshot>>;
}
