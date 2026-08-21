use std::sync::Arc;
use std::sync::Weak;

use codex_core::ThreadManager;
use codex_core::config::Config;
use codex_extension_api::ExtensionFuture;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::InternalSessionSpawnFuture;
use codex_extension_api::InternalSessionSpawner;
use codex_extension_api::ThreadLifecycleContributor;
use codex_extension_api::ThreadReadyInput;
use codex_protocol::ThreadId;

/// Guardian extension dependencies supplied by the host at construction time.
#[derive(Clone, Debug)]
pub struct GuardianExtension<S> {
    thread_manager: Weak<ThreadManager>,
    internal_session_spawner: S,
}

impl<S> GuardianExtension<S> {
    /// Creates a guardian extension with its host-owned internal-session spawner.
    pub fn new(thread_manager: Weak<ThreadManager>, internal_session_spawner: S) -> Self {
        Self {
            thread_manager,
            internal_session_spawner,
        }
    }

    /// Delegates a fresh internal-session request to the host helper.
    pub fn spawn_internal_session<'a, R>(
        &'a self,
        parent_thread_id: ThreadId,
        request: R,
    ) -> InternalSessionSpawnFuture<
        'a,
        <S as InternalSessionSpawner<R>>::Spawned,
        <S as InternalSessionSpawner<R>>::Error,
    >
    where
        S: InternalSessionSpawner<R>,
    {
        self.internal_session_spawner
            .spawn_internal_session(parent_thread_id, request)
    }
}

/// Thread-local guardian state captured after the host registers a thread.
#[derive(Clone, Debug)]
pub struct GuardianThreadContext {
    parent_thread_id: ThreadId,
    parent_model: String,
}

impl GuardianThreadContext {
    /// Returns the parent thread associated with future Guardian reviewer sessions.
    pub fn parent_thread_id(&self) -> ThreadId {
        self.parent_thread_id
    }

    /// Returns the parent's effective model after provider fallback.
    pub fn parent_model(&self) -> &str {
        &self.parent_model
    }
}

impl<S> ThreadLifecycleContributor<Config> for GuardianExtension<S>
where
    S: Send + Sync,
{
    fn on_thread_ready<'a>(
        &'a self,
        input: ThreadReadyInput<'a, Config>,
    ) -> ExtensionFuture<'a, ()> {
        Box::pin(async move {
            if input.session_source.is_internal() {
                return;
            }
            let Ok(parent_thread_id) = ThreadId::from_string(input.thread_store.level_id()) else {
                return;
            };
            let Some(thread_manager) = self.thread_manager.upgrade() else {
                return;
            };
            let Ok(parent) = thread_manager.get_thread(parent_thread_id).await else {
                return;
            };
            input.thread_store.insert(GuardianThreadContext {
                parent_thread_id,
                parent_model: parent.config_snapshot().await.model,
            });
        })
    }
}

/// Installs the guardian contributors into the extension registry.
pub fn install<S>(
    registry: &mut ExtensionRegistryBuilder<Config>,
    thread_manager: Weak<ThreadManager>,
    internal_session_spawner: S,
) where
    S: Send + Sync + 'static,
{
    registry.thread_lifecycle_contributor(Arc::new(GuardianExtension::new(
        thread_manager,
        internal_session_spawner,
    )));
}
