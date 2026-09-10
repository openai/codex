//! Activates an admitted task's plugin selection without changing pending settings.

use std::sync::Arc;

use super::Session;
use super::turn_context::TurnContext;

impl Session {
    pub(crate) async fn activate_plugin_selection(&self, turn_context: &TurnContext) {
        let config = {
            let mut state = self.state.lock().await;
            if state.active_disabled_plugin_ids != turn_context.disabled_plugin_ids {
                state.active_disabled_plugin_ids = turn_context.disabled_plugin_ids.clone();
                self.mark_mcp_runtime_dirty();
            }
            Arc::clone(&state.session_configuration.original_config_do_not_use)
        };
        let plugin_outcome = self
            .services
            .plugins_manager
            .plugins_for_config(&config.plugins_config_input())
            .await
            .without_plugins(&turn_context.disabled_plugin_ids);
        // Cache changes from another process do not notify this session's hook runtime.
        if !self.hooks().matches_plugin_hooks(
            plugin_outcome.iter_effective_plugin_hook_sources(),
            plugin_outcome.iter_effective_plugin_hook_warnings(),
        ) {
            self.refresh_hooks(config).await;
        }
    }
}
