use std::sync::Arc;

use codex_core::config::Config;
use codex_core::skills::SkillLoadOutcome;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
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
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
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
    fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        input
            .thread_store
            .insert(SkillSearchExtensionConfig::from_config(input.config));
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
            .get::<SkillLoadOutcome>()
            .map_or_else(Vec::new, |outcome| {
                outcome.allowed_skills_for_implicit_invocation()
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
mod tests {
    use codex_extension_api::PromptSlot;
    use codex_extension_api::ToolContributor;
    use codex_extension_api::ToolName;
    use pretty_assertions::assert_eq;

    use super::*;
    use crate::tool::SKILL_SEARCH_TOOL_NAME;

    #[tokio::test]
    async fn prompt_contribution_is_gated_by_feature_config() {
        let extension = SkillSearchExtension;
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("thread");

        assert!(
            extension
                .contribute(&session_store, &thread_store)
                .await
                .is_empty()
        );

        thread_store.insert(SkillSearchExtensionConfig { enabled: true });
        let fragments = extension.contribute(&session_store, &thread_store).await;

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].slot(), PromptSlot::DeveloperPolicy);
        assert!(fragments[0].text().contains(SKILL_SEARCH_TOOL_NAME));
    }

    #[test]
    fn tool_contribution_is_gated_by_feature_config() {
        let extension = SkillSearchExtension;
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("thread");
        let turn_store = ExtensionData::new("turn");

        assert!(
            extension
                .tools(&session_store, &thread_store, &turn_store)
                .is_empty()
        );

        thread_store.insert(SkillSearchExtensionConfig { enabled: true });
        let tool_names = extension
            .tools(&session_store, &thread_store, &turn_store)
            .into_iter()
            .map(|tool| tool.tool_name())
            .collect::<Vec<_>>();

        assert_eq!(tool_names, vec![ToolName::plain(SKILL_SEARCH_TOOL_NAME)]);
    }
}
