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
use codex_protocol::AgentPath;
use codex_protocol::openai_models::HistoryToolMessages;
use codex_protocol::openai_models::NotesToolMessages;
use codex_utils_output_truncation::TruncationPolicy;
use serde_json::json;

use crate::backend::HttpBackend;
use crate::backend::HttpContext;
use crate::tools::history_notes_tools;

// Bound model context even when note paths are unusually long.
const MAX_THREAD_HINT_BYTES: usize = 4_000;

struct HistoryNotesExtension {
    auth_manager: Arc<AuthManager>,
}

impl HistoryNotesExtension {
    fn update_config(&self, thread_store: &ExtensionData, config: &Config) {
        let auth = thread_store
            .get::<codex_model_provider::ProviderAuthContext>()
            .unwrap_or_else(|| {
                Arc::new(codex_model_provider::ProviderAuthContext {
                    credentials: Arc::clone(&self.auth_manager),
                    runtime_auth: None,
                })
            });
        if config
            .token_budget
            .as_ref()
            .is_some_and(|token_budget| token_budget.use_history_notes_extension)
            && config.model_provider.is_openai()
            && auth.credentials.current_auth_uses_codex_backend()
            && let Some(context) = thread_store.get::<HttpContext>()
        {
            thread_store.insert(HttpBackend::new(
                auth.model_provider(config.model_provider.clone()),
                config.http_client_factory(),
                context,
            ));
        } else {
            thread_store.remove::<HttpBackend>();
        }
    }
}

impl ThreadLifecycleContributor<Config> for HistoryNotesExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            input.thread_store.insert(HttpContext {
                session_id: input.session_store.level_id().to_owned(),
                current_agent_name: input
                    .session_source
                    .get_agent_path()
                    .unwrap_or_else(AgentPath::root)
                    .to_string(),
            });
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
            let Some(backend) = thread_store.get::<HttpBackend>() else {
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
            let Ok(result) = backend
                .call(
                    "alpha/notes/v2/thread_hint",
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
            vec![PromptFragment::tool(
                PromptSlot::ContextWindow,
                text,
                ContentItemKind("notes.thread_hint".to_string()),
                "notes".to_string(),
            )]
        })
    }
}

impl ToolContributor for HistoryNotesExtension {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        self.tools_with_messages(
            thread_store,
            /*history_messages*/ None,
            /*notes_messages*/ None,
        )
    }

    fn tools_for_step(
        &self,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
        step_store: &ExtensionData,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let history_messages = step_store.get::<HistoryToolMessages>();
        let notes_messages = step_store.get::<NotesToolMessages>();
        self.tools_with_messages(
            thread_store,
            history_messages.as_deref(),
            notes_messages.as_deref(),
        )
    }
}

impl HistoryNotesExtension {
    fn tools_with_messages(
        &self,
        thread_store: &ExtensionData,
        history_messages: Option<&HistoryToolMessages>,
        notes_messages: Option<&NotesToolMessages>,
    ) -> Vec<Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>> {
        let Some(backend) = thread_store.get::<HttpBackend>() else {
            return Vec::new();
        };
        history_notes_tools((*backend).clone(), history_messages, notes_messages)
    }
}

/// Installs the standalone history and notes tools.
pub fn install(registry: &mut ExtensionRegistryBuilder<Config>, auth_manager: Arc<AuthManager>) {
    let extension = Arc::new(HistoryNotesExtension { auth_manager });
    registry.thread_lifecycle_contributor(extension.clone());
    registry.config_contributor(extension.clone());
    registry.prompt_contributor(extension.clone());
    registry.tool_contributor(extension);
}
