//! Invocation feature overrides apply only when starting a missing daemon.
//! The lifecycle lock protects their persistence against concurrent starts and updates.

use crate::Daemon;
use crate::LifecycleOutput;
use crate::ensure_supported_platform;
use anyhow::Result;
use std::collections::BTreeMap;

/// Start a missing daemon with these features, or leave a running daemon unchanged.
/// Callers must check the running server's configuration before using it.
pub async fn start_with_features(features: &BTreeMap<String, bool>) -> Result<LifecycleOutput> {
    ensure_supported_platform()?;
    #[cfg(windows)]
    crate::backend::windows::ensure_not_elevated()?;
    let daemon = Daemon::from_environment()?;
    let _operation_lock = daemon.acquire_operation_lock().await?;
    let selected = daemon.current_installation()?;
    Box::pin(selected.start(features)).await
}
