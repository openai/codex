//! SQLite-backed state for rollout metadata.
//!
//! This crate is intentionally small and focused: it extracts rollout metadata
//! from JSONL rollouts and mirrors it into a local SQLite database.

mod db;
mod extract;
mod migrations;
mod model;
mod paths;
mod runtime;

pub use db::StateDb;
pub use extract::extract_metadata_from_rollout;
pub use model::Anchor;
pub use model::BackfillStats;
pub use model::ExtractionOutcome;
pub use model::SortKey;
pub use model::ThreadMetadata;
pub use model::ThreadsPage;
pub use runtime::STATE_DB_FILENAME;
pub use runtime::StateRuntime;

/// Errors encountered during DB operations. Tags: [stage]
pub(crate) const DB_ERROR_METRIC: &str = "codex.db.error";
/// Metrics on backfill process during first init of the db. Tags: [status]
pub(crate) const DB_METRIC_BACKFILL: &str = "codex.db.backfill";
/// Metrics on errors during comparison between DB and rollout file. Tags: [stage]
pub(crate) const DB_METRIC_COMPARE_ERROR: &str = "codex.db.compare_error";
