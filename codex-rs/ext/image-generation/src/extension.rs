use std::sync::Arc;

use codex_config::types::ImageGenerationSettings;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadOriginator;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_login::AuthManager;
use codex_tools::ToolExposure;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::backend::CodexImagesBackend;
use crate::tool::ImageGenerationTool;

struct ImageGenerationExtension<C> {
    auth_manager: Arc<AuthManager>,
    config_from_host: Box<dyn Fn(&C) -> ImageGenerationExtensionConfig + Send + Sync>,
}

struct ImageGenerationExtensionConfig {
    settings: ImageGenerationSettings,
    save_root: Option<AbsolutePathBuf>,
}

impl<C: Sync> ThreadLifecycleContributor<C> for ImageGenerationExtension<C> {
    fn on_thread_start<'a>(&'a self, input: ThreadStartInput<'a, C>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input
                .thread_store
                .insert((self.config_from_host)(input.config));
        })
    }
}

impl<C> ConfigContributor<C> for ImageGenerationExtension<C> {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &C,
        new_config: &C,
    ) {
        thread_store.insert((self.config_from_host)(new_config));
    }
}

impl<C> ToolContributor for ImageGenerationExtension<C> {
    /// Creates the image-generation tool exposed by this installed extension.
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let Some(config) = thread_store.get::<ImageGenerationExtensionConfig>() else {
            return Vec::new();
        };
        let settings = &config.settings;
        let provider = &settings.provider;
        if !provider.is_openai()
            && !provider.requires_openai_auth
            && !provider.uses_openai_actor_authorization()
        {
            return Vec::new();
        }
        let auth = thread_store
            .get::<codex_model_provider::ProviderAuthContext>()
            .unwrap_or_else(|| {
                Arc::new(codex_model_provider::ProviderAuthContext {
                    credentials: Arc::clone(&self.auth_manager),
                    runtime_auth: None,
                })
            });

        let exposure = if settings.deferred {
            ToolExposure::Deferred
        } else {
            ToolExposure::Direct
        };
        vec![Arc::new(ImageGenerationTool::new(
            CodexImagesBackend::new(
                auth.model_provider(provider.clone()),
                settings.http_client_factory.clone(),
                thread_store
                    .get::<ThreadOriginator>()
                    .map(|originator| originator.0.clone()),
            ),
            config.save_root.clone(),
            thread_store.level_id().to_string(),
            settings.gif_enabled,
            exposure,
        ))]
    }
}

/// Installs image generation using the host's settings and artifact storage policy.
pub fn install<C: Sync + 'static>(
    registry: &mut ExtensionRegistryBuilder<C>,
    auth_manager: Arc<AuthManager>,
    resolve_save_root: impl Fn(&C) -> Option<AbsolutePathBuf> + Send + Sync + 'static,
) where
    for<'a> ImageGenerationSettings: From<&'a C>,
{
    let extension = Arc::new(ImageGenerationExtension {
        auth_manager,
        config_from_host: Box::new(move |config: &C| ImageGenerationExtensionConfig {
            settings: ImageGenerationSettings::from(config),
            save_root: resolve_save_root(config),
        }),
    });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}
