use std::future::Future;
use std::pin::Pin;

use codex_protocol::ThreadId;

/// Future returned by one injected subagent-spawn helper.
pub type AgentSpawnFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// Constructor-injected host helper for extensions that need to spawn subagents.
///
/// The extension owns the request shape and resulting handle types. The host
/// provides the implementation when it constructs the extension.
pub trait AgentSpawner<R>: Send + Sync {
    type Spawned;
    type Error;

    fn spawn_subagent<'a>(
        &'a self,
        forked_from_thread_id: ThreadId,
        request: R,
    ) -> AgentSpawnFuture<'a, Self::Spawned, Self::Error>;
}

impl<R, S, E, F> AgentSpawner<R> for F
where
    F: Fn(ThreadId, R) -> AgentSpawnFuture<'static, S, E> + Send + Sync,
{
    type Spawned = S;
    type Error = E;

    fn spawn_subagent<'a>(
        &'a self,
        forked_from_thread_id: ThreadId,
        request: R,
    ) -> AgentSpawnFuture<'a, Self::Spawned, Self::Error> {
        self(forked_from_thread_id, request)
    }
}
