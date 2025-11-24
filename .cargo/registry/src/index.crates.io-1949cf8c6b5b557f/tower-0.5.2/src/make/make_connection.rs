use crate::sealed::Sealed;
use std::future::Future;
use std::task::{Context, Poll};
use tokio::io::{AsyncRead, AsyncWrite};
use tower_service::Service;

/// The [`MakeConnection`] trait is used to create transports.
///
/// The goal of this service is to allow composable methods for creating
/// `AsyncRead + AsyncWrite` transports. This could mean creating a TLS
/// based connection or using some other method to authenticate the connection.
pub trait MakeConnection<Target>: Sealed<(Target,)> {
    /// The transport provided by this service
    type Connection: AsyncRead + AsyncWrite;

    /// Errors produced by the connecting service
    type Error;

    /// The future that eventually produces the transport
    type Future: Future<Output = Result<Self::Connection, Self::Error>>;

    /// Returns `Poll::Ready(Ok(()))` when it is able to make more connections.
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>>;

    /// Connect and return a transport asynchronously
    fn make_connection(&mut self, target: Target) -> Self::Future;
}

impl<S, Target> Sealed<(Target,)> for S where S: Service<Target> {}

impl<C, Target> MakeConnection<Target> for C
where
    C: Service<Target>,
    C::Response: AsyncRead + AsyncWrite,
{
    type Connection = C::Response;
    type Error = C::Error;
    type Future = C::Future;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Service::poll_ready(self, cx)
    }

    fn make_connection(&mut self, target: Target) -> Self::Future {
        Service::call(self, target)
    }
}
