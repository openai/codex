#[cfg(not(target_arch = "wasm32"))]
use codex_rollout::state_db as rollout_state_db;

use crate::config::Config;

#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::StateDbHandle;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::apply_rollout_items;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::find_rollout_path_by_id;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::get_dynamic_tools;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::init;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::list_thread_ids_db;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::list_threads_db;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::mark_thread_memory_mode_polluted;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::normalize_cwd_for_state_db;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::open_if_present;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::persist_dynamic_tools;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::read_repair_rollout_path;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::reconcile_rollout;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_rollout::state_db::touch_thread_updated_at;
#[cfg(not(target_arch = "wasm32"))]
pub use codex_state::LogEntry;

#[cfg(not(target_arch = "wasm32"))]
pub async fn get_state_db(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::get_state_db(config).await
}

#[cfg(target_arch = "wasm32")]
pub mod wasm {
    use std::path::Path;
    use std::path::PathBuf;
    use std::sync::Arc;

    use chrono::DateTime;
    use chrono::Utc;
    use codex_protocol::ThreadId;
    use codex_protocol::dynamic_tools::DynamicToolSpec;
    use codex_protocol::protocol::RolloutItem;
    use codex_protocol::protocol::SessionSource;

    use crate::config::Config;
    use crate::rollout::list::Cursor;
    use crate::rollout::list::ThreadSortKey;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct StateRuntimeStub;

    pub type StateDbHandle = Arc<StateRuntimeStub>;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct LogEntry;

    #[derive(Debug, Clone, PartialEq, Eq)]
    pub struct ThreadMetadataStub {
        pub agent_nickname: Option<String>,
        pub agent_role: Option<String>,
    }

    pub async fn init(_config: &Config) -> Option<StateDbHandle> {
        None
    }

    pub async fn get_state_db(_config: &Config) -> Option<StateDbHandle> {
        None
    }

    pub async fn open_if_present(
        _codex_home: &Path,
        _default_provider: &str,
    ) -> Option<StateDbHandle> {
        None
    }

    pub async fn find_rollout_path_by_id(
        _context: Option<&StateRuntimeStub>,
        _thread_id: ThreadId,
        _archived_only: Option<bool>,
        _stage: &str,
    ) -> Option<PathBuf> {
        None
    }

    pub async fn get_dynamic_tools(
        _context: Option<&StateRuntimeStub>,
        _thread_id: ThreadId,
        _stage: &str,
    ) -> Option<Vec<DynamicToolSpec>> {
        None
    }

    pub async fn list_thread_ids_db(
        _context: Option<&StateRuntimeStub>,
        _codex_home: &Path,
        _page_size: usize,
        _cursor: Option<&Cursor>,
        _sort_key: ThreadSortKey,
        _allowed_sources: &[SessionSource],
        _model_providers: Option<&[String]>,
        _archived_only: bool,
        _stage: &str,
    ) -> Option<Vec<ThreadId>> {
        None
    }

    pub async fn list_threads_db(
        _context: Option<&StateRuntimeStub>,
        _codex_home: &Path,
        _page_size: usize,
        _cursor: Option<&Cursor>,
        _sort_key: ThreadSortKey,
        _allowed_sources: &[SessionSource],
        _model_providers: Option<&[String]>,
        _archived: bool,
        _search_term: Option<&str>,
    ) -> Option<crate::rollout::list::ThreadsPage> {
        None
    }

    pub fn normalize_cwd_for_state_db(cwd: &Path) -> PathBuf {
        cwd.to_path_buf()
    }

    pub async fn persist_dynamic_tools(
        _context: Option<&StateRuntimeStub>,
        _thread_id: ThreadId,
        _tools: Option<&[DynamicToolSpec]>,
        _stage: &str,
    ) {
    }

    pub async fn mark_thread_memory_mode_polluted(
        _context: Option<&StateRuntimeStub>,
        _thread_id: ThreadId,
        _stage: &str,
    ) {
    }

    pub async fn reconcile_rollout(
        _context: Option<&StateRuntimeStub>,
        _rollout_path: &Path,
        _default_provider: &str,
        _builder: Option<&()>,
        _items: &[RolloutItem],
        _archived_only: Option<bool>,
        _new_thread_memory_mode: Option<&str>,
    ) {
    }

    pub async fn read_repair_rollout_path(
        _context: Option<&StateRuntimeStub>,
        _thread_id: Option<ThreadId>,
        _archived_only: Option<bool>,
        _rollout_path: &Path,
    ) {
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn apply_rollout_items(
        _context: Option<&StateRuntimeStub>,
        _rollout_path: &Path,
        _default_provider: &str,
        _builder: Option<&()>,
        _items: &[RolloutItem],
        _stage: &str,
        _new_thread_memory_mode: Option<&str>,
        _updated_at_override: Option<DateTime<Utc>>,
    ) {
    }

    pub async fn touch_thread_updated_at(
        _context: Option<&StateRuntimeStub>,
        _thread_id: Option<ThreadId>,
        _updated_at: DateTime<Utc>,
        _stage: &str,
    ) -> bool {
        false
    }

    impl StateRuntimeStub {
        pub async fn clear_memory_data(&self) -> anyhow::Result<()> {
            Ok(())
        }

        pub async fn record_stage1_output_usage(
            &self,
            _thread_ids: &[ThreadId],
        ) -> anyhow::Result<usize> {
            Ok(0)
        }

        pub async fn get_thread(
            &self,
            _id: ThreadId,
        ) -> anyhow::Result<Option<ThreadMetadataStub>> {
            Ok(None)
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub use wasm::*;
