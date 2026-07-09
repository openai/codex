use std::collections::BTreeSet;
use std::sync::Arc;

use codex_app_server_protocol::ConfigBatchWriteParams;
use codex_app_server_protocol::ConfigEdit;
use codex_app_server_protocol::MergeStrategy;
use codex_core::ThreadManager;
use codex_core_plugins::EffectivePluginsChange;
use codex_core_plugins::remote::RemotePluginMaterialization;
use codex_core_plugins::remote::RemotePluginScope;
use codex_core_plugins::remote::RemotePluginShareDiscoverability;
use codex_login::AuthManager;
use serde_json::json;
use tracing::warn;

use crate::config_manager::ConfigManager;
use crate::request_processors::ConfigRequestProcessor;
use crate::request_serialization::RequestSerializationAccess;
use crate::request_serialization::RequestSerializationQueueKey;
use crate::request_serialization::RequestSerializationQueues;

/// Refresh plugin consumers and trust hooks from newly materialized Workspace + Listed bundles.
pub(crate) fn effective_plugins_changed_callback(
    auth_manager: Arc<AuthManager>,
    thread_manager: Arc<ThreadManager>,
    config_manager: ConfigManager,
    config_processor: ConfigRequestProcessor,
    request_serialization_queues: RequestSerializationQueues,
) -> Arc<dyn Fn(EffectivePluginsChange) + Send + Sync> {
    Arc::new(move |change| {
        thread_manager.plugins_manager().clear_cache();
        thread_manager.skills_service().clear_cache();

        let refresh_thread_manager = Arc::clone(&thread_manager);
        let refresh_config_manager = config_manager.clone();
        tokio::spawn(async move {
            if refresh_thread_manager.list_thread_ids().await.is_empty() {
                return;
            }
            crate::mcp_refresh::queue_best_effort_refresh(
                &refresh_thread_manager,
                &refresh_config_manager,
            )
            .await;
        });

        if change.materialized_remote_plugins.is_empty() {
            return;
        }

        let trust_auth_manager = Arc::clone(&auth_manager);
        let trust_thread_manager = Arc::clone(&thread_manager);
        let trust_config_manager = config_manager.clone();
        let trust_config_processor = config_processor.clone();
        let trust_request_serialization_queues = request_serialization_queues.clone();
        tokio::spawn(async move {
            trust_request_serialization_queues
                .enqueue_background(
                    RequestSerializationQueueKey::Global("config"),
                    RequestSerializationAccess::Exclusive,
                    async move {
                        if let Err(err) = trust_materialized_plugin_hooks(
                            change.materialized_remote_plugins,
                            &trust_auth_manager,
                            &trust_thread_manager,
                            &trust_config_manager,
                            &trust_config_processor,
                        )
                        .await
                        {
                            warn!(error = %err, "failed to trust materialized plugin hooks");
                        }
                    },
                )
                .await;
        });
    })
}

fn workspace_listed_plugin_ids(
    materializations: Vec<RemotePluginMaterialization>,
    current_account_id: &str,
) -> BTreeSet<String> {
    materializations
        .into_iter()
        .filter(|plugin| {
            plugin.scope == RemotePluginScope::Workspace
                && plugin.discoverability == Some(RemotePluginShareDiscoverability::Listed)
                && plugin.authenticated_account_id.as_deref() == Some(current_account_id)
        })
        .map(|plugin| plugin.plugin_id.as_key())
        .collect()
}

fn hook_trusted_hash_edit(hook_key: &str, current_hash: &str) -> ConfigEdit {
    let escaped_hook_key = hook_key.replace('\\', "\\\\").replace('"', "\\\"");
    ConfigEdit {
        key_path: format!(r#"hooks.state."{escaped_hook_key}".trusted_hash"#),
        value: json!(current_hash),
        merge_strategy: MergeStrategy::Replace,
    }
}

async fn trust_materialized_plugin_hooks(
    materializations: Vec<RemotePluginMaterialization>,
    auth_manager: &AuthManager,
    thread_manager: &ThreadManager,
    config_manager: &ConfigManager,
    config_processor: &ConfigRequestProcessor,
) -> Result<(), String> {
    let Some(current_account_id) = auth_manager
        .auth_cached()
        .and_then(|auth| auth.get_account_id())
    else {
        return Ok(());
    };
    let plugin_ids = workspace_listed_plugin_ids(materializations, &current_account_id);
    if plugin_ids.is_empty() {
        return Ok(());
    }
    let config = config_manager
        .load_latest_config(/*fallback_cwd*/ None)
        .await
        .map_err(|err| format!("failed to reload config: {err}"))?;
    let plugin_outcome = thread_manager
        .plugins_manager()
        .plugins_for_config(&config.plugins_config_input())
        .await;
    let hooks = codex_hooks::list_hooks(codex_hooks::HooksConfig {
        feature_enabled: true,
        bypass_hook_trust: config.bypass_hook_trust,
        config_layer_stack: Some(config.config_layer_stack),
        plugin_hook_sources: plugin_outcome.effective_plugin_hook_sources(),
        plugin_hook_load_warnings: plugin_outcome.effective_plugin_hook_warnings(),
        ..Default::default()
    });
    if !hooks.warnings.is_empty() {
        warn!(
            warnings = ?hooks.warnings,
            "hook discovery reported warnings while trusting materialized plugins"
        );
    }
    let edits = hooks
        .hooks
        .into_iter()
        .filter(|hook| {
            hook.plugin_id
                .as_ref()
                .is_some_and(|plugin_id| plugin_ids.contains(plugin_id))
        })
        .map(|hook| hook_trusted_hash_edit(&hook.key, &hook.current_hash))
        .collect::<Vec<_>>();
    if edits.is_empty() {
        return Ok(());
    }
    if auth_manager
        .auth_cached()
        .and_then(|auth| auth.get_account_id())
        .as_deref()
        != Some(current_account_id.as_str())
    {
        warn!("skipping materialized plugin hook trust after account changed");
        return Ok(());
    }

    let params = ConfigBatchWriteParams {
        edits,
        file_path: None,
        expected_version: None,
        reload_user_config: true,
    };
    config_processor
        .batch_write(params)
        .await
        .map_err(|err| format!("failed to write hook trust: {}", err.message))?;
    Ok(())
}

#[cfg(test)]
#[path = "effective_plugin_change_tests.rs"]
mod tests;
