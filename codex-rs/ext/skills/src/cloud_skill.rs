//! Refreshes cloud skills before steps when their auth or Apps resources change.

use codex_extension_api::CatalogRefreshInput;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::TurnLifecycleContributor;

use super::SkillsExtension;
use crate::provider::SkillListQuery;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;

struct CloudDiscoveryWarningEmitted;

impl<C: Send + Sync + 'static> SkillsExtension<C> {
    async fn refresh_cloud_catalog(&self, input: CatalogRefreshInput<'_>) {
        let Some(state) = input.thread_store.get::<SkillsThreadState>() else {
            return;
        };
        if !state.cloud_skill_enabled() {
            return;
        }
        let config = state.config();
        let query = SkillListQuery {
            turn_id: input.turn_id.to_string(),
            executor_roots: Vec::new(),
            host_snapshot: None,
            include_host_skills: false,
            include_bundled_skills: config.bundled_skills_enabled,
            include_cloud_skills: true,
            mcp_resources: input
                .session_store
                .get::<SkillsSessionState>()
                .and_then(|state| state.mcp_resources.clone()),
            executor_capability_discovery: None,
            executor_capability_discovery_v2: None,
        };
        if let Err(error) = state.refresh_cloud_catalog(&self.providers, query).await
            && input
                .turn_store
                .insert(CloudDiscoveryWarningEmitted)
                .is_none()
        {
            self.emit_warning(
                input.thread_store.level_id(),
                Some(input.turn_id),
                format!(
                    "Cloud skill discovery failed; retaining any catalog from the same cloud auth scope: {error}"
                ),
            );
        }
    }
}

impl<C: Send + Sync + 'static> TurnLifecycleContributor for SkillsExtension<C> {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        input: CatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, bool> {
        Box::pin(async move {
            self.refresh_cloud_catalog(input).await;
            false
        })
    }
}
