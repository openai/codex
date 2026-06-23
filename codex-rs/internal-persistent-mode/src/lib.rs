#![recursion_limit = "256"]

use std::sync::Arc;
use std::sync::Weak;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadIdleInput;
use codex_extension_api::ThreadLifecycleContributor;
use codex_protocol::ThreadId;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use tracing::debug;

struct PersistentModeExtension {
    thread_manager: Weak<ThreadManager>,
}

impl ThreadLifecycleContributor<Config> for PersistentModeExtension {
    fn on_thread_idle<'a>(&'a self, input: ThreadIdleInput<'a>) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            let thread_id = match ThreadId::from_string(input.thread_store.level_id()) {
                Ok(thread_id) => thread_id,
                Err(err) => panic!("thread extension level id must be a valid thread id: {err}"),
            };
            let Some(thread_manager) = self.thread_manager.upgrade() else {
                debug!("skipping persistent mode because the thread manager was dropped");
                return;
            };
            let Ok(thread) = thread_manager.get_thread(thread_id).await else {
                debug!("skipping persistent mode because the live thread is unavailable");
                return;
            };
            let stored_thread = match thread
                .read_thread(
                    /*include_archived*/ true, /*include_history*/ false,
                )
                .await
            {
                Ok(stored_thread) => stored_thread,
                Err(err) => {
                    debug!(
                        "skipping persistent mode because the thread store is unavailable: {err}"
                    );
                    return;
                }
            };
            let Some(message) = stored_thread
                .extra_config
                .and_then(|extra_config| extra_config.persistent_mode_message)
            else {
                return;
            };
            let item = ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: message }],
                phase: None,
                internal_chat_message_metadata_passthrough: None,
            };

            if thread.try_start_turn_if_idle(vec![item]).await.is_err() {
                debug!("skipping persistent mode because the thread is no longer idle");
            }
        })
    }
}

/// Installs persistent mode into a host's extension registry.
pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    thread_manager: Weak<ThreadManager>,
) {
    registry.thread_lifecycle_contributor(Arc::new(PersistentModeExtension { thread_manager }));
}
