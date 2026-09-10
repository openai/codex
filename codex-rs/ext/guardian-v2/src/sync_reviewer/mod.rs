//! Installs the synchronous reviewer independently of async scorer startup.
//! Reviewer policy and pool state live in the extension; core supplies context and runtime handles.

use std::sync::Arc;
use std::sync::Weak;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_core::guardian_review::GuardianReviewSessionHost;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_extension_api::ThreadStartInput;

/// Owns reviewer state through the same thread manager as the parent conversation.
#[derive(Clone, Debug)]
pub struct GuardianExtension {
    thread_manager: Weak<ThreadManager>,
}

impl GuardianExtension {
    pub fn new(thread_manager: Weak<ThreadManager>) -> Self {
        Self { thread_manager }
    }
}

impl ThreadLifecycleContributor<Config> for GuardianExtension {
    fn on_thread_start<'a>(
        &'a self,
        input: ThreadStartInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal() {
                return;
            }
            input.thread_store.get_or_init(|| {
                GuardianReviewSessionHost::with_thread_manager(self.thread_manager.clone())
            });
        })
    }

    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if let Some(sessions) = input.thread_store.get::<GuardianReviewSessionHost>() {
                sessions.mark_ready();
            }
        })
    }
}

pub fn install(
    registry: &mut ExtensionRegistryBuilder<Config>,
    thread_manager: Weak<ThreadManager>,
) {
    registry.thread_lifecycle_contributor(Arc::new(GuardianExtension::new(thread_manager)));
}
