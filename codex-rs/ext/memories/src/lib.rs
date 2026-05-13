use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolContributor;
use codex_features::Feature;
use codex_memories_read::build_memory_tool_developer_instructions;
use codex_utils_absolute_path::AbsolutePathBuf;

mod backend;
mod local;
mod schema;
mod tools;

use local::LocalMemoriesBackend;

/// Contributes Codex memory read-path prompt context and memory read tools.
#[derive(Clone, Copy, Debug, Default)]
pub struct MemoriesExtension;

#[derive(Clone, Debug)]
struct MemoriesExtensionConfig {
    enabled: bool,
    codex_home: AbsolutePathBuf,
}

impl ContextContributor for MemoriesExtension {
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

            build_memory_tool_developer_instructions(&config.codex_home)
                .await
                .map(PromptFragment::developer_policy)
                .into_iter()
                .collect()
        })
    }
}

impl ThreadLifecycleContributor<Config> for MemoriesExtension {
    fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        input.thread_store.insert(MemoriesExtensionConfig {
            enabled: input.config.features.enabled(Feature::MemoryTool)
                && input.config.memories.use_memories,
            codex_home: input.config.codex_home.clone(),
        });
    }
}

impl ToolContributor for MemoriesExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn codex_extension_api::ExtensionToolExecutor>> {
        let Some(config) = thread_store.get::<MemoriesExtensionConfig>() else {
            return Vec::new();
        };
        if !config.enabled {
            return Vec::new();
        }

        tools::memory_tools(LocalMemoriesBackend::from_codex_home(&config.codex_home))
    }
}

/// Installs the memories extension contributors into the extension registry.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>) {
    let extension = Arc::new(MemoriesExtension);
    registry.thread_lifecycle_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.tool_contributor(extension);
}

#[cfg(test)]
mod tests {
    use codex_extension_api::ContextContributor;
    use codex_extension_api::ExtensionData;
    use codex_extension_api::PromptSlot;
    use codex_extension_api::ToolContributor;
    use codex_utils_absolute_path::test_support::PathBufExt;
    use codex_utils_absolute_path::test_support::PathExt;
    use codex_utils_absolute_path::test_support::test_path_buf;
    use pretty_assertions::assert_eq;

    use super::MemoriesExtension;
    use super::MemoriesExtensionConfig;

    #[test]
    fn tools_are_not_contributed_without_thread_config() {
        let extension = MemoriesExtension;

        assert!(
            extension
                .tools(
                    &ExtensionData::new("session"),
                    &ExtensionData::new("thread")
                )
                .is_empty()
        );
    }

    #[test]
    fn tools_are_not_contributed_when_disabled() {
        let extension = MemoriesExtension;
        let thread_store = ExtensionData::new("thread");
        thread_store.insert(MemoriesExtensionConfig {
            enabled: false,
            codex_home: test_path_buf("/tmp/codex-home").abs(),
        });

        assert!(
            extension
                .tools(&ExtensionData::new("session"), &thread_store)
                .is_empty()
        );
    }

    #[test]
    fn tools_are_contributed_when_enabled() {
        let extension = MemoriesExtension;
        let thread_store = ExtensionData::new("thread");
        thread_store.insert(MemoriesExtensionConfig {
            enabled: true,
            codex_home: test_path_buf("/tmp/codex-home").abs(),
        });

        let tool_names = extension
            .tools(&ExtensionData::new("session"), &thread_store)
            .into_iter()
            .map(|tool| tool.tool_name().to_string())
            .collect::<Vec<_>>();

        assert_eq!(
            tool_names,
            vec![
                "memory_list".to_string(),
                "memory_read".to_string(),
                "memory_search".to_string()
            ]
        );
    }

    #[tokio::test]
    async fn prompt_contribution_uses_memory_summary_when_enabled() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let memories_dir = tempdir.path().join("memories");
        tokio::fs::create_dir_all(&memories_dir)
            .await
            .expect("create memories dir");
        tokio::fs::write(
            memories_dir.join("memory_summary.md"),
            "Remember repository-specific implementation preferences.",
        )
        .await
        .expect("write memory summary");

        let extension = MemoriesExtension;
        let thread_store = ExtensionData::new("thread");
        thread_store.insert(MemoriesExtensionConfig {
            enabled: true,
            codex_home: tempdir.path().abs(),
        });

        let fragments = extension
            .contribute(&ExtensionData::new("session"), &thread_store)
            .await;

        assert_eq!(fragments.len(), 1);
        assert_eq!(fragments[0].slot(), PromptSlot::DeveloperPolicy);
        assert!(
            fragments[0]
                .text()
                .contains("Remember repository-specific implementation preferences.")
        );
    }
}
