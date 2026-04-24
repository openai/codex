use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::config::Config;
use crate::plugins::PluginsManager;
use codex_core_plugins::startup_sync::has_local_curated_plugins_snapshot;
use codex_login::AuthManager;
use tracing::info;
use tracing::warn;

const STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE: &str = ".tmp/app-server-remote-plugin-sync-v1";
const STARTUP_REMOTE_PLUGIN_SYNC_PREREQUISITE_TIMEOUT: Duration = Duration::from_secs(10);

pub(crate) fn start_startup_remote_plugin_sync_once(
    manager: Arc<PluginsManager>,
    codex_home: PathBuf,
    config: Config,
    auth_manager: Arc<AuthManager>,
) {
    let marker_path = startup_remote_plugin_sync_marker_path(codex_home.as_path());
    if marker_path.is_file() {
        return;
    }

    tokio::spawn(async move {
        if marker_path.is_file() {
            return;
        }

        if !wait_for_startup_remote_plugin_sync_prerequisites(codex_home.as_path()).await {
            warn!(
                codex_home = %codex_home.display(),
                "skipping startup remote plugin sync because curated marketplace is not ready"
            );
            return;
        }

        let auth = auth_manager.auth().await;
        match manager
            .sync_plugins_from_remote(&config, auth.as_ref(), /*additive_only*/ true)
            .await
        {
            Ok(sync_result) => {
                info!(
                    installed_plugin_ids = ?sync_result.installed_plugin_ids,
                    enabled_plugin_ids = ?sync_result.enabled_plugin_ids,
                    disabled_plugin_ids = ?sync_result.disabled_plugin_ids,
                    uninstalled_plugin_ids = ?sync_result.uninstalled_plugin_ids,
                    "completed startup remote plugin sync"
                );
                if let Err(err) =
                    write_startup_remote_plugin_sync_marker(codex_home.as_path()).await
                {
                    warn!(
                        error = %err,
                        path = %marker_path.display(),
                        "failed to persist startup remote plugin sync marker"
                    );
                }
            }
            Err(err) => {
                warn!(
                    error = %err,
                    "startup remote plugin sync failed; will retry on next app-server start"
                );
            }
        }
    });
}

fn startup_remote_plugin_sync_marker_path(codex_home: &Path) -> PathBuf {
    codex_home.join(STARTUP_REMOTE_PLUGIN_SYNC_MARKER_FILE)
}

async fn wait_for_startup_remote_plugin_sync_prerequisites(codex_home: &Path) -> bool {
    let deadline = tokio::time::Instant::now() + STARTUP_REMOTE_PLUGIN_SYNC_PREREQUISITE_TIMEOUT;
    loop {
        if has_local_curated_plugins_snapshot(codex_home) {
            return true;
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

async fn write_startup_remote_plugin_sync_marker(codex_home: &Path) -> std::io::Result<()> {
    let marker_path = startup_remote_plugin_sync_marker_path(codex_home);
    if let Some(parent) = marker_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(marker_path, b"ok\n").await
}

#[cfg(test)]
#[path = "startup_sync_tests.rs"]
mod tests;
