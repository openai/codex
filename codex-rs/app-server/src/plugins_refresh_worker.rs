use std::sync::Arc;
use std::time::Duration;

use codex_core_plugins::PluginsManager;
use codex_login::AuthManager;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::config_manager::ConfigManager;

const PLUGINS_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

#[derive(Debug)]
pub(crate) struct PluginsRefreshWorker {
    shutdown: CancellationToken,
    _task: JoinHandle<()>,
}

impl PluginsRefreshWorker {
    pub(crate) fn shutdown(&self) {
        self.shutdown.cancel();
    }
}

impl Drop for PluginsRefreshWorker {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub(crate) fn spawn(
    plugins_manager: &Arc<PluginsManager>,
    auth_manager: &Arc<AuthManager>,
    config_manager: ConfigManager,
    on_effective_plugins_changed: Arc<dyn Fn() + Send + Sync>,
) -> PluginsRefreshWorker {
    spawn_with_interval(
        plugins_manager,
        auth_manager,
        config_manager,
        on_effective_plugins_changed,
        PLUGINS_REFRESH_INTERVAL,
    )
}

fn spawn_with_interval(
    plugins_manager: &Arc<PluginsManager>,
    auth_manager: &Arc<AuthManager>,
    config_manager: ConfigManager,
    on_effective_plugins_changed: Arc<dyn Fn() + Send + Sync>,
    refresh_interval: Duration,
) -> PluginsRefreshWorker {
    let plugins_manager = Arc::downgrade(plugins_manager);
    let auth_manager = Arc::downgrade(auth_manager);
    let shutdown = CancellationToken::new();
    let worker_shutdown = shutdown.clone();
    let task = tokio::spawn(async move {
        loop {
            // Plugin startup tasks perform the initial refresh. Wait before the first periodic
            // pass so app-server startup does not issue duplicate remote requests.
            tokio::select! {
                _ = worker_shutdown.cancelled() => break,
                _ = tokio::time::sleep(refresh_interval) => {}
            }

            let Some(plugins_manager) = plugins_manager.upgrade() else {
                break;
            };
            let Some(auth_manager) = auth_manager.upgrade() else {
                break;
            };
            let config = match config_manager
                .load_latest_config(/*fallback_cwd*/ None)
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    warn!(
                        error = %err,
                        "failed to reload config for periodic plugin refresh"
                    );
                    continue;
                }
            };
            let auth = auth_manager.auth().await;
            if worker_shutdown.is_cancelled() {
                break;
            }
            let plugins_config = config.plugins_config_input();
            plugins_manager.maybe_start_remote_plugin_caches_refresh(
                &plugins_config,
                auth.clone(),
                Some(Arc::clone(&on_effective_plugins_changed)),
            );
            plugins_manager.maybe_start_remote_installed_plugin_bundle_sync(
                &plugins_config,
                auth,
                Some(Arc::clone(&on_effective_plugins_changed)),
            );
        }
    });
    PluginsRefreshWorker {
        shutdown,
        _task: task,
    }
}

#[cfg(test)]
#[path = "plugins_refresh_worker_tests.rs"]
mod tests;
