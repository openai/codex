use std::sync::Arc;

use codex_analytics::AnalyticsEventsClient;
use codex_analytics::ThreadHintStatus;
use codex_analytics::ThreadHintStatusEvent;
use codex_core::config::Config;
use codex_extension_api::ConfigContributor;
use codex_extension_api::ContentItemKind;
use codex_extension_api::ContextContributor;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::PromptFragment;
use codex_extension_api::PromptSlot;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolContributor;
use codex_extension_api::ToolExecutor;
use codex_login::AuthManager;
use codex_model_provider::create_model_provider;
use codex_protocol::AgentPath;
use codex_utils_output_truncation::TruncationPolicy;
use serde_json::json;

use crate::backend::HistoryNotesBackend;
use crate::tools::HistoryNotesAction;
use crate::tools::HistoryNotesTool;

// Bound model context even when note paths are unusually long.
const MAX_THREAD_HINT_BYTES: usize = 4_000;

struct HistoryNotesExtension {
    auth_manager: Arc<AuthManager>,
}

struct HistoryNotesExtensionConfig {
    backend: HistoryNotesBackend,
}

struct HistoryNotesAgentIdentity {
    agent_name: String,
}

impl HistoryNotesExtension {
    fn update_config(&self, thread_store: &ExtensionData, config: &Config) {
        if config
            .token_budget
            .as_ref()
            .is_some_and(|token_budget| token_budget.use_history_notes_extension)
            && config.model_provider.is_openai()
            && self.auth_manager.current_auth_uses_codex_backend()
        {
            thread_store.insert(HistoryNotesExtensionConfig {
                backend: HistoryNotesBackend::new(
                    create_model_provider(
                        config.model_provider.clone(),
                        Some(self.auth_manager.clone()),
                    ),
                    config.http_client_factory(),
                ),
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
            let agent_name = input
                .session_source
                .get_agent_path()
                .unwrap_or_else(AgentPath::root)
                .to_string();
            input
                .thread_store
                .insert(HistoryNotesAgentIdentity { agent_name });
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

impl ContextContributor for HistoryNotesExtension {
    fn contribute_thread_context<'a>(
        &'a self,
        session_store: &'a ExtensionData,
        thread_store: &'a ExtensionData,
    ) -> ExtensionFuture<'a, Vec<PromptFragment>> {
        Box::pin(async move {
            let Some(config) = thread_store.get::<HistoryNotesExtensionConfig>() else {
                return Vec::new();
            };
            let Some(identity) = thread_store.get::<HistoryNotesAgentIdentity>() else {
                return Vec::new();
            };
            let track_status = |status| {
                if let Some(analytics) = session_store.get::<AnalyticsEventsClient>() {
                    analytics.track_thread_hint_status(ThreadHintStatusEvent {
                        thread_id: thread_store.level_id().to_string(),
                        status,
                        occurred_at_ms: codex_analytics::now_unix_millis(),
                    });
                }
            };
            let Ok(result) = config
                .backend
                .call(
                    "alpha/notes/v2/thread_hint",
                    session_store.level_id(),
                    &identity.agent_name,
                    json!({}),
                    TruncationPolicy::Bytes(MAX_THREAD_HINT_BYTES),
                )
                .await
            else {
                track_status(ThreadHintStatus::Failed);
                return Vec::new();
            };
            let Some(text) = result.get("text").and_then(serde_json::Value::as_str) else {
                track_status(ThreadHintStatus::Failed);
                return Vec::new();
            };
            if text.len() > MAX_THREAD_HINT_BYTES {
                track_status(ThreadHintStatus::Failed);
                return Vec::new();
            }
            track_status(ThreadHintStatus::Succeeded);
            if text.is_empty() {
                return Vec::new();
            }
            vec![PromptFragment::new(
                PromptSlot::ContextWindow,
                text,
                ContentItemKind("notes.thread_hint".to_string()),
            )]
        })
    }
}

impl ToolContributor for HistoryNotesExtension {
    fn tools(
        &self,
        session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let Some(config) = thread_store.get::<HistoryNotesExtensionConfig>() else {
            return Vec::new();
        };
        let Some(identity) = thread_store.get::<HistoryNotesAgentIdentity>() else {
            return Vec::new();
        };

        HistoryNotesAction::ALL
            .into_iter()
            .map(|action| {
                Arc::new(HistoryNotesTool::new(
                    action,
                    config.backend.clone(),
                    session_store.level_id().to_string(),
                    identity.agent_name.clone(),
                )) as Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>
            })
            .collect()
    }
}

/// Installs the standalone history and notes tools backed by the Codex backend.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>, auth_manager: Arc<AuthManager>) {
    let extension = Arc::new(HistoryNotesExtension { auth_manager });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.tool_contributor(extension);
}
