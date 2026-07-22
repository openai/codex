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

use crate::backend::HistoryNotesBackend;
use crate::tools::HistoryNotesAction;
use crate::tools::HistoryNotesTool;

struct HistoryNotesExtension {
    auth_manager: Arc<AuthManager>,
}

struct HistoryNotesExtensionConfig {
    backend: HistoryNotesBackend,
}

impl HistoryNotesExtension {
    fn update_config(&self, thread_store: &ExtensionData, config: &Config) {
        if config
            .extra_config
            .as_ref()
            .is_some_and(|extra_config| extra_config.persistent_mode_message.is_some())
        {
            thread_store.insert(HistoryNotesExtensionConfig {
                backend: HistoryNotesBackend::new(create_model_provider(
                    config.model_provider.clone(),
                    Some(self.auth_manager.clone()),
                )),
            });
        } else {
            thread_store.remove::<HistoryNotesExtensionConfig>();
        }
    }
}

impl ThreadLifecycleContributor<Config> for HistoryNotesExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            self.update_config(input.thread_store, input.config);
        })
    }
}

impl ConfigContributor<Config> for HistoryNotesExtension {
    fn on_config_changed(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _previous_config: &Config,
        new_config: &Config,
    ) {
        self.update_config(thread_store, new_config);
    }
}

impl ToolContributor for HistoryNotesExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        _step_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ToolCall>>> {
        let Some(config) = thread_store.get::<HistoryNotesExtensionConfig>() else {
            return Vec::new();
        };

        HistoryNotesAction::ALL
            .into_iter()
            .map(|action| {
                Arc::new(HistoryNotesTool::new(
                    action,
                    config.backend.clone(),
                    thread_store.level_id().to_string(),
                )) as Arc<dyn ToolExecutor<ToolCall>>
            })
            .collect()
    }
}

/// Installs the standalone history and notes tools backed by the Codex backend.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>, auth_manager: Arc<AuthManager>) {
    let extension = Arc::new(HistoryNotesExtension { auth_manager });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.tool_contributor(extension);
}
