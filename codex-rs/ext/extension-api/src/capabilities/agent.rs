use std::future::Future;
use std::pin::Pin;

/// Future returned by one injected agent-spawn helper.
pub type AgentSpawnFuture<'a, T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>;

/// Constructor-injected host helper for extensions that need to spawn agents.
///
/// The extension owns the request shape and resulting handle types. The host
/// provides the implementation when it constructs the extension.
pub trait AgentSpawner<R>: Send + Sync {
    type Spawned;
    type Error;

    fn spawn_agent<'a>(&'a self, request: R) -> AgentSpawnFuture<'a, Self::Spawned, Self::Error>;
}

impl<R, S, E, F> AgentSpawner<R> for F
where
    F: Fn(R) -> AgentSpawnFuture<'static, S, E> + Send + Sync,
{
    type Spawned = S;
    type Error = E;

    fn spawn_agent<'a>(&'a self, request: R) -> AgentSpawnFuture<'a, Self::Spawned, Self::Error> {
        self(request)
    }
}
