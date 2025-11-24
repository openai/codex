use tower::ServiceBuilder;

#[allow(unused_imports)]
use http::header::HeaderName;
#[allow(unused_imports)]
use tower_layer::Stack;

mod sealed {
    #[allow(unreachable_pub, unused)]
    pub trait Sealed<T> {}
}

/// Extension trait that adds methods to [`tower::ServiceBuilder`] for adding middleware from
/// tower-http.
///
/// [`Service`]: tower::Service
///
/// # Example
///
/// ```rust
/// use http::{Request, Response, header::HeaderName};
/// use bytes::Bytes;
/// use http_body_util::Full;
/// use std::{time::Duration, convert::Infallible};
/// use tower::{ServiceBuilder, ServiceExt, Service};
/// use tower_http::ServiceBuilderExt;
///
/// async fn handle(request: Request<Full<Bytes>>) -> Result<Response<Full<Bytes>>, Infallible> {
///     Ok(Response::new(Full::default()))
/// }
///
/// # #[tokio::main]
/// # async fn main() {
/// let service = ServiceBuilder::new()
///     // Methods from tower
///     .timeout(Duration::from_secs(30))
///     // Methods from tower-http
///     .trace_for_http()
///     .propagate_header(HeaderName::from_static("x-request-id"))
///     .service_fn(handle);
/// # let mut service = service;
/// # service.ready().await.unwrap().call(Request::new(Full::default())).await.unwrap();
/// # }
/// ```
#[cfg(feature = "util")]
// ^ work around rustdoc not inferring doc(cfg)s for cfg's from surrounding scopes
pub trait ServiceBuilderExt<L>: sealed::Sealed<L> + Sized {
    /// Propagate a header from the request to the response.
    ///
    /// See [`tower_http::propagate_header`] for more details.
    ///
    /// [`tower_http::propagate_header`]: crate::propagate_header
    #[cfg(feature = "propagate-header")]
    fn propagate_header(
        self,
        header: HeaderName,
    ) -> ServiceBuilder<Stack<crate::propagate_header::PropagateHeaderLayer, L>>;

    /// Add some shareable value to [request extensions].
    ///
    /// See [`tower_http::add_extension`] for more details.
    ///
    /// [`tower_http::add_extension`]: crate::add_extension
    /// [request extensions]: https://docs.rs/http/latest/http/struct.Extensions.html
    #[cfg(feature = "add-extension")]
    fn add_extension<T>(
        self,
        value: T,
    ) -> ServiceBuilder<Stack<crate::add_extension::AddExtensionLayer<T>, L>>;

    /// Apply a transformation to the request body.
    ///
    /// See [`tower_http::map_request_body`] for more details.
    ///
    /// [`tower_http::map_request_body`]: crate::map_request_body
    #[cfg(feature = "map-request-body")]
    fn map_request_body<F>(
        self,
        f: F,
    ) -> ServiceBuilder<Stack<crate::map_request_body::MapRequestBodyLayer<F>, L>>;

    /// Apply a transformation to the response body.
    ///
    /// See [`tower_http::map_response_body`] for more details.
    ///
    /// [`tower_http::map_response_body`]: crate::map_response_body
    #[cfg(feature = "map-response-body")]
    fn map_response_body<F>(
        self,
        f: F,
    ) -> ServiceBuilder<Stack<crate::map_response_body::MapResponseBodyLayer<F>, L>>;

