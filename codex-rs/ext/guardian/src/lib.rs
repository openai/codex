use std::sync::Arc;

use codex_core::config::Config;
use codex_extension_api::AgentSpawnFuture;
use codex_extension_api::AgentSpawner;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ThreadStartContributor;

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

    /// Returns the host-provided agent spawn helper.
    pub fn agent_spawner(&self) -> &S {
        &self.agent_spawner
    }

    /// Delegates one guardian-owned spawn request to the host-provided helper.
    pub fn spawn_agent<'a, R>(
        &'a self,
        request: R,
    ) -> AgentSpawnFuture<'a, <S as AgentSpawner<R>>::Spawned, <S as AgentSpawner<R>>::Error>
    where
        S: AgentSpawner<R>,
    {
        self.agent_spawner.spawn_agent(request)
    }
}

impl<S> ThreadStartContributor<Config> for GuardianExtension<S>
where
    S: Send + Sync,
{
    fn contribute(
        &self,
        _input: &Config,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) {
    }
}

/// Installs the guardian contributors into the extension registry.
pub fn install<S>(registry: &mut ExtensionRegistryBuilder<Config>, agent_spawner: S)
where
    S: Send + Sync + 'static,
{
    registry.thread_start_contributor(Arc::new(GuardianExtension::new(agent_spawner)));
}
