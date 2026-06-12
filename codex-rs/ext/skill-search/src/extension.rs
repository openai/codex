use std::sync::Arc;

use codex_core::config::Config;
use codex_core_skills::HostLoadedSkills;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_features::Feature;

use crate::tool::SkillSearchTool;

const SKILL_SEARCH_GUIDANCE: &str = "## Skills\nSkills are local instruction bundles stored in `SKILL.md` files. Use `skill_search` when a task may benefit from one, open the returned path before following it, and use only the smallest relevant set for the current turn. If no result fits, continue without a skill.";

#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct SkillSearchExtension;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SkillSearchExtensionConfig {
    pub(crate) enabled: bool,
}

impl SkillSearchExtensionConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.features.enabled(Feature::SkillSearchTool),
        }
    }
}

impl ContextContributor for SkillSearchExtension {
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(async move {
            let Some(config) = thread_store.get::<SkillSearchExtensionConfig>() else {
                return Vec::new();
            };
            if !config.enabled {
                return Vec::new();
            }

            vec![PromptFragment::developer_policy(SKILL_SEARCH_GUIDANCE)]
        })
    }
}

impl ThreadLifecycleContributor<Config> for SkillSearchExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> codex_extension_api::ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input
                .thread_store
                .insert(SkillSearchExtensionConfig::from_config(input.config));
        })
    }
}

impl ConfigContributor<Config> for SkillSearchExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        thread_store.insert(SkillSearchExtensionConfig::from_config(new_config));
    }
}

impl ToolContributor for SkillSearchExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        turn_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(config) = thread_store.get::<SkillSearchExtensionConfig>() else {
            return Vec::new();
        };
        if !config.enabled {
            return Vec::new();
        }

        let skills = turn_store
            .get::<HostLoadedSkills>()
            .map_or_else(Vec::new, |skills| {
                skills.outcome().allowed_skills_for_implicit_invocation()
            });
        let tool = turn_store.get_or_init(|| SkillSearchTool::new(skills));
        vec![tool]
    }
}

/// Installs the skills context contributor and skill_search tool.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    let extension = Arc::new(SkillSearchExtension);
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.tool_contributor(extension);
}

#[cfg(test)]
#[path = "extension_tests.rs"]
mod tests;