    /// Compresses response bodies.
    ///
    /// See [`tower_http::compression`] for more details.
    ///
    /// [`tower_http::compression`]: crate::compression
    #[cfg(any(
        feature = "compression-br",
        feature = "compression-deflate",
        feature = "compression-gzip",
        feature = "compression-zstd",
    ))]
    fn compression(self) -> ServiceBuilder<Stack<crate::compression::CompressionLayer, L>>;

    /// Decompress response bodies.
    ///
    /// See [`tower_http::decompression`] for more details.
    ///
    /// [`tower_http::decompression`]: crate::decompression
    #[cfg(any(
        feature = "decompression-br",
        feature = "decompression-deflate",
        feature = "decompression-gzip",
        feature = "decompression-zstd",
    ))]
    fn decompression(self) -> ServiceBuilder<Stack<crate::decompression::DecompressionLayer, L>>;

    /// High level tracing that classifies responses using HTTP status codes.
    ///
    /// This method does not support customizing the output, to do that use [`TraceLayer`]
    /// instead.
    ///
    /// See [`tower_http::trace`] for more details.
    ///
    /// [`tower_http::trace`]: crate::trace
    /// [`TraceLayer`]: crate::trace::TraceLayer
    #[cfg(feature = "trace")]
    fn trace_for_http(
        self,
    ) -> ServiceBuilder<Stack<crate::trace::TraceLayer<crate::trace::HttpMakeClassifier>, L>>;

    /// High level tracing that classifies responses using gRPC headers.
    ///
    /// This method does not support customizing the output, to do that use [`TraceLayer`]
    /// instead.
    ///
    /// See [`tower_http::trace`] for more details.
    ///
    /// [`tower_http::trace`]: crate::trace
    /// [`TraceLayer`]: crate::trace::TraceLayer
    #[cfg(feature = "trace")]
    fn trace_for_grpc(
        self,
    ) -> ServiceBuilder<Stack<crate::trace::TraceLayer<crate::trace::GrpcMakeClassifier>, L>>;

    /// Follow redirect resposes using the [`Standard`] policy.
    ///
    /// See [`tower_http::follow_redirect`] for more details.
    ///
    /// [`tower_http::follow_redirect`]: crate::follow_redirect
    /// [`Standard`]: crate::follow_redirect::policy::Standard
    #[cfg(feature = "follow-redirect")]
    fn follow_redirects(
        self,
    ) -> ServiceBuilder<
        Stack<
            crate::follow_redirect::FollowRedirectLayer<crate::follow_redirect::policy::Standard>,
            L,
        >,
    >;

    /// Mark headers as [sensitive] on both requests and responses.
    ///
    /// See [`tower_http::sensitive_headers`] for more details.
    ///
    /// [sensitive]: https://docs.rs/http/latest/http/header/struct.HeaderValue.html#method.set_sensitive
    /// [`tower_http::sensitive_headers`]: crate::sensitive_headers
    #[cfg(feature = "sensitive-headers")]
    fn sensitive_headers<I>(
        self,
        headers: I,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveHeadersLayer, L>>
    where
        I: IntoIterator<Item = HeaderName>;

    /// Mark headers as [sensitive] on requests.
    ///
    /// See [`tower_http::sensitive_headers`] for more details.
    ///
    /// [sensitive]: https://docs.rs/http/latest/http/header/struct.HeaderValue.html#method.set_sensitive
    /// [`tower_http::sensitive_headers`]: crate::sensitive_headers
    #[cfg(feature = "sensitive-headers")]
    fn sensitive_request_headers(
        self,
        headers: std::sync::Arc<[HeaderName]>,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveRequestHeadersLayer, L>>;

    /// Mark headers as [sensitive] on responses.
    ///
    /// See [`tower_http::sensitive_headers`] for more details.
    ///
    /// [sensitive]: https://docs.rs/http/latest/http/header/struct.HeaderValue.html#method.set_sensitive
    /// [`tower_http::sensitive_headers`]: crate::sensitive_headers
    #[cfg(feature = "sensitive-headers")]
    fn sensitive_response_headers(
        self,
        headers: std::sync::Arc<[HeaderName]>,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveResponseHeadersLayer, L>>;

    /// Insert a header into the request.
    ///
    /// If a previous value exists for the same header, it is removed and replaced with the new
    /// header value.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn override_request_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>>;

    /// Append a header into the request.
    ///
    /// If previous values exist, the header will have multiple values.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn append_request_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>>;

    /// Insert a header into the request, if the header is not already present.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn insert_request_header_if_not_present<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>>;

    /// Insert a header into the response.
    ///
    /// If a previous value exists for the same header, it is removed and replaced with the new
    /// header value.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn override_response_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>>;

    /// Append a header into the response.
    ///
    /// If previous values exist, the header will have multiple values.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn append_response_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>>;

    /// Insert a header into the response, if the header is not already present.
    ///
    /// See [`tower_http::set_header`] for more details.
    ///
    /// [`tower_http::set_header`]: crate::set_header
    #[cfg(feature = "set-header")]
    fn insert_response_header_if_not_present<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>>;

    /// Add request id header and extension.
    ///
    /// See [`tower_http::request_id`] for more details.
    ///
    /// [`tower_http::request_id`]: crate::request_id
    #[cfg(feature = "request-id")]
    fn set_request_id<M>(
        self,
        header_name: HeaderName,
        make_request_id: M,
    ) -> ServiceBuilder<Stack<crate::request_id::SetRequestIdLayer<M>, L>>
    where
        M: crate::request_id::MakeRequestId;

    /// Add request id header and extension, using `x-request-id` as the header name.
    ///
    /// See [`tower_http::request_id`] for more details.
    ///
    /// [`tower_http::request_id`]: crate::request_id
    #[cfg(feature = "request-id")]
    fn set_x_request_id<M>(
        self,
        make_request_id: M,
    ) -> ServiceBuilder<Stack<crate::request_id::SetRequestIdLayer<M>, L>>
    where
        M: crate::request_id::MakeRequestId,
    {
        self.set_request_id(crate::request_id::X_REQUEST_ID, make_request_id)
    }

    /// Propgate request ids from requests to responses.
    ///
    /// See [`tower_http::request_id`] for more details.
    ///
    /// [`tower_http::request_id`]: crate::request_id
    #[cfg(feature = "request-id")]
    fn propagate_request_id(
        self,
        header_name: HeaderName,
    ) -> ServiceBuilder<Stack<crate::request_id::PropagateRequestIdLayer, L>>;

    /// Propgate request ids from requests to responses, using `x-request-id` as the header name.
    ///
    /// See [`tower_http::request_id`] for more details.
    ///
    /// [`tower_http::request_id`]: crate::request_id
    #[cfg(feature = "request-id")]
    fn propagate_x_request_id(
        self,
    ) -> ServiceBuilder<Stack<crate::request_id::PropagateRequestIdLayer, L>> {
        self.propagate_request_id(crate::request_id::X_REQUEST_ID)
    }

    /// Catch panics and convert them into `500 Internal Server` responses.
    ///
    /// See [`tower_http::catch_panic`] for more details.
    ///
    /// [`tower_http::catch_panic`]: crate::catch_panic
    #[cfg(feature = "catch-panic")]
    fn catch_panic(
        self,
    ) -> ServiceBuilder<
        Stack<crate::catch_panic::CatchPanicLayer<crate::catch_panic::DefaultResponseForPanic>, L>,
    >;

    /// Intercept requests with over-sized payloads and convert them into
    /// `413 Payload Too Large` responses.
    ///
    /// See [`tower_http::limit`] for more details.
    ///
    /// [`tower_http::limit`]: crate::limit
    #[cfg(feature = "limit")]
    fn request_body_limit(
        self,
        limit: usize,
    ) -> ServiceBuilder<Stack<crate::limit::RequestBodyLimitLayer, L>>;

    /// Remove trailing slashes from paths.
    ///
    /// See [`tower_http::normalize_path`] for more details.
    ///
    /// [`tower_http::normalize_path`]: crate::normalize_path
    #[cfg(feature = "normalize-path")]
    fn trim_trailing_slash(
        self,
    ) -> ServiceBuilder<Stack<crate::normalize_path::NormalizePathLayer, L>>;

    /// Append trailing slash to paths.
    ///
    /// See [`tower_http::normalize_path`] for more details.
    ///
    /// [`tower_http::normalize_path`]: crate::normalize_path
    #[cfg(feature = "normalize-path")]
    fn append_trailing_slash(
        self,
    ) -> ServiceBuilder<Stack<crate::normalize_path::NormalizePathLayer, L>>;
}

impl<L> sealed::Sealed<L> for ServiceBuilder<L> {}

impl<L> ServiceBuilderExt<L> for ServiceBuilder<L> {
    #[cfg(feature = "propagate-header")]
    fn propagate_header(
        self,
        header: HeaderName,
    ) -> ServiceBuilder<Stack<crate::propagate_header::PropagateHeaderLayer, L>> {
        self.layer(crate::propagate_header::PropagateHeaderLayer::new(header))
    }

    #[cfg(feature = "add-extension")]
    fn add_extension<T>(
        self,
        value: T,
    ) -> ServiceBuilder<Stack<crate::add_extension::AddExtensionLayer<T>, L>> {
        self.layer(crate::add_extension::AddExtensionLayer::new(value))
    }

    #[cfg(feature = "map-request-body")]
    fn map_request_body<F>(
        self,
        f: F,
    ) -> ServiceBuilder<Stack<crate::map_request_body::MapRequestBodyLayer<F>, L>> {
        self.layer(crate::map_request_body::MapRequestBodyLayer::new(f))
    }

    #[cfg(feature = "map-response-body")]
    fn map_response_body<F>(
        self,
        f: F,
    ) -> ServiceBuilder<Stack<crate::map_response_body::MapResponseBodyLayer<F>, L>> {
        self.layer(crate::map_response_body::MapResponseBodyLayer::new(f))
    }

    #[cfg(any(
        feature = "compression-br",
        feature = "compression-deflate",
        feature = "compression-gzip",
        feature = "compression-zstd",
    ))]
    fn compression(self) -> ServiceBuilder<Stack<crate::compression::CompressionLayer, L>> {
        self.layer(crate::compression::CompressionLayer::new())
    }

    #[cfg(any(
        feature = "decompression-br",
        feature = "decompression-deflate",
        feature = "decompression-gzip",
        feature = "decompression-zstd",
    ))]
    fn decompression(self) -> ServiceBuilder<Stack<crate::decompression::DecompressionLayer, L>> {
        self.layer(crate::decompression::DecompressionLayer::new())
    }

    #[cfg(feature = "trace")]
    fn trace_for_http(
        self,
    ) -> ServiceBuilder<Stack<crate::trace::TraceLayer<crate::trace::HttpMakeClassifier>, L>> {
        self.layer(crate::trace::TraceLayer::new_for_http())
    }

    #[cfg(feature = "trace")]
    fn trace_for_grpc(
        self,
    ) -> ServiceBuilder<Stack<crate::trace::TraceLayer<crate::trace::GrpcMakeClassifier>, L>> {
        self.layer(crate::trace::TraceLayer::new_for_grpc())
    }

    #[cfg(feature = "follow-redirect")]
    fn follow_redirects(
        self,
    ) -> ServiceBuilder<
        Stack<
            crate::follow_redirect::FollowRedirectLayer<crate::follow_redirect::policy::Standard>,
            L,
        >,
    > {
        self.layer(crate::follow_redirect::FollowRedirectLayer::new())
    }

    #[cfg(feature = "sensitive-headers")]
    fn sensitive_headers<I>(
        self,
        headers: I,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveHeadersLayer, L>>
    where
        I: IntoIterator<Item = HeaderName>,
    {
        self.layer(crate::sensitive_headers::SetSensitiveHeadersLayer::new(
            headers,
        ))
    }

    #[cfg(feature = "sensitive-headers")]
    fn sensitive_request_headers(
        self,
        headers: std::sync::Arc<[HeaderName]>,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveRequestHeadersLayer, L>> {
        self.layer(crate::sensitive_headers::SetSensitiveRequestHeadersLayer::from_shared(headers))
    }

    #[cfg(feature = "sensitive-headers")]
    fn sensitive_response_headers(
        self,
        headers: std::sync::Arc<[HeaderName]>,
    ) -> ServiceBuilder<Stack<crate::sensitive_headers::SetSensitiveResponseHeadersLayer, L>> {
        self.layer(crate::sensitive_headers::SetSensitiveResponseHeadersLayer::from_shared(headers))
    }

    #[cfg(feature = "set-header")]
    fn override_request_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetRequestHeaderLayer::overriding(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "set-header")]
    fn append_request_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetRequestHeaderLayer::appending(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "set-header")]
    fn insert_request_header_if_not_present<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetRequestHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetRequestHeaderLayer::if_not_present(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "set-header")]
    fn override_response_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetResponseHeaderLayer::overriding(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "set-header")]
    fn append_response_header<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetResponseHeaderLayer::appending(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "set-header")]
    fn insert_response_header_if_not_present<M>(
        self,
        header_name: HeaderName,
        make: M,
    ) -> ServiceBuilder<Stack<crate::set_header::SetResponseHeaderLayer<M>, L>> {
        self.layer(crate::set_header::SetResponseHeaderLayer::if_not_present(
            header_name,
            make,
        ))
    }

    #[cfg(feature = "request-id")]
    fn set_request_id<M>(
        self,
        header_name: HeaderName,
        make_request_id: M,
    ) -> ServiceBuilder<Stack<crate::request_id::SetRequestIdLayer<M>, L>>
    where
        M: crate::request_id::MakeRequestId,
    {
        self.layer(crate::request_id::SetRequestIdLayer::new(
            header_name,
            make_request_id,
        ))
    }

    #[cfg(feature = "request-id")]
    fn propagate_request_id(
        self,
        header_name: HeaderName,
    ) -> ServiceBuilder<Stack<crate::request_id::PropagateRequestIdLayer, L>> {
        self.layer(crate::request_id::PropagateRequestIdLayer::new(header_name))
    }

    #[cfg(feature = "catch-panic")]
    fn catch_panic(
        self,
    ) -> ServiceBuilder<
        Stack<crate::catch_panic::CatchPanicLayer<crate::catch_panic::DefaultResponseForPanic>, L>,
    > {
        self.layer(crate::catch_panic::CatchPanicLayer::new())
    }

    #[cfg(feature = "limit")]
    fn request_body_limit(
        self,
        limit: usize,
    ) -> ServiceBuilder<Stack<crate::limit::RequestBodyLimitLayer, L>> {
        self.layer(crate::limit::RequestBodyLimitLayer::new(limit))
    }

    #[cfg(feature = "normalize-path")]
    fn trim_trailing_slash(
        self,
    ) -> ServiceBuilder<Stack<crate::normalize_path::NormalizePathLayer, L>> {
        self.layer(crate::normalize_path::NormalizePathLayer::trim_trailing_slash())
    }

    #[cfg(feature = "normalize-path")]
    fn append_trailing_slash(
        self,
    ) -> ServiceBuilder<Stack<crate::normalize_path::NormalizePathLayer, L>> {
        self.layer(crate::normalize_path::NormalizePathLayer::append_trailing_slash())
    }
}
