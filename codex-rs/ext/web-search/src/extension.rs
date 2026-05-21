use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::ThreadId;
use codex_thread_store::ThreadStore;

use crate::tool::WebSearchTool;

#[derive(Clone)]
struct WebSearchExtension {
    auth_manager: Arc<AuthManager>,
    thread_store: Arc<dyn ThreadStore>,
}

#[derive(Clone)]
struct WebSearchExtensionConfig {
    enabled: bool,
    provider: ModelProviderInfo,
}

impl WebSearchExtensionConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.features.enabled(Feature::StandaloneWebSearch)
                && config.model_provider.is_openai(),
            provider: config.model_provider.clone(),
        }
    }
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for WebSearchExtension {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        input
            .thread_store
            .insert(WebSearchExtensionConfig::from_config(input.config));
    }
}

impl ConfigContributor<Config> for WebSearchExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        thread_store.insert(WebSearchExtensionConfig::from_config(new_config));
    }
}

impl ToolContributor for WebSearchExtension {
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        let Some(config) = thread_store.get::<WebSearchExtensionConfig>() else {
            return Vec::new();
        };
        if !config.enabled {
            return Vec::new();
        }
        let Ok(thread_id) = ThreadId::from_string(thread_store.level_id()) else {
            return Vec::new();
        };

        vec![Arc::new(WebSearchTool {
            session_id: session_store.level_id().to_string(),
            thread_id,
            thread_store: Arc::clone(&self.thread_store),
            provider: create_model_provider(
                config.provider.clone(),
                Some(self.auth_manager.clone()),
            ),
        })]
    }
}

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    thread_store: Arc<dyn ThreadStore>,
) {
    let extension = Arc::new(WebSearchExtension {
        auth_manager,
        thread_store,
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ToolName;
    use codex_login::CodexAuth;
    use codex_model_provider_info::ModelProviderInfo;
    use codex_thread_store::InMemoryThreadStore;
    use pretty_assertions::assert_eq;

    use super::AuthManager;
    use super::Config;
    use super::WebSearchExtensionConfig;
    use super::install;

    #[test]
    fn installed_extension_contributes_web_run_when_enabled() {
        let mut builder = ExtensionRegistryBuilder::<Config>::new();
        install(
            &mut builder,
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy")),
            Arc::new(InMemoryThreadStore::default()),
        );
        let registry = builder.build();
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("11111111-1111-4111-8111-111111111111");
        thread_store.insert(WebSearchExtensionConfig {
            enabled: true,
            provider: ModelProviderInfo::create_openai_provider(/*base_url*/ None),
        });

        let tool_names = registry
            .tool_contributors()
            .iter()
            .flat_map(|contributor| contributor.tools(&session_store, &thread_store))
            .map(|tool| tool.tool_name())
            .collect::<Vec<_>>();

        assert_eq!(tool_names, vec![ToolName::namespaced("web", "run")]);
    }
}
