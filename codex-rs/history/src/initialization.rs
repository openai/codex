//! Request-only provenance for the history installed when a runtime starts.
//! Warm resumes retain this value: reconnecting does not reconstruct history or
//! explain metadata that was already missing before the reconnect.

use crate::InitialHistory;
use serde::Serialize;

/// Describes a reconstruction path, not proof that tool metadata was captured or lost.
/// Fork temperature records whether the source runtime was loaded and not shut down
/// at initialization. The value lasts for the runtime, including after compaction;
/// consumers must check missing tool metadata independently for each window.
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HistoryInitialization {
    New,
    Cleared,
    ColdResume,
    WarmFork,
    ColdFork,
    SuppliedHistory,
}

impl HistoryInitialization {
    /// Classifies history without assuming that a fork's source runtime is available.
    pub fn from_history(history: &InitialHistory) -> Self {
        match history {
            InitialHistory::New => Self::New,
            InitialHistory::Cleared => Self::Cleared,
            InitialHistory::Resumed(_) => Self::ColdResume,
            // Callers with a known fork source supply its runtime availability.
            InitialHistory::Forked(_) => Self::SuppliedHistory,
        }
    }
}
