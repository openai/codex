use std::future::Future;
use std::pin::Pin;

/// Future returned while an extension waits for a host environment to finish starting.
pub type EnvironmentStartupFuture<'a> =
    Pin<Box<dyn Future<Output = EnvironmentStartupOutcome> + Send + 'a>>;

/// Host-observed result of waiting for an environment to finish starting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EnvironmentStartupOutcome {
    /// The environment is now available to the host.
    Ready,
    /// The environment failed to start and will not become available.
    Failed,
}

/// Host-provided capability for an environment that is still starting.
///
/// Implementations must wait for the same startup operation that the host uses
/// to decide whether the environment is available to subsequent model requests.
pub trait StartingEnvironment: Send + Sync {
    /// Stable host environment id used to select the environment.
    fn environment_id(&self) -> &str;

    /// Waits until the environment becomes available or permanently fails.
    fn wait_until_ready(&self) -> EnvironmentStartupFuture<'_>;
}
