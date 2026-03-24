use codex_rollout::state_db as rollout_state_db;
pub use codex_rollout::state_db::StateDbHandle;
pub use codex_rollout::state_db::apply_rollout_items;
pub use codex_rollout::state_db::find_rollout_path_by_id;
pub use codex_rollout::state_db::get_dynamic_tools;
pub use codex_rollout::state_db::list_thread_ids_db;
pub use codex_rollout::state_db::list_threads_db;
pub use codex_rollout::state_db::mark_thread_memory_mode_polluted;
pub use codex_rollout::state_db::normalize_cwd_for_state_db;
pub use codex_rollout::state_db::open_if_present;
pub use codex_rollout::state_db::persist_dynamic_tools;
pub use codex_rollout::state_db::read_repair_rollout_path;
pub use codex_rollout::state_db::reconcile_rollout;
pub use codex_rollout::state_db::touch_thread_updated_at;
pub use codex_state::LogEntry;

use crate::config::Config;

pub(crate) async fn init(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::init(config).await
}

pub async fn get_state_db(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::get_state_db(config).await
}
