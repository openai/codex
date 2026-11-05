//! Fixed ranges of original records for live, forward-only history consumers.
//! Positions identify persisted source records; they do not acknowledge downstream writes.

use codex_protocol::protocol::HistoryPosition;
use codex_rollout::RolloutItem;

/// Capture the current complete tail, optionally reading records after a saved position.
/// This API deliberately provides no implicit backfill or restart recovery.
pub enum OriginalHistoryRequest {
    /// Return only the current complete end position.
    Tail,
    /// Freeze the current complete tail and read originals after this position.
    /// Appends beyond that cutoff are excluded from the returned records.
    After(HistoryPosition),
}

/// One original record and the exclusive position immediately after it.
pub struct OriginalHistoryItem {
    pub item: RolloutItem,
    pub end: HistoryPosition,
}

/// Original records in source order and their fixed upper boundary.
/// A tail request returns no items. Reading never advances consumer progress.
pub struct OriginalHistoryRead {
    pub items: Vec<OriginalHistoryItem>,
    pub end: HistoryPosition,
}
