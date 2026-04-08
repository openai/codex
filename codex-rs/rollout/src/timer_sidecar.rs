//! Helpers for locating timer sidecars and surfacing scheduler-only threads in
//! session listings.

use std::path::Path;
use std::path::PathBuf;

const TIMER_THREAD_PREVIEW: &str = "(timer configured)";

pub(crate) fn timer_sidecar_path_for_rollout(rollout_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.timers.json", rollout_path.display()))
}

pub(crate) async fn thread_preview_from_timer_sidecar(rollout_path: &Path) -> Option<String> {
    let sidecar_path = timer_sidecar_path_for_rollout(rollout_path);
    tokio::fs::try_exists(sidecar_path)
        .await
        .ok()
        .filter(|exists| *exists)
        .map(|_| TIMER_THREAD_PREVIEW.to_string())
}
