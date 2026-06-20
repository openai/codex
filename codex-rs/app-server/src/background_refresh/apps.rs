use std::sync::Arc;
use std::time::Duration;

use codex_chatgpt::connectors;
use codex_core::McpManager;
use codex_exec_server::EnvironmentManager;
use tracing::warn;

use super::PeriodicRefreshWorker;
use super::periodic;
use super::periodic::InitialRefresh;
use super::periodic::RefreshControl;
use crate::config_manager::ConfigManager;

const APPS_REFRESH_INTERVAL: Duration = Duration::from_secs(5 * 60);

pub(crate) fn spawn_apps_refresh_worker(
    config_manager: ConfigManager,
    environment_manager: &Arc<EnvironmentManager>,
    mcp_manager: &Arc<McpManager>,
) -> PeriodicRefreshWorker {
    spawn_with_interval(
        config_manager,
        environment_manager,
        mcp_manager,
        APPS_REFRESH_INTERVAL,
    )
}

fn spawn_with_interval(
    config_manager: ConfigManager,
    environment_manager: &Arc<EnvironmentManager>,
    mcp_manager: &Arc<McpManager>,
    refresh_interval: Duration,
) -> PeriodicRefreshWorker {
    let environment_manager = Arc::downgrade(environment_manager);
    let mcp_manager = Arc::downgrade(mcp_manager);
    // app/list populates this cache on demand. Delay the background pass so startup RPCs do not
    // race a second force-refresh of the same Codex Apps MCP server.
    periodic::spawn(refresh_interval, InitialRefresh::AfterInterval, move || {
        let config_manager = config_manager.clone();
        let environment_manager = environment_manager.clone();
        let mcp_manager = mcp_manager.clone();
        async move {
            let Some(environment_manager) = environment_manager.upgrade() else {
                return RefreshControl::Stop;
            };
            let Some(mcp_manager) = mcp_manager.upgrade() else {
                return RefreshControl::Stop;
            };
            let config = match config_manager
                .load_latest_config(/*fallback_cwd*/ None)
                .await
            {
                Ok(config) => config,
                Err(err) => {
                    warn!(error = %err, "failed to reload config for periodic apps refresh");
                    return RefreshControl::Continue;
                }
            };
            if let Err(err) =
                connectors::list_accessible_connectors_from_mcp_tools_with_mcp_manager(
                    &config,
                    /*force_refetch*/ true,
                    environment_manager,
                    mcp_manager,
                )
                .await
            {
                warn!(error = %err, "failed to refresh Codex Apps tools cache");
            }
            RefreshControl::Continue
        }
    })
}

#[cfg(test)]
#[path = "apps_tests.rs"]
mod tests;
