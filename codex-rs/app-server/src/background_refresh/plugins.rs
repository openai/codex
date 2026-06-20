use std::sync::Arc;
use std::time::Duration;

use codex_core_plugins::PluginsManager;
use codex_login::AuthManager;
use tracing::warn;

use super::PeriodicRefreshWorker;
use super::periodic;
use super::periodic::InitialRefresh;
use super::periodic::RefreshControl;
use crate::config_manager::ConfigManager;

const PLUGINS_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(crate) fn spawn_plugins_refresh_worker(
    plugins_manager: &Arc<PluginsManager>,
    auth_manager: &Arc<AuthManager>,
    config_manager: ConfigManager,
    on_effective_plugins_changed: Arc<dyn Fn() + Send + Sync>,
) -> PeriodicRefreshWorker {
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
) -> PeriodicRefreshWorker {
    let plugins_manager = Arc::downgrade(plugins_manager);
    let auth_manager = Arc::downgrade(auth_manager);
    periodic::spawn(refresh_interval, InitialRefresh::Immediate, move || {
        let plugins_manager = plugins_manager.clone();
        let auth_manager = auth_manager.clone();
        let config_manager = config_manager.clone();
        let on_effective_plugins_changed = Arc::clone(&on_effective_plugins_changed);
        async move {
            let Some(plugins_manager) = plugins_manager.upgrade() else {
                return RefreshControl::Stop;
            };
            let Some(auth_manager) = auth_manager.upgrade() else {
                return RefreshControl::Stop;
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
                    return RefreshControl::Continue;
                }
            };
            let auth = auth_manager.auth().await;
            let plugins_config = config.plugins_config_input();
            plugins_manager.maybe_start_remote_installed_plugins_cache_refresh(
                &plugins_config,
                auth.clone(),
                Some(Arc::clone(&on_effective_plugins_changed)),
            );
            plugins_manager.maybe_start_remote_installed_plugin_bundle_sync(
                &plugins_config,
                auth,
                Some(on_effective_plugins_changed),
            );
            RefreshControl::Continue
        }
    })
}

#[cfg(test)]
#[path = "plugins_tests.rs"]
mod tests;
