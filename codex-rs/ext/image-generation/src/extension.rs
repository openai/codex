use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_model_provider_info::ModelProviderInfo;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::backend::CodexImagesBackend;
use crate::tool::ImageGenerationTool;

const ACTOR_AUTHORIZATION_HEADER: &str = "x-openai-actor-authorization";

#[derive(Clone)]
struct ImageGenerationExtension {
    auth_manager: Arc<AuthManager>,
    resolve_save_root: Arc<SaveRootResolver>,
}

type SaveRootResolver = dyn Fn(&Config) -> Option<AbsolutePathBuf> + Send + Sync;

#[derive(Clone)]
struct ImageGenerationExtensionConfig {
    available: bool,
    uses_provider_auth: bool,
    provider: ModelProviderInfo,
    save_root: Option<AbsolutePathBuf>,
}

impl ImageGenerationExtensionConfig {
    /// Resolves whether standalone image generation should be available for a thread.
    fn from_config(config: &Config, resolve_save_root: &SaveRootResolver) -> Self {
        let uses_provider_auth = provider_uses_actor_authorization(&config.model_provider);
        Self {
            // Core selects this executor per turn using the feature flag or model metadata.
            available: config.model_provider.is_openai() || uses_provider_auth,
            uses_provider_auth,
            provider: config.model_provider.clone(),
            save_root: resolve_save_root(config),
        }
    }
}

fn provider_uses_actor_authorization(provider: &ModelProviderInfo) -> bool {
    !provider.requires_openai_auth
        && provider.http_headers.as_ref().is_some_and(|headers| {
            headers.iter().any(|(name, value)| {
                name.eq_ignore_ascii_case(ACTOR_AUTHORIZATION_HEADER) && !value.trim().is_empty()
            })
        })
}

impl ThreadLifecycleContributor<Config> for ImageGenerationExtension {
    /// Seeds image-generation availability when a thread begins.
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input
                .thread_store
                .insert(ImageGenerationExtensionConfig::from_config(
                    input.config,
                    self.resolve_save_root.as_ref(),
                ));
        })
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
        thread_store.insert(ImageGenerationExtensionConfig::from_config(
            new_config,
            self.resolve_save_root.as_ref(),
        ));
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
        if !config.available
            || (!config.uses_provider_auth && !self.auth_manager.current_auth_uses_codex_backend())
        {
            return Vec::new();
        }

        vec![Arc::new(ImageGenerationTool::new(
            CodexImagesBackend::new(create_model_provider(
                config.provider.clone(),
                Some(self.auth_manager.clone()),
            )),
            config.save_root.clone(),
            thread_store.level_id().to_string(),
        ))]
    }
}

/// Installs the standalone image-generation extension contributors.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    auth_manager: Arc<AuthManager>,
    resolve_save_root: impl Fn(&Config) -> Option<AbsolutePathBuf> + Send + Sync + 'static,
) {
    let extension = Arc::new(ImageGenerationExtension {
        auth_manager,
        resolve_save_root: Arc::new(resolve_save_root),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use codex_extension_api::ExtensionData;
    use codex_extension_api::ToolName;
    use codex_login::CodexAuth;

    use super::*;
    use crate::IMAGE_GEN_NAMESPACE;
    use crate::IMAGEGEN_TOOL_NAME;

    #[test]
    fn actor_authorization_requires_a_nonempty_header_on_a_provider_auth_path() {
        let provider = |requires_openai_auth, value: Option<&str>| {
            let mut provider = ModelProviderInfo::default();
            provider.requires_openai_auth = requires_openai_auth;
            provider.http_headers = value.map(|value| {
                HashMap::from([(ACTOR_AUTHORIZATION_HEADER.to_string(), value.to_string())])
            });
            provider
        };

        assert!(provider_uses_actor_authorization(&provider(
            false,
            Some("actor-biscuit")
        )));
        assert!(!provider_uses_actor_authorization(&provider(
            false,
            Some("  ")
        )));
        assert!(!provider_uses_actor_authorization(&provider(
            true,
            Some("actor-biscuit")
        )));
    }

    #[test]
    fn installed_extension_contributes_imagegen_with_provider_auth() {
        let mut builder = ExtensionRegistryBuilder::<Config>::new();
        install(
            &mut builder,
            AuthManager::from_auth_for_testing(CodexAuth::from_api_key("dummy")),
            |_| None,
        );
        let registry = builder.build();
        let session_store = ExtensionData::new("session");
        let thread_store = ExtensionData::new("11111111-1111-4111-8111-111111111111");
        let mut provider = ModelProviderInfo::default();
        provider.http_headers = Some(HashMap::from([(
            ACTOR_AUTHORIZATION_HEADER.to_string(),
            "actor-biscuit".to_string(),
        )]));
        thread_store.insert(ImageGenerationExtensionConfig {
            available: true,
            uses_provider_auth: true,
            provider,
            save_root: None,
        });

        let tool_names = registry
            .tool_contributors()
            .iter()
            .flat_map(|contributor| contributor.tools(&session_store, &thread_store))
            .map(|tool| tool.tool_name())
            .collect::<Vec<_>>();

        assert_eq!(
            tool_names,
            vec![ToolName::namespaced(
                IMAGE_GEN_NAMESPACE,
                IMAGEGEN_TOOL_NAME
            )]
        );
    }
}
