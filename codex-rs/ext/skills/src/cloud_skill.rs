//! Attempts cloud discovery once during cancellable regular-turn startup.
//! Snapshot readers never discover or retry. Failures may retain a same-auth-scope
//! catalog; missing or invalidated catalogs remain empty until the next turn.

use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::TurnLifecycleContributor;
use codex_extension_api::TurnStartInput;
use codex_extension_api::TurnStartPhase;

use super::SkillsExtension;
use crate::provider::SkillListQuery;
use crate::state::SkillsSessionState;
use crate::state::SkillsThreadState;

impl<C: Send + Sync + 'static> TurnLifecycleContributor for SkillsExtension<C> {
    fn turn_start_phase(&self, thread_store: &ExtensionData) -> TurnStartPhase {
        if self.providers.has_cloud_provider()
            && thread_store
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
            let Some(state) = input.thread_store.get::<SkillsThreadState>() else {
                return;
            };
            if !state.cloud_skill_enabled() || !self.providers.has_cloud_provider() {
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
            };
            if let Err(error) = state.refresh_cloud_catalog(&self.providers, query).await {
                self.emit_warning(
                    input.thread_store.level_id(),
                    Some(input.turn_id),
                    format!(
                        "Cloud skill discovery failed; retaining any catalog from the same cloud auth scope: {error}"
                    ),
                );
            }
        })
    }
}
