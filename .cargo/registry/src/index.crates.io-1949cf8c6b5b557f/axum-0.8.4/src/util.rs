use axum_core::response::{IntoResponse, Response};
use pin_project_lite::pin_project;
use std::{
    future::Future,
    ops::Deref,
    pin::Pin,
    sync::Arc,
    task::{ready, Context, Poll},
};
use tower::Service;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub(crate) struct PercentDecodedStr(Arc<str>);

impl PercentDecodedStr {
    pub(crate) fn new<S>(s: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        percent_encoding::percent_decode(s.as_ref().as_bytes())
            .decode_utf8()
            .ok()
            .map(|decoded| Self(decoded.as_ref().into()))
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl Deref for PercentDecodedStr {
    type Target = str;

    #[inline]
    fn deref(&self) -> &Self::Target {
        self.as_str()
    }
}

pin_project! {
    #[project = EitherProj]
    pub(crate) enum Either<A, B> {
        A { #[pin] inner: A },
        B { #[pin] inner: B },
    }
}

#[derive(Clone)]
pub(crate) struct MapIntoResponse<S> {
    inner: S,
}

impl<S> MapIntoResponse<S> {
    pub(crate) fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<B, S> Service<http::Request<B>> for MapIntoResponse<S>
where
    S: Service<http::Request<B>>,
    S::Response: IntoResponse,
{
    type Response = Response;
    type Error = S::Error;
    type Future = MapIntoResponseFuture<S::Future>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<B>) -> Self::Future {
        MapIntoResponseFuture {
            inner: self.inner.call(req),
        }
    }
}

pin_project! {
    pub(crate) struct MapIntoResponseFuture<F> {
        #[pin]
        inner: F,
    }
}

impl<F, T, E> Future for MapIntoResponseFuture<F>
where
    F: Future<Output = Result<T, E>>,
    T: IntoResponse,
{
    type Output = Result<Response, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let res = ready!(self.project().inner.poll(cx)?);
        Poll::Ready(Ok(res.into_response()))
    }
}

pub(crate) fn try_downcast<T, K>(k: K) -> Result<T, K>
where
    T: 'static,
    K: Send + 'static,
{
    let mut k = Some(k);
    if let Some(k) = <dyn std::any::Any>::downcast_mut::<Option<T>>(&mut k) {
        Ok(k.take().unwrap())
    } else {
        Err(k.unwrap())
    }
}

#[test]
fn test_try_downcast() {
    assert_eq!(try_downcast::<i32, _>(5_u32), Err(5_u32));
    assert_eq!(try_downcast::<i32, _>(5_i32), Ok(5_i32));
}
