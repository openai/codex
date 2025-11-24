use http::{
    header::{self, HeaderName, HeaderValue},
    request::Parts as RequestParts,
};
use pin_project_lite::pin_project;
use std::{
    array, fmt,
    future::Future,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
};

use super::{Any, WILDCARD};

/// Holds configuration for how to set the [`Access-Control-Allow-Origin`][mdn] header.
///
/// See [`CorsLayer::allow_origin`] for more details.
///
/// [mdn]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin
/// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
#[derive(Clone, Default)]
#[must_use]
pub struct AllowOrigin(OriginInner);

impl AllowOrigin {
    /// Allow any origin by sending a wildcard (`*`)
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    pub fn any() -> Self {
        Self(OriginInner::Const(WILDCARD))
    }

    /// Set a single allowed origin
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    pub fn exact(origin: HeaderValue) -> Self {
        Self(OriginInner::Const(origin))
    }

    /// Set multiple allowed origins
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// # Panics
    ///
    /// If the iterator contains a wildcard (`*`).
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    #[allow(clippy::borrow_interior_mutable_const)]
    pub fn list<I>(origins: I) -> Self
    where
        I: IntoIterator<Item = HeaderValue>,
    {
        let origins = origins.into_iter().collect::<Vec<_>>();
        if origins.contains(&WILDCARD) {
            panic!(
                "Wildcard origin (`*`) cannot be passed to `AllowOrigin::list`. \
                 Use `AllowOrigin::any()` instead"
            );
        }

        Self(OriginInner::List(origins))
    }

    /// Set the allowed origins from a predicate
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    pub fn predicate<F>(f: F) -> Self
    where
        F: Fn(&HeaderValue, &RequestParts) -> bool + Send + Sync + 'static,
    {
        Self(OriginInner::Predicate(Arc::new(f)))
    }

    /// Set the allowed origins from an async predicate
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    pub fn async_predicate<F, Fut>(f: F) -> Self
    where
        F: FnOnce(HeaderValue, &RequestParts) -> Fut + Send + Sync + 'static + Clone,
        Fut: Future<Output = bool> + Send + 'static,
    {
        Self(OriginInner::AsyncPredicate(Arc::new(move |v, p| {
            Box::pin((f.clone())(v, p))
        })))
    }

    /// Allow any origin, by mirroring the request origin
    ///
    /// This is equivalent to
    /// [`AllowOrigin::predicate(|_, _| true)`][Self::predicate].
    ///
    /// See [`CorsLayer::allow_origin`] for more details.
    ///
    /// [`CorsLayer::allow_origin`]: super::CorsLayer::allow_origin
    pub fn mirror_request() -> Self {
        Self::predicate(|_, _| true)
    }

    #[allow(clippy::borrow_interior_mutable_const)]
    pub(super) fn is_wildcard(&self) -> bool {
        matches!(&self.0, OriginInner::Const(v) if v == WILDCARD)
    }

    pub(super) fn to_future(
        &self,
        origin: Option<&HeaderValue>,
        parts: &RequestParts,
    ) -> AllowOriginFuture {
        let name = header::ACCESS_CONTROL_ALLOW_ORIGIN;

        match &self.0 {
            OriginInner::Const(v) => AllowOriginFuture::ok(Some((name, v.clone()))),
            OriginInner::List(l) => {
                AllowOriginFuture::ok(origin.filter(|o| l.contains(o)).map(|o| (name, o.clone())))
            }
            OriginInner::Predicate(c) => AllowOriginFuture::ok(
                origin
                    .filter(|origin| c(origin, parts))
                    .map(|o| (name, o.clone())),
            ),
            OriginInner::AsyncPredicate(f) => {
                if let Some(origin) = origin.cloned() {
                    let fut = f(origin.clone(), parts);
                    AllowOriginFuture::fut(async move { fut.await.then_some((name, origin)) })
                } else {
                    AllowOriginFuture::ok(None)
                }
            }
        }
    }
}

pin_project! {
    #[project = AllowOriginFutureProj]
    pub(super) enum AllowOriginFuture {
        Ok{
            res: Option<(HeaderName, HeaderValue)>
        },
        Future{
            #[pin]
            future: Pin<Box<dyn Future<Output = Option<(HeaderName, HeaderValue)>> + Send + 'static>>
        },
    }
}

impl AllowOriginFuture {
    fn ok(res: Option<(HeaderName, HeaderValue)>) -> Self {
        Self::Ok { res }
    }

    fn fut<F: Future<Output = Option<(HeaderName, HeaderValue)>> + Send + 'static>(
        future: F,
    ) -> Self {
        Self::Future {
            future: Box::pin(future),
        }
    }
}

impl Future for AllowOriginFuture {
    type Output = Option<(HeaderName, HeaderValue)>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match self.project() {
            AllowOriginFutureProj::Ok { res } => Poll::Ready(res.take()),
            AllowOriginFutureProj::Future { future } => future.poll(cx),
        }
    }
}

impl fmt::Debug for AllowOrigin {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            OriginInner::Const(inner) => f.debug_tuple("Const").field(inner).finish(),
            OriginInner::List(inner) => f.debug_tuple("List").field(inner).finish(),
            OriginInner::Predicate(_) => f.debug_tuple("Predicate").finish(),
            OriginInner::AsyncPredicate(_) => f.debug_tuple("AsyncPredicate").finish(),
        }
    }
}

impl From<Any> for AllowOrigin {
    fn from(_: Any) -> Self {
        Self::any()
    }
}

impl From<HeaderValue> for AllowOrigin {
    fn from(val: HeaderValue) -> Self {
        Self::exact(val)
    }
}

impl<const N: usize> From<[HeaderValue; N]> for AllowOrigin {
    fn from(arr: [HeaderValue; N]) -> Self {
        #[allow(deprecated)] // Can be changed when MSRV >= 1.53
        Self::list(array::IntoIter::new(arr))
    }
}

impl From<Vec<HeaderValue>> for AllowOrigin {
    fn from(vec: Vec<HeaderValue>) -> Self {
        Self::list(vec)
    }
}

#[derive(Clone)]
enum OriginInner {
    Const(HeaderValue),
    List(Vec<HeaderValue>),
    Predicate(
        Arc<dyn for<'a> Fn(&'a HeaderValue, &'a RequestParts) -> bool + Send + Sync + 'static>,
    ),
    AsyncPredicate(
        Arc<
            dyn for<'a> Fn(
                    HeaderValue,
                    &'a RequestParts,
                ) -> Pin<Box<dyn Future<Output = bool> + Send + 'static>>
                + Send
                + Sync
                + 'static,
        >,
    ),
}

impl Default for OriginInner {
    fn default() -> Self {
        Self::List(Vec::new())
    }
}
