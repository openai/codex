use std::path::PathBuf;
use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_features::Feature;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;

use crate::backend::CodexImagesBackend;
use crate::tool::ImageGenerationTool;
use crate::tool::generated_image_output_dir;

#[derive(Clone)]
struct ImageGenerationExtension {
    auth_manager: Arc<AuthManager>,
}

#[derive(Clone)]
struct ImageGenerationExtensionConfig {
    enabled: bool,
    provider: ModelProviderInfo,
    codex_home: PathBuf,
}

impl From<&Config> for ImageGenerationExtensionConfig {
    /// Resolves whether standalone image generation should be available for a thread.
    fn from(config: &Config) -> Self {
        Self {
            enabled: config.features.enabled(Feature::ImageGenExt)
                && config.model_provider.is_openai(),
            provider: config.model_provider.clone(),
            codex_home: config.codex_home.to_path_buf(),
        }
    }
}

#[async_trait::async_trait]
impl ThreadLifecycleContributor<Config> for ImageGenerationExtension {
    /// Seeds image-generation availability when a thread begins.
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        input
            .thread_store
            .insert(ImageGenerationExtensionConfig::from(input.config));
    }
}

impl ConfigContributor<Config> for ImageGenerationExtension {
    /// Refreshes image-generation availability after thread configuration changes.
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        thread_store.insert(ImageGenerationExtensionConfig::from(new_config));
    }
}

impl ToolContributor for ImageGenerationExtension {
    /// Creates the image-generation tool exposed by this installed extension.
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(config) = thread_store.get::<ImageGenerationExtensionConfig>() else {
            return Vec::new();
        };
        if !config.enabled || !self.auth_manager.current_auth_uses_codex_backend() {
            return Vec::new();
        }

        vec![Arc::new(ImageGenerationTool::new(
            CodexImagesBackend::new(create_model_provider(
                config.provider.clone(),
                Some(self.auth_manager.clone()),
            )),
            generated_image_output_dir(&config.codex_home, thread_store.level_id()),
        ))]
    }
}

/// Installs the feature-gated standalone image-generation extension contributors.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>, auth_manager: Arc<AuthManager>) {
    let extension = Arc::new(ImageGenerationExtension { auth_manager });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}

#[cfg(test)]
mod tests {
    use codex_extension_api::ExtensionData;
    use codex_extension_api::ExtensionRegistryBuilder;
    use codex_extension_api::ToolName;
    use codex_login::AuthManager;
    use codex_login::CodexAuth;
    use codex_model_provider_info::ModelProviderInfo;
    use pretty_assertions::assert_eq;

    use super::Config;
    use super::ImageGenerationExtensionConfig;
    use super::install;

    #[test]
    fn enabled_extension_contributes_imagegen_with_chatgpt_auth() {
        assert_eq!(
            contributed_tool_names(CodexAuth::create_dummy_chatgpt_auth_for_testing()),
            vec![ToolName::namespaced("image_gen", "imagegen")]
        );
    }

    #[test]
    fn enabled_extension_is_not_available_with_api_key_auth() {
        assert_eq!(
            contributed_tool_names(CodexAuth::from_api_key("dummy")),
            Vec::<ToolName>::new()
        );
    }

    fn contributed_tool_names(auth: CodexAuth) -> Vec<ToolName> {
        let mut builder = ExtensionRegistryBuilder::<Config>::new();
        install(&mut builder, AuthManager::from_auth_for_testing(auth));
        let registry = builder.build();
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("thread");
        thread_store.insert(ImageGenerationExtensionConfig {
            enabled: true,
            provider: ModelProviderInfo::create_openai_provider(/*base_url*/ None),
            codex_home: std::path::PathBuf::from("/tmp/codex-home"),
        });

        registry
            .tool_contributors()
            .iter()
            .flat_map(|contributor| contributor.tools(&session_store, &thread_store))
            .map(|tool| tool.tool_name())
            .collect()
    }
}
