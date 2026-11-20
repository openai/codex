#![cfg(test)]

use super::*;

#[tokio::test]
async fn managed_policy_controls_daemon_updates_and_rejects_invalid_policy() -> Result<()> {
    let home = tempfile::tempdir()?;
    let path = home.path().join("requirements.toml");
    let mut overrides = LoaderOverrides::without_managed_config_for_tests();
    overrides.system_requirements_path = Some(path.clone());
    assert!(updates_allowed(home.path(), overrides.clone()).await?);
    tokio::fs::write(&path, "[features]\ndaemon_updates = false\n").await?;
    assert!(!updates_allowed(home.path(), overrides.clone()).await?);
    tokio::fs::write(&path, "[features]\ndaemon_updates = 'invalid'\n").await?;
    assert!(updates_allowed(home.path(), overrides).await.is_err());
    Ok(())
}
