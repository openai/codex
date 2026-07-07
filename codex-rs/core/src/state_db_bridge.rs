use codex_rollout::state_db as rollout_state_db;
pub use codex_rollout::state_db::StateDbHandle;

use crate::config::Config;

pub async fn init_state_db(config: &Config) -> Option<StateDbHandle> {
    if !config.features.enabled(codex_features::Feature::Sqlite) {
        return None;
    }
    rollout_state_db::init(config).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ConfigBuilder;
    use codex_features::Feature;
    use tempfile::TempDir;

    #[tokio::test]
    async fn sqlite_disabled_skips_state_db_initialization() -> anyhow::Result<()> {
        let codex_home = TempDir::new()?;
        let mut config = ConfigBuilder::default()
            .codex_home(codex_home.path().to_path_buf())
            .build()
            .await?;
        config.features.disable(Feature::Sqlite)?;

        assert!(init_state_db(&config).await.is_none());
        for db in codex_state::runtime_db_paths(config.sqlite_home.as_path()) {
            assert!(!db.path.exists(), "{} should not exist", db.label);
        }
        Ok(())
    }
}
