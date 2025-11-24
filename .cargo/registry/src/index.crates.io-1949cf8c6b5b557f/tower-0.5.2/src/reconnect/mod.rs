//! Reconnect services when they fail.
//!
//! Reconnect takes some [`MakeService`] and transforms it into a
//! [`Service`]. It then attempts to lazily connect and
//! reconnect on failure. The `Reconnect` service becomes unavailable
//! when the inner `MakeService::poll_ready` returns an error. When the
//! connection future returned from `MakeService::call` fails this will be
//! returned in the next call to `Reconnect::call`. This allows the user to
//! call the service again even if the inner `MakeService` was unable to
//! connect on the last call.
//!
//! [`MakeService`]: crate::make::MakeService
//! [`Service`]: crate::Service

mod future;

pub use future::ResponseFuture;

use crate::make::MakeService;
use std::fmt;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;
use tracing::trace;

/// Reconnect to failed services.
pub struct Reconnect<M, Target>
where
    M: Service<Target>,
{
    mk_service: M,
    state: State<M::Future, M::Response>,
    target: Target,
    error: Option<M::Error>,
}

#[derive(Debug)]
enum State<F, S> {
    Idle,
    Connecting(F),
    Connected(S),
}

impl<M, Target> Reconnect<M, Target>
where
    M: Service<Target>,
{
    /// Lazily connect and reconnect to a [`Service`].
    pub const fn new(mk_service: M, target: Target) -> Self {
        Reconnect {
            mk_service,
            state: State::Idle,
            target,
            error: None,
        }
    }

    /// Reconnect to a already connected [`Service`].
    pub const fn with_connection(init_conn: M::Response, mk_service: M, target: Target) -> Self {
        Reconnect {
            mk_service,
            state: State::Connected(init_conn),
            target,
            error: None,
        }
    }
}

impl<M, Target, S, Request> Service<Request> for Reconnect<M, Target>
where
    M: Service<Target, Response = S>,
    S: Service<Request>,
    M::Future: Unpin,
    crate::BoxError: From<M::Error> + From<S::Error>,
    Target: Clone,
{
    type Response = S::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<S::Future, M::Error>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        loop {
            match &mut self.state {
                State::Idle => {
                    trace!("poll_ready; idle");
                    match self.mk_service.poll_ready(cx) {
                        Poll::Ready(r) => r?,
                        Poll::Pending => {
                            trace!("poll_ready; MakeService not ready");
                            return Poll::Pending;
                        }
                    }

                    let fut = self.mk_service.make_service(self.target.clone());
                    self.state = State::Connecting(fut);
                    continue;
                }
                State::Connecting(ref mut f) => {
                    trace!("poll_ready; connecting");
                    match Pin::new(f).poll(cx) {
                        Poll::Ready(Ok(service)) => {
                            self.state = State::Connected(service);
                        }
                        Poll::Pending => {
                            trace!("poll_ready; not ready");
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(e)) => {
                            trace!("poll_ready; error");
                            self.state = State::Idle;
                            self.error = Some(e);
                            break;
                        }
                    }
                }
                State::Connected(ref mut inner) => {
                    trace!("poll_ready; connected");
                    match inner.poll_ready(cx) {
                        Poll::Ready(Ok(())) => {
                            trace!("poll_ready; ready");
                            return Poll::Ready(Ok(()));
                        }
                        Poll::Pending => {
                            trace!("poll_ready; not ready");
                            return Poll::Pending;
                        }
                        Poll::Ready(Err(_)) => {
                            trace!("poll_ready; error");
                            self.state = State::Idle;
                        }
                    }
                }
            }
        }

        Poll::Ready(Ok(()))
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if let Some(error) = self.error.take() {
            return ResponseFuture::error(error);
        }

        let service = match self.state {
            State::Connected(ref mut service) => service,
            _ => panic!("service not ready; poll_ready must be called first"),
        };

        let fut = service.call(request);
        ResponseFuture::new(fut)
    }
}

impl<M, Target> fmt::Debug for Reconnect<M, Target>
where
    M: Service<Target> + fmt::Debug,
    M::Future: fmt::Debug,
    M::Response: fmt::Debug,
    Target: fmt::Debug,
{
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        fmt.debug_struct("Reconnect")
            .field("mk_service", &self.mk_service)
            .field("state", &self.state)
            .field("target", &self.target)
            .finish()
    }
}
