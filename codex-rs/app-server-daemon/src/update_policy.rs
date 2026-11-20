//! Enforces managed daemon update policy independently of daemon preferences.

use anyhow::Context;
use anyhow::Result;
use codex_config::LoaderOverrides;
use codex_config::loader::load_managed_requirements_state;
use codex_config::loader::refresh_managed_preferences;
use codex_exec_server::LOCAL_FS;
use codex_features::Feature;

use crate::Daemon;

pub(crate) async fn allowed(daemon: &Daemon) -> Result<bool> {
    let home = daemon
        .settings_file
        .parent()
        .and_then(std::path::Path::parent)
        .context("daemon settings path has no Codex home")?;
    updates_allowed(home, LoaderOverrides::default()).await
}

pub(crate) async fn ensure_allowed(daemon: &Daemon) -> Result<()> {
    anyhow::ensure!(
        allowed(daemon).await?,
        "Daemon updates are disabled by managed policy (features.daemon_updates = false)."
    );
    Ok(())
}

async fn updates_allowed(home: &std::path::Path, overrides: LoaderOverrides) -> Result<bool> {
    refresh_managed_preferences(&overrides).await?;
    let requirements = load_managed_requirements_state(LOCAL_FS.as_ref(), home, overrides)
        .await
        .context("failed to load managed daemon update policy")?;
    Ok(requirements
        .feature_requirements
        .as_ref()
        .and_then(|features| features.entries.get(Feature::DaemonUpdates.key()))
        .copied()
        .unwrap_or(true))
}

#[cfg(test)]
#[path = "update_policy_tests.rs"]
mod tests;
