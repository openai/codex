use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartContributor;
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

    /// Delegates one guardian-owned subagent spawn request to the host helper.
    pub fn spawn_subagent<'a, R>(
        &'a self,
        forked_from_thread_id: ThreadId,
        request: R,
    ) -> AgentSpawnFuture<'a, <S as AgentSpawner<R>>::Spawned, <S as AgentSpawner<R>>::Error>
    where
        S: AgentSpawner<R>,
    {
        self.agent_spawner
            .spawn_subagent(forked_from_thread_id, request)
    }
}

/// Thread-local guardian state captured when the host starts a thread.
#[derive(Clone, Copy, Debug)]
pub struct GuardianThreadContext {
    forked_from_thread_id: ThreadId,
}

impl GuardianThreadContext {
    /// Returns the thread that future guardian subagents should fork from by default.
    pub fn forked_from_thread_id(&self) -> ThreadId {
        self.forked_from_thread_id
    }
}

impl<S> ThreadStartContributor<Config> for GuardianExtension<S>
where
    S: Send + Sync,
{
    fn contribute(
        &self,
        thread_id: ThreadId,
        _input: &Config,
        _session_store: &ExtensionData,
        thread_store: &ExtensionData,
    ) {
        thread_store.insert(GuardianThreadContext {
            forked_from_thread_id: thread_id,
        });
    }
}

/// Installs the guardian contributors into the extension registry.
pub fn install<S>(registry: &mut ExtensionRegistryBuilder<Config>, agent_spawner: S)
where
    S: Send + Sync + 'static,
{
    registry.thread_start_contributor(Arc::new(GuardianExtension::new(agent_spawner)));
}
