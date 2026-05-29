use codex_rollout::state_db as rollout_state_db;
pub use codex_rollout::state_db::StateDbHandle;

use crate::config::Config;

pub async fn init_state_db(config: &Config) -> Option<StateDbHandle> {
    rollout_state_db::init(config).await
}
