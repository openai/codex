//! Helpers for locating alarm sidecars and surfacing scheduler-only threads in
//! session listings.

use std::path::Path;
use std::path::PathBuf;

const ALARM_THREAD_PREVIEW: &str = "(alarm configured)";

pub(crate) fn alarm_sidecar_path_for_rollout(rollout_path: &Path) -> PathBuf {
    PathBuf::from(format!("{}.alarms.json", rollout_path.display()))
}

pub(crate) async fn thread_preview_from_alarm_sidecar(rollout_path: &Path) -> Option<String> {
    let sidecar_path = alarm_sidecar_path_for_rollout(rollout_path);
    tokio::fs::try_exists(sidecar_path)
        .await
        .ok()
        .filter(|exists| *exists)
        .map(|_| ALARM_THREAD_PREVIEW.to_string())
}
