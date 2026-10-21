//! Refreshes cloud skills at turn start and after auth changes.

use codex_extension_api::CatalogRefreshInput;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;

use super::SkillsExtension;
use crate::provider::SkillListQuery;
use crate::state::LatestCloudSkillsAuth;
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
            resolved_executor_roots: Vec::new(),
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
        if let Err(error) = state
            .refresh_cloud_catalog(&self.providers, query, input.turn_store)
            .await
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
    fn turn_start_phase(&self, thread_store: &ExtensionData) -> TurnStartPhase {
        if thread_store
            .get::<SkillsThreadState>()
            .is_some_and(|state| state.cloud_skill_enabled())
        {
            TurnStartPhase::RegularTaskStart
        } else {
            TurnStartPhase::BeforeTaskRegistration
        }
    }

    fn requires_mcp_runtime(&self, thread_store: &ExtensionData) -> bool {
        self.turn_start_phase(thread_store) == TurnStartPhase::RegularTaskStart
    }

    fn on_turn_start<'a>(&'a self, input: TurnStartInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.turn_store.remove::<LatestCloudSkillsAuth>();
            self.refresh_cloud_catalog(CatalogRefreshInput {
                turn_id: input.turn_id,
                session_store: input.session_store,
                thread_store: input.thread_store,
                turn_store: input.turn_store,
            })
            .await;
        })
    }

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
