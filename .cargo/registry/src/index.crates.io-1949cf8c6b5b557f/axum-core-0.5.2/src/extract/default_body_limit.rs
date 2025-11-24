use self::private::DefaultBodyLimitService;
use tower_layer::Layer;

/// Layer for configuring the default request body limit.
///
/// For security reasons, [`Bytes`] will, by default, not accept bodies larger than 2MB. This also
/// applies to extractors that uses [`Bytes`] internally such as `String`, [`Json`], and [`Form`].
///
/// This middleware provides ways to configure that.
///
/// Note that if an extractor consumes the body directly with [`Body::poll_frame`], or similar, the
/// default limit is _not_ applied.
///
/// # Difference between `DefaultBodyLimit` and [`RequestBodyLimit`]
///
/// `DefaultBodyLimit` and [`RequestBodyLimit`] serve similar functions but in different ways.
///
/// `DefaultBodyLimit` is local in that it only applies to [`FromRequest`] implementations that
/// explicitly apply it (or call another extractor that does). You can apply the limit with
/// [`RequestExt::with_limited_body`] or [`RequestExt::into_limited_body`]
///
/// [`RequestBodyLimit`] is applied globally to all requests, regardless of which extractors are
/// used or how the body is consumed.
///
/// # Example
///
/// ```
/// use axum::{
///     Router,
///     routing::post,
///     body::Body,
///     extract::{Request, DefaultBodyLimit},
/// };
///
/// let app = Router::new()
///     .route("/", post(|request: Request| async {}))
///     // change the default limit
///     .layer(DefaultBodyLimit::max(1024));
/// # let _: Router = app;
/// ```
///
/// In general using `DefaultBodyLimit` is recommended but if you need to use third party
/// extractors and want to make sure a limit is also applied there then [`RequestBodyLimit`] should
/// be used.
///
/// # Different limits for different routes
///
/// `DefaultBodyLimit` can also be selectively applied to have different limits for different
/// routes:
///
/// ```
/// use axum::{
///     Router,
///     routing::post,
///     body::Body,
///     extract::{Request, DefaultBodyLimit},
/// };
///
/// let app = Router::new()
///     // this route has a different limit
///     .route("/", post(|request: Request| async {}).layer(DefaultBodyLimit::max(1024)))
///     // this route still has the default limit
///     .route("/foo", post(|request: Request| async {}));
/// # let _: Router = app;
/// ```
///
/// [`Body::poll_frame`]: http_body::Body::poll_frame
/// [`Bytes`]: bytes::Bytes
/// [`Json`]: https://docs.rs/axum/0.8/axum/struct.Json.html
/// [`Form`]: https://docs.rs/axum/0.8/axum/struct.Form.html
/// [`FromRequest`]: crate::extract::FromRequest
/// [`RequestBodyLimit`]: tower_http::limit::RequestBodyLimit
/// [`RequestExt::with_limited_body`]: crate::RequestExt::with_limited_body
/// [`RequestExt::into_limited_body`]: crate::RequestExt::into_limited_body
#[derive(Debug, Clone, Copy)]
#[must_use]
pub struct DefaultBodyLimit {
    kind: DefaultBodyLimitKind,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum DefaultBodyLimitKind {
    Disable,
    Limit(usize),
}

impl DefaultBodyLimit {
    /// Disable the default request body limit.
    ///
    /// This must be used to receive bodies larger than the default limit of 2MB using [`Bytes`] or
    /// an extractor built on it such as `String`, [`Json`], [`Form`].
    ///
    /// Note that if you're accepting data from untrusted remotes it is recommend to add your own
    /// limit such as [`tower_http::limit`].
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::{Bytes, Body},
    ///     extract::DefaultBodyLimit,
    /// };
    /// use tower_http::limit::RequestBodyLimitLayer;
    ///
    /// let app: Router<()> = Router::new()
    ///     .route("/", get(|body: Bytes| async {}))
    ///     // Disable the default limit
    ///     .layer(DefaultBodyLimit::disable())
    ///     // Set a different limit
    ///     .layer(RequestBodyLimitLayer::new(10 * 1000 * 1000));
    /// ```
    ///
    /// [`Bytes`]: bytes::Bytes
    /// [`Json`]: https://docs.rs/axum/0.8/axum/struct.Json.html
    /// [`Form`]: https://docs.rs/axum/0.8/axum/struct.Form.html
    pub const fn disable() -> Self {
        Self {
            kind: DefaultBodyLimitKind::Disable,
        }
    }

    /// Set the default request body limit.
    ///
    /// By default the limit of request body sizes that [`Bytes::from_request`] (and other
    /// extractors built on top of it such as `String`, [`Json`], and [`Form`]) is 2MB. This method
    /// can be used to change that limit.
    ///
    /// # Example
    ///
    /// ```
    /// use axum::{
    ///     Router,
    ///     routing::get,
    ///     body::{Bytes, Body},
    ///     extract::DefaultBodyLimit,
    /// };
    ///
    /// let app: Router<()> = Router::new()
    ///     .route("/", get(|body: Bytes| async {}))
    ///     // Replace the default of 2MB with 1024 bytes.
    ///     .layer(DefaultBodyLimit::max(1024));
    /// ```
    ///
    /// [`Bytes::from_request`]: bytes::Bytes
    /// [`Json`]: https://docs.rs/axum/0.8/axum/struct.Json.html
    /// [`Form`]: https://docs.rs/axum/0.8/axum/struct.Form.html
    pub const fn max(limit: usize) -> Self {
        Self {
            kind: DefaultBodyLimitKind::Limit(limit),
        }
    }
}

impl<S> Layer<S> for DefaultBodyLimit {
    type Service = DefaultBodyLimitService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        DefaultBodyLimitService {
            inner,
            kind: self.kind,
        }
    }
}

mod private {
    use super::DefaultBodyLimitKind;
    use http::Request;
    use std::task::Context;
    use tower_service::Service;

    #[derive(Debug, Clone, Copy)]
    pub struct DefaultBodyLimitService<S> {
        pub(super) inner: S,
        pub(super) kind: DefaultBodyLimitKind,
    }

    impl<B, S> Service<Request<B>> for DefaultBodyLimitService<S>
    where
        S: Service<Request<B>>,
    {
        type Response = S::Response;
        type Error = S::Error;
        type Future = S::Future;

        #[inline]
        fn poll_ready(&mut self, cx: &mut Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
            self.inner.poll_ready(cx)
        }

        #[inline]
        fn call(&mut self, mut req: Request<B>) -> Self::Future {
            req.extensions_mut().insert(self.kind);
            self.inner.call(req)
        }
    }
}
