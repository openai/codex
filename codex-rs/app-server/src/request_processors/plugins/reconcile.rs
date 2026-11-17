//! Exposes shared installed-plugin reconciliation with hook lifecycle updates.

use super::*;
use crate::effective_plugin_change::trust_materialized_plugin_hooks;
use crate::request_serialization::RequestSerializationAccess;
use crate::request_serialization::RequestSerializationQueueKey;
use crate::request_serialization::RequestSerializationQueues;
use codex_app_server_protocol::PluginReconcileChangedPlugin;
use codex_app_server_protocol::PluginReconcileParams;
use codex_app_server_protocol::PluginReconcileResponse;
use codex_core_plugins::remote::RemotePluginShareDiscoverability;

impl PluginRequestProcessor {
    #[tracing::instrument(level = "debug", skip_all, fields(reason = ?params.reason))]
    pub(crate) async fn plugin_reconcile(
        &self,
        params: PluginReconcileParams,
        config_processor: ConfigRequestProcessor,
        request_serialization_queues: &RequestSerializationQueues,
    ) -> Result<Option<ClientResponsePayload>, JSONRPCErrorError> {
        let config = self.load_latest_config(/*fallback_cwd*/ None).await?;
        let plugins_input = config.plugins_config_input();
        let auth = self.auth_manager.auth().await;
        if !plugins_input.plugins_enabled
            || !auth.as_ref().is_some_and(CodexAuth::uses_codex_backend)
        {
            return Ok(Some(PluginReconcileResponse::default().into()));
        }

        // Match background bundle sync: remote_plugin controls catalog visibility, not sync.
        // The shared reconciler owns synchronization, auth checks, and cache publication.
        let outcome = self
            .thread_manager
            .plugins_manager()
            .reconcile_remote_installed_plugins_with_notify(
                &plugins_input,
                auth.as_ref(),
                Some(self.effective_plugins_changed_callback()),
            )
            .await
            .map_err(|err| {
                internal_error(format!(
                    "failed to reconcile remote installed plugins: {err}"
                ))
            })?;

        let hooks_changed = outcome
            .changed_plugins
            .iter()
            .any(|plugin| plugin.capabilities.has_hooks);
        // Preserve materialization-owned trust: unchanged background passes cannot recover it.
        // Serialize hook updates with config writes, after releasing the bundle writer gate.
        // Changes without hooks or eligible materializations still skip this queue.
        if hooks_changed
            || outcome.materialized_remote_plugins.iter().any(|plugin| {
                plugin.scope == RemotePluginScope::Workspace
                    && plugin.discoverability == Some(RemotePluginShareDiscoverability::Listed)
            })
        {
            let materializations = outcome.materialized_remote_plugins.clone();
            let processor = self.clone();
            let (complete, completion) = tokio::sync::oneshot::channel();
            request_serialization_queues
                .enqueue_background(
                    RequestSerializationQueueKey::Global("config"),
                    RequestSerializationAccess::Exclusive,
                    async move {
                        let result = trust_materialized_plugin_hooks(
                            materializations,
                            &processor.auth_manager,
                            &processor.thread_manager,
                            &processor.config_manager,
                            &config_processor,
                        )
                        .await;
                        // Removals and disablements have no trust write to rebuild loaded hooks.
                        // Rebuild after the trust attempt, even if it failed, to drop stale hooks.
                        if hooks_changed {
                            processor.thread_manager.refresh_hook_runtimes().await;
                        }
                        let _ = complete.send(result);
                    },
                )
                .await;
            completion
                .await
                .map_err(|err| {
                    internal_error(format!("plugin hook trust update was cancelled: {err}"))
                })?
                .map_err(|err| {
                    internal_error(format!("failed to trust materialized plugin hooks: {err}"))
                })?;
        }

        Ok(Some(
            PluginReconcileResponse {
                changed_plugins: outcome
                    .changed_plugins
                    .into_iter()
                    .map(|plugin| PluginReconcileChangedPlugin {
                        id: plugin.plugin_id,
                        has_mcps: plugin.capabilities.has_mcps,
                        has_apps: plugin.capabilities.has_apps,
                        has_hooks: plugin.capabilities.has_hooks,
                        has_skills: plugin.capabilities.has_skills,
                    })
                    .collect(),
                failed_remote_plugin_ids: outcome.failed_remote_plugin_ids,
                failed_materialization_remote_plugin_ids: outcome
                    .failed_materialization_remote_plugin_ids,
            }
            .into(),
        ))
    }
}
