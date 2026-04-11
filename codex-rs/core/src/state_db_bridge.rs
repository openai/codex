use codex_rollout::state_db as rollout_state_db;
pub use codex_rollout::state_db::StateDbHandle;

use crate::config::Config;

pub async fn get_state_db(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::get_state_db(config).await
}
