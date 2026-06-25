use std::sync::Arc;

use codex_exec_server::LOCAL_ENVIRONMENT_ID;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_mcp::McpResourceClient;

use crate::sources::orchestrator_skill_sources;
use crate::state::SkillsThreadState;
use crate::tools::skill_tools;

struct SkillsExtension<C> {
    orchestrator_skills_enabled: Arc<dyn Fn(&C) -> bool + Send + Sync>,
}

impl<C> ThreadLifecycleContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let orchestrator_skills_available = !input
                .environments
                .iter()
                .any(|environment| environment.environment_id == LOCAL_ENVIRONMENT_ID);
            let thread_state = input.thread_store.get_or_init(|| {
                SkillsThreadState::new(
                    (self.orchestrator_skills_enabled)(input.config),
                    orchestrator_skills_available,
                )
            });
            thread_state
                .set_orchestrator_skills_enabled((self.orchestrator_skills_enabled)(input.config));
            input.thread_store.insert(orchestrator_skill_sources(
                thread_state,
                input.session_store.get::<McpResourceClient>(),
            ));
        })
    }
}

impl<C> ConfigContributor<C> for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn on_config_changed(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &C,
        new_config: &C,
    ) {
        let enabled = (self.orchestrator_skills_enabled)(new_config);
        if let Some(state) = thread_store.get::<SkillsThreadState>() {
            state.set_orchestrator_skills_enabled(enabled);
        } else {
            let orchestrator_skills_available = true;
            let thread_state = thread_store
                .get_or_init(|| SkillsThreadState::new(enabled, orchestrator_skills_available));
            thread_store.insert(orchestrator_skill_sources(
                thread_state,
                session_store.get::<McpResourceClient>(),
            ));
        }
    }
}

impl<C> ToolContributor for SkillsExtension<C>
where
    C: Send + Sync + 'static,
{
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(thread_state) = thread_store.get::<SkillsThreadState>() else {
            return Vec::new();
        };
        if !thread_state.orchestrator_skills_enabled() {
            return Vec::new();
        }

        skill_tools(session_store.get::<McpResourceClient>(), thread_state)
    }
}

pub fn install<C>(
    registry: &mut ExtensionRegistryBuilder<C>,
    orchestrator_skills_enabled: impl Fn(&C) -> bool + Send + Sync + 'static,
) where
    C: Send + Sync + 'static,
{
    let extension = Arc::new(SkillsExtension {
        orchestrator_skills_enabled: Arc::new(orchestrator_skills_enabled),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}
