//! Rollout persistence and discovery for recorded Codex sessions.

use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::LazyLock;

use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_state::ThreadMetadataBuilder;

mod error;
mod git_info;
pub mod list;
mod metadata;
mod path_utils;
pub mod policy;
pub mod recorder;
pub mod session_index;
mod state_db;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub use list::Cursor;
pub use list::ThreadItem;
pub use list::ThreadSortKey;
pub use list::ThreadsPage;
pub use list::find_archived_thread_path_by_id_str;
pub use list::find_thread_path_by_id_str;
pub use list::parse_cursor;
pub use list::read_head_for_summary;
pub use list::read_session_meta_line;
pub use list::rollout_date_parts;
pub use policy::EventPersistenceMode;
pub use policy::should_persist_response_item_for_memories;
pub use recorder::RolloutRecorder;
pub use recorder::RolloutRecorderParams;
pub use session_index::append_thread_name;
pub use session_index::find_thread_name_by_id;
pub use session_index::find_thread_names_by_ids;
pub use session_index::find_thread_path_by_name_str;
pub use state_db::StateDbHandle;
pub use state_db::read_repair_rollout_path;
pub use state_db::reconcile_rollout;

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

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RolloutConfig {
    pub codex_home: PathBuf,
    pub sqlite_home: PathBuf,
    pub cwd: PathBuf,
    pub model_provider_id: String,
    pub generate_memories: bool,
}

impl RolloutConfig {
    pub fn new(
        codex_home: PathBuf,
        sqlite_home: PathBuf,
        cwd: PathBuf,
        model_provider_id: String,
        generate_memories: bool,
    ) -> Self {
        Self {
            codex_home,
            sqlite_home,
            cwd,
            model_provider_id,
            generate_memories,
        }
    }
}

pub fn build_thread_metadata_builder(
    items: &[RolloutItem],
    rollout_path: &Path,
) -> Option<ThreadMetadataBuilder> {
    metadata::builder_from_items(items, rollout_path)
}

pub async fn spawn_backfill_if_needed(
    runtime: Option<Arc<codex_state::StateRuntime>>,
    config: &RolloutConfig,
) {
    let Some(runtime) = runtime else {
        return;
    };
    let backfill_state = match runtime.get_backfill_state().await {
        Ok(state) => state,
        Err(err) => {
            tracing::warn!(
                "failed to read backfill state at {}: {err}",
                config.codex_home.display()
            );
            return;
        }
    };
    if backfill_state.status == codex_state::BackfillStatus::Complete {
        return;
    }
    let runtime_for_backfill = Arc::clone(&runtime);
    let config = config.clone();
    tokio::spawn(async move {
        metadata::backfill_sessions(runtime_for_backfill.as_ref(), &config).await;
    });
}

pub async fn has_recorded_sessions(codex_home: &Path, default_provider: &str) -> io::Result<bool> {
    list::has_recorded_sessions(codex_home, default_provider).await
}

pub fn session_init_error_message(err: &anyhow::Error, codex_home: &Path) -> String {
    error::session_init_error_message(err, codex_home)
}
