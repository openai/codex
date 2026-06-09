use std::marker::PhantomData;
use std::sync::Arc;

use crate::backend::MemoriesBackend;
use crate::local::LocalMemoriesBackend;
use crate::prompt_source::MemoryPromptSource;
use crate::prompts::build_memory_tool_developer_instructions;
use crate::tools;
use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_otel::MetricsClient;

/// Contributes Codex memory read-path prompt context and memory read tools.
#[derive(Clone)]
pub(crate) struct MemoriesExtension<B = LocalMemoriesBackend, S = LocalMemoriesBackend> {
    metrics_client: Option<MetricsClient>,
    storage: PhantomData<fn() -> (B, S)>,
}

impl Default for MemoriesExtension {
    fn default() -> Self {
        Self::new(/*metrics_client*/ None)
    }
}

impl<B, S> MemoriesExtension<B, S> {
    pub(crate) fn new(metrics_client: Option<MetricsClient>) -> Self {
        Self {
            metrics_client,
            storage: PhantomData,
        }
    }

    fn store_thread_state(thread_store: &ExtensionData, config: &Config) {
        thread_store.insert(MemoriesExtensionConfig::from_config(config));
        thread_store.insert(MemoriesExtensionStorageDeps::local(config));
    }
}

#[derive(Clone, Debug)]
pub(crate) struct MemoriesExtensionConfig {
    pub(crate) enabled: bool,
    pub(crate) dedicated_tools: bool,
}

impl MemoriesExtensionConfig {
    fn from_config(config: &Config) -> Self {
        Self {
            enabled: config.features.enabled(Feature::MemoryTool) && config.memories.use_memories,
            dedicated_tools: config.memories.dedicated_tools,
        }
    }
}

#[derive(Clone)]
pub(crate) struct MemoriesExtensionStorageDeps<B = LocalMemoriesBackend, S = LocalMemoriesBackend> {
    pub(crate) backend: B,
    pub(crate) prompt_source: S,
}

impl MemoriesExtensionStorageDeps {
    fn local(config: &Config) -> Self {
        let backend = LocalMemoriesBackend::from_codex_home(&config.codex_home);
        Self::new(backend.clone(), backend)
    }
}

impl<B, S> MemoriesExtensionStorageDeps<B, S> {
    pub(crate) fn new(backend: B, prompt_source: S) -> Self {
        Self {
            backend,
            prompt_source,
        }
    }
}

impl<B, S> ContextContributor for MemoriesExtension<B, S>
where
    B: MemoriesBackend,
    S: MemoryPromptSource,
{
    fn contribute<'a>(
        &'a self,
        _session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = Vec<PromptFragment>> + Send + 'a>> {
        Box::pin(async move {
            let Some(config) = thread_store.get::<MemoriesExtensionConfig>() else {
                return Vec::new();
            };
            if !config.enabled {
                return Vec::new();
            }
            let Some(deps) = thread_store.get::<MemoriesExtensionStorageDeps<B, S>>() else {
                return Vec::new();
            };

            build_memory_tool_developer_instructions(&deps.prompt_source)
                .await
                .map(PromptFragment::developer_policy)
                .into_iter()
                .collect()
        })
    }
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for MemoriesExtension {
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        Self::store_thread_state(input.thread_store, input.config);
    }
}

impl ConfigContributor<Config> for MemoriesExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        Self::store_thread_state(thread_store, new_config);
    }
}

impl<B, S> ToolContributor for MemoriesExtension<B, S>
where
    B: MemoriesBackend,
    S: MemoryPromptSource,
{
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ToolExecutor<codex_extension_api::ToolCall>>> {
        let Some(config) = thread_store.get::<MemoriesExtensionConfig>() else {
            return Vec::new();
        };
        if !config.enabled || !config.dedicated_tools {
            return Vec::new();
        }
        let Some(deps) = thread_store.get::<MemoriesExtensionStorageDeps<B, S>>() else {
            return Vec::new();
        };

        tools::memory_tools(deps.backend.clone(), self.metrics_client.clone())
    }
}

/// Installs the memories extension contributors into the extension registry.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    metrics_client: Option<MetricsClient>,
) {
    let extension = Arc::new(MemoriesExtension::new(metrics_client));
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.tool_contributor(extension);
}
