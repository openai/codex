//! Contains [`Either`] and related types and functions.
//!
//! See [`Either`] documentation for more details.

use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_layer::Layer;
use tower_service::Service;

/// Combine two different service types into a single type.
///
/// Both services must be of the same request, response, and error types.
/// [`Either`] is useful for handling conditional branching in service middleware
/// to different inner service types.
#[derive(Clone, Copy, Debug)]
pub enum Either<A, B> {
    #[allow(missing_docs)]
    Left(A),
    #[allow(missing_docs)]
    Right(B),
}

impl<A, B, Request> Service<Request> for Either<A, B>
where
    A: Service<Request>,
    B: Service<Request, Response = A::Response, Error = A::Error>,
{
    type Response = A::Response;
    type Error = A::Error;
    type Future = EitherResponseFuture<A::Future, B::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match self {
            Either::Left(service) => service.poll_ready(cx),
            Either::Right(service) => service.poll_ready(cx),
        }
    }

    fn call(&mut self, request: Request) -> Self::Future {
        match self {
            Either::Left(service) => EitherResponseFuture {
                kind: Kind::Left {
                    inner: service.call(request),
                },
            },
            Either::Right(service) => EitherResponseFuture {
                kind: Kind::Right {
                    inner: service.call(request),
                },
            },
        }
    }
}

pin_project! {
    /// Response future for [`Either`].
    pub struct EitherResponseFuture<A, B> {
        #[pin]
        kind: Kind<A, B>
    }
}

pin_project! {
    #[project = KindProj]
    enum Kind<A, B> {
        Left { #[pin] inner: A },
        Right { #[pin] inner: B },
    }
}

impl<A, B> Future for EitherResponseFuture<A, B>
where
    A: Future,
    B: Future<Output = A::Output>,
{
    type Output = A::Output;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project().kind.project() {
            KindProj::Left { inner } => inner.poll(cx),
            KindProj::Right { inner } => inner.poll(cx),
        }
    }
}

impl<S, A, B> Layer<S> for Either<A, B>
where
    A: Layer<S>,
    B: Layer<S>,
{
    type Service = Either<A::Service, B::Service>;

    fn layer(&self, inner: S) -> Self::Service {
        match self {
            Either::Left(layer) => Either::Left(layer.layer(inner)),
            Either::Right(layer) => Either::Right(layer.layer(inner)),
        }
    }
}
