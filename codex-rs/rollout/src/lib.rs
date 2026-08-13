//! Rollout persistence and discovery for Codex session files.

use std::sync::LazyLock;

use codex_protocol::protocol::SessionSource;
use serde::de::Error as _;
use serde_json::Value;

pub(crate) mod compression;
pub(crate) mod config;
pub(crate) mod list;
mod maintenance;
pub(crate) mod metadata;
mod model_context;
mod ordinal;
mod persistence_metrics;
pub(crate) mod policy;
pub(crate) mod recorder;
mod reverse_jsonl_scanner;
mod rollout_file_name;
mod rollout_reference_index;
pub(crate) mod search;
pub(crate) mod session_index;
mod sqlite_metrics;
pub mod state_db;

pub use codex_history::CompactedItem;
pub use codex_history::InitialHistory;
pub use codex_history::ResponseItemEnvelope;
pub use codex_history::ResumedHistory;
pub use codex_history::RolloutItem;
pub use codex_history::RolloutLine;
pub(crate) use codex_protocol::protocol;

/// Decodes a persisted rollout record without Serde's flattened-envelope buffering.
///
/// With `serde_json/arbitrary_precision`, Serde's generic buffer cannot replay
/// floating-point values nested inside flattened or internally tagged fields:
/// https://github.com/serde-rs/json/issues/721
/// https://github.com/serde-rs/serde/issues/1183
///
/// Keep this JSON-specific workaround at the persistence boundary so history
/// remains format-neutral and resume and projection use the same item decoder.
/// Remove it once Serde supports format-specific buffering.
pub fn decode_rollout_line(value: Value) -> serde_json::Result<RolloutLine> {
    let Value::Object(mut fields) = value else {
        return serde_json::from_value(value);
    };
    let timestamp = fields
        .remove("timestamp")
        .ok_or_else(|| serde_json::Error::missing_field("timestamp"))
        .and_then(serde_json::from_value)?;
    let ordinal = fields
        .remove("ordinal")
        .map(serde_json::from_value::<Option<u64>>)
        .transpose()?
        .flatten();
    let item = serde_json::from_value(Value::Object(fields))?;

    Ok(RolloutLine {
        timestamp,
        ordinal,
        item,
    })
}

pub const SESSIONS_SUBDIR: &str = "sessions";
pub const ARCHIVED_SESSIONS_SUBDIR: &str = "archived_sessions";
pub static INTERACTIVE_SESSION_SOURCES: LazyLock<Vec<SessionSource>> = LazyLock::new(|| {
    vec![
        SessionSource::Cli,
        SessionSource::VSCode,
        SessionSource::Custom("atlas".to_string()),
        SessionSource::Custom("chatgpt".to_string()),
    ]
});

pub use codex_protocol::protocol::SessionMeta;
pub use compression::RolloutLineReader;
pub use compression::existing_rollout_path;
pub use compression::open_rollout_line_reader;
pub use compression::plain_rollout_path;
pub use compression::spawn_rollout_compression_worker;

/// Materializes a compressed rollout as plain JSONL before another rollout references it.
pub async fn materialize_rollout_for_reference(
    path: &std::path::Path,
) -> std::io::Result<std::path::PathBuf> {
    compression::materialize_rollout_for_append(path).await
}
pub use config::Config;
pub use config::RolloutConfig;
pub use config::RolloutConfigView;
pub use list::Cursor;
pub use list::SortDirection;
pub use list::ThreadItem;
pub use list::ThreadListConfig;
pub use list::ThreadListLayout;
pub use list::ThreadSortKey;
pub use list::ThreadsPage;
pub use list::find_archived_thread_path_by_id_str;
pub use list::find_rollout_path_by_rollout_id;
pub use list::find_thread_path_by_id_str;
#[deprecated(note = "use find_thread_path_by_id_str")]
pub use list::find_thread_path_by_id_str as find_conversation_path_by_id_str;
pub use list::get_threads;
pub use list::get_threads_in_root;
pub use list::parse_cursor;
pub use list::read_head_for_summary;
pub use list::read_session_meta_line;
pub use list::read_thread_item_from_rollout;
pub use list::rollout_date_parts;
pub use maintenance::RolloutMaintenanceGuard;
pub use maintenance::try_acquire_rollout_maintenance_lock;
pub use metadata::builder_from_items;
pub use metadata::rollout_id_from_path;
pub use model_context::ModelContextScan;
pub use model_context::ModelContextScanProgress;
pub use persistence_metrics::RolloutPersistenceBatchMeasurement;
pub use persistence_metrics::RolloutPersistenceTelemetry;
pub use persistence_metrics::measure_and_filter_rollout_items;
pub use policy::is_persisted_rollout_item;
pub use policy::persisted_rollout_items;
pub use policy::should_persist_response_item_for_memories;
pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;
pub use recorder::append_rollout_item_to_path;
pub use reverse_jsonl_scanner::ReverseJsonlScanner;
pub use reverse_jsonl_scanner::ScanOutcome;
pub use rollout_reference_index::RolloutReferenceIndex;
pub use search::first_rollout_content_match_snippet;
pub use search::search_rollout_matches;
pub use search::search_rollout_paths;
pub use session_index::append_thread_name;
pub use session_index::find_thread_meta_by_name_str;
pub use session_index::find_thread_meta_candidates_by_name_str;
pub use session_index::find_thread_name_by_id;
pub use session_index::find_thread_names_by_ids;
pub use session_index::remove_thread_name_entries;
pub use state_db::StateDbHandle;
pub use state_db::sqlite_telemetry_recorder;

#[cfg(test)]
mod tests;
