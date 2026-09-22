//! Invocation overrides apply on fresh starts or explicitly confirmed managed restarts.
//! The lifecycle lock serializes persistence against other clients and updates.

use crate::Daemon;
use crate::LifecycleOutput;
use crate::ensure_supported_platform;
use anyhow::Result;
use anyhow::ensure;
use std::collections::BTreeMap;

/// Start a missing daemon with these features, preserving other saved overrides,
/// or leave a running daemon unchanged.
/// Callers must check the running server's configuration before using it.
pub async fn start_with_features(features: &BTreeMap<String, bool>) -> Result<LifecycleOutput> {
    ensure_supported_platform()?;
    #[cfg(windows)]
    crate::backend::windows::ensure_not_elevated()?;
    let daemon = Daemon::from_environment()?;
    let _operation_lock = daemon.acquire_operation_lock().await?;
    let selected = daemon.current_installation()?;
    let mut overrides = selected.load_settings().await?.feature_overrides;
    overrides.extend(features.clone());
    Box::pin(selected.start(&overrides)).await
}

/// Apply confirmed feature settings to an existing managed daemon and restart it.
/// Callers must obtain consent for changes to shared services and interrupted work,
/// then verify effective server compatibility after this operation completes.
pub async fn restart_with_features(features: &BTreeMap<String, bool>) -> Result<LifecycleOutput> {
    ensure_supported_platform()?;
    #[cfg(windows)]
    crate::backend::windows::ensure_not_elevated()?;
    let daemon = Daemon::from_environment()?;
    let _operation_lock = daemon.acquire_operation_lock().await?;
    let selected = daemon.current_installation()?;
    Box::pin(selected.restart_with_features_locked(features)).await
}

impl Daemon {
    pub(super) async fn restart(&self) -> Result<LifecycleOutput> {
        self.restart_with_settings(self.load_settings().await?)
            .await
    }

    pub(super) async fn restart_with_features_locked(
        &self,
        features: &BTreeMap<String, bool>,
    ) -> Result<LifecycleOutput> {
        let mut settings = self.load_settings().await?;
        ensure!(
            self.running_backend_instance(&settings).await?.is_some(),
            "no running managed daemon; rerun the command to check the current server"
        );
        let previous = settings.feature_overrides.clone();
        settings.feature_overrides.extend(features.clone());
        if settings.feature_overrides == previous {
            return self.start(&previous).await;
        }
        self.restart_with_settings(settings).await
    }
}
