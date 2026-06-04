use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadStartInput;
use codex_protocol::ThreadId;

/// Guardian extension dependencies supplied by the host at construction time.
#[derive(Clone, Debug)]
pub struct GuardianExtension<S> {
    agent_spawner: S,
}

impl<S> GuardianExtension<S> {
    /// Creates a guardian extension with its host-provided agent spawn helper.
    pub fn new(agent_spawner: S) -> Self {
        Self { agent_spawner }
    }
}

/// Thread-local guardian state captured when the host starts a thread.
#[derive(Clone, Copy, Debug)]
pub struct GuardianThreadContext {
    forked_from_thread_id: ThreadId,
}

#[async_trait::async_trait]
impl<S> ThreadLifecycleContributor<Config> for GuardianExtension<S>
where
    S: Send + Sync,
{
    async fn on_thread_start(&self, input: ThreadStartInput<'_, Config>) {
        let Ok(forked_from_thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
            return;
        };
        input.thread_store.insert(GuardianThreadContext {
            forked_from_thread_id,
        });
    }
}

/// Installs the guardian contributors into the extension registry.
pub fn install<S>(registry: &mut ExtensionRegistryBuilder<Config>, agent_spawner: S)
where
    S: Send + Sync + 'static,
{
    registry.thread_lifecycle_contributor(Arc::new(GuardianExtension::new(agent_spawner)));
}
