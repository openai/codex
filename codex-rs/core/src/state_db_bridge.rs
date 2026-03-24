use codex_rollout::db as rollout_state_db;
pub use codex_rollout::db::StateDbHandle;
pub use codex_rollout::db::apply_rollout_items;
pub use codex_rollout::db::find_rollout_path_by_id;
pub use codex_rollout::db::get_dynamic_tools;
pub use codex_rollout::db::list_thread_ids_db;
pub use codex_rollout::db::list_threads_db;
pub use codex_rollout::db::mark_thread_memory_mode_polluted;
pub use codex_rollout::db::normalize_cwd_for_state_db;
pub use codex_rollout::db::open_if_present;
pub use codex_rollout::db::persist_dynamic_tools;
pub use codex_rollout::db::read_repair_rollout_path;
pub use codex_rollout::db::reconcile_rollout;
pub use codex_rollout::db::touch_thread_updated_at;
pub use codex_state::LogEntry;

use crate::config::Config;

pub(crate) async fn init(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::init(config).await
}

pub async fn get_state_db(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::get_state_db(config).await
}
