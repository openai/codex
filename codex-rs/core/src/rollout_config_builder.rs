use std::path::PathBuf;

use crate::config::Config;
use codex_rollout::RolloutConfig;

pub fn rollout_config(config: &Config) -> RolloutConfig {
    rollout_config_with_cwd(config, config.cwd.clone())
}

pub fn rollout_config_with_cwd(config: &Config, cwd: PathBuf) -> RolloutConfig {
    RolloutConfig::new(
        config.codex_home.clone(),
        config.sqlite_home.clone(),
        cwd,
        config.model_provider_id.clone(),
        config.memories.generate_memories,
    )
}
