use pin_project_lite::pin_project;
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use tower_service::Service;

/// A policy which decides which requests can be cloned and sent to the B
/// service.
pub trait Policy<Request> {
    fn clone_request(&self, req: &Request) -> Option<Request>;
}

/// Select is a middleware which attempts to clone the request and sends the
/// original request to the A service and, if the request was able to be cloned,
/// the cloned request to the B service.  Both resulting futures will be polled
/// and whichever future completes first will be used as the result.
#[derive(Debug)]
pub struct Select<P, A, B> {
    policy: P,
    a: A,
    b: B,
}

pin_project! {
    #[derive(Debug)]
    pub struct ResponseFuture<AF, BF> {
        #[pin]
        a_fut: AF,
        #[pin]
        b_fut: Option<BF>,
    }
}

impl<P, A, B> Select<P, A, B> {
    pub const fn new<Request>(policy: P, a: A, b: B) -> Self
    where
        P: Policy<Request>,
        A: Service<Request>,
        A::Error: Into<crate::BoxError>,
        B: Service<Request, Response = A::Response>,
        B::Error: Into<crate::BoxError>,
    {
        Select { policy, a, b }
    }
}

impl<P, A, B, Request> Service<Request> for Select<P, A, B>
where
    P: Policy<Request>,
    A: Service<Request>,
    A::Error: Into<crate::BoxError>,
    B: Service<Request, Response = A::Response>,
    B::Error: Into<crate::BoxError>,
{
    type Response = A::Response;
    type Error = crate::BoxError;
    type Future = ResponseFuture<A::Future, B::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        match (self.a.poll_ready(cx), self.b.poll_ready(cx)) {
            (Poll::Ready(Ok(())), Poll::Ready(Ok(()))) => Poll::Ready(Ok(())),
            (Poll::Ready(Err(e)), _) => Poll::Ready(Err(e.into())),
            (_, Poll::Ready(Err(e))) => Poll::Ready(Err(e.into())),
            _ => Poll::Pending,
        }
    }

    fn call(&mut self, request: Request) -> Self::Future {
        let b_fut = if let Some(cloned_req) = self.policy.clone_request(&request) {
            Some(self.b.call(cloned_req))
        } else {
            None
        };
        ResponseFuture {
            a_fut: self.a.call(request),
            b_fut,
        }
    }
}

impl<AF, BF, T, AE, BE> Future for ResponseFuture<AF, BF>
where
    AF: Future<Output = Result<T, AE>>,
    AE: Into<crate::BoxError>,
    BF: Future<Output = Result<T, BE>>,
    BE: Into<crate::BoxError>,
{
    type Output = Result<T, crate::BoxError>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        if let Poll::Ready(r) = this.a_fut.poll(cx) {
            return Poll::Ready(Ok(r.map_err(Into::into)?));
        }
        if let Some(b_fut) = this.b_fut.as_pin_mut() {
            if let Poll::Ready(r) = b_fut.poll(cx) {
                return Poll::Ready(Ok(r.map_err(Into::into)?));
            }
        }
        Poll::Pending
    }
}
