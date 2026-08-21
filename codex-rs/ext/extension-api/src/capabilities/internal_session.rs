use codex_protocol::ThreadId;

use super::agent::AgentSpawnFuture;

/// Future returned by one host-owned internal-session spawning request.
pub type InternalSessionSpawnFuture<'a, T, E> = AgentSpawnFuture<'a, T, E>;

/// Constructor-injected host helper for extensions that need private internal sessions.
///
/// The extension owns the request shape and resulting handle types. The host
/// provides the implementation when it constructs the extension.
pub trait InternalSessionSpawner<R>: Send + Sync {
    type Spawned;
    type Error;

    /// Starts a fresh host-owned internal session associated with its parent.
    fn spawn_internal_session<'a>(
        &'a self,
        parent_thread_id: ThreadId,
        request: R,
    ) -> InternalSessionSpawnFuture<'a, Self::Spawned, Self::Error>;
}

impl<R, S, E, F> InternalSessionSpawner<R> for F
where
    F: Fn(ThreadId, R) -> InternalSessionSpawnFuture<'static, S, E> + Send + Sync,
{
    type Spawned = S;
    type Error = E;

    fn spawn_internal_session<'a>(
        &'a self,
        parent_thread_id: ThreadId,
        request: R,
    ) -> InternalSessionSpawnFuture<'a, Self::Spawned, Self::Error> {
        self(parent_thread_id, request)
    }
}
