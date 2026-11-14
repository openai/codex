//! Refreshes cloud skills explicitly or through step freshness checks.

use codex_extension_api::CatalogRefreshInput as StepCatalogRefreshInput;
use codex_extension_api::CloudCatalogRefreshInput;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::TurnLifecycleContributor;
use std::sync::Arc;

use super::SkillsExtension;
use crate::provider::SkillListQuery;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;

struct CloudDiscoveryWarningEmitted;

fn cloud_catalog_query(
    input: CloudCatalogRefreshInput<'_>,
    turn_id: Option<&str>,
) -> Option<(Arc<SkillsThreadState>, SkillListQuery<'static>)> {
    let state = input.thread_store.get::<SkillsThreadState>()?;
    if !state.cloud_skill_enabled() {
        return None;
    }
    let query = SkillListQuery {
        turn_id: turn_id.map(str::to_owned),
        executor_roots: Vec::new(),
        host_snapshot: None,
        include_host_skills: false,
        include_bundled_skills: state.config().bundled_skills_enabled,
        include_cloud_skills: true,
        mcp_resources: input
            .session_store
            .get::<SkillsSessionState>()
            .and_then(|state| state.mcp_resources.clone()),
        executor_capability_discovery: None,
        executor_capability_discovery_v2: None,
    };
    Some((state, query))
}

impl<C: Send + Sync + 'static> TurnLifecycleContributor for SkillsExtension<C> {
    fn refresh_catalog_for_step<'a>(
        &'a self,
        input: StepCatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, bool> {
        Box::pin(async move {
            let Some((state, query)) = cloud_catalog_query(input.into(), Some(input.turn_id))
            else {
                return false;
            };
            if let Err(error) = state
                .refresh_cloud_catalog_for_step(&self.providers, query)
                .await
                && input
                    .turn_store
                    .insert(CloudDiscoveryWarningEmitted)
                    .is_none()
            {
                let message = format!(
                    "Cloud skill discovery failed; retaining any catalog from the same cloud auth scope: {error}"
                );
                self.emit_warning(input.thread_store.level_id(), Some(input.turn_id), message);
            }
            false
        })
    }

    fn refresh_cloud_catalog<'a>(
        &'a self,
        input: CloudCatalogRefreshInput<'a>,
    ) -> ExtensionFuture<'a, Result<(), String>> {
        Box::pin(async move {
            let Some((state, query)) = cloud_catalog_query(input, /*turn_id*/ None) else {
                return Ok(());
            };
            state
                .refresh_cloud_catalog(&self.providers, query)
                .await
                .map_err(|error| error.to_string())
        })
    }
}
