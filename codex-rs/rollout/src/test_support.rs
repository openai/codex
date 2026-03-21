use std::path::Path;

use crate::RolloutConfig;

pub(crate) fn test_rollout_config(codex_home: &Path) -> RolloutConfig {
    RolloutConfig::new(
        codex_home.to_path_buf(),
        codex_home.to_path_buf(),
        codex_home.to_path_buf(),
        "test-provider".to_string(),
        false,
    )
}
