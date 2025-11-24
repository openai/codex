use crate::metadata::{MetadataMap, MetadataValue};
#[cfg(feature = "server")]
use crate::transport::server::TcpConnectInfo;
#[cfg(all(feature = "server", feature = "_tls-any"))]
use crate::transport::server::TlsConnectInfo;
use http::Extensions;
#[cfg(feature = "server")]
use std::net::SocketAddr;
#[cfg(all(feature = "server", feature = "_tls-any"))]
use std::sync::Arc;
use std::time::Duration;
#[cfg(all(feature = "server", feature = "_tls-any"))]
use tokio_rustls::rustls::pki_types::CertificateDer;
use tokio_stream::Stream;

/// A gRPC request and metadata from an RPC call.
#[derive(Debug)]
pub struct Request<T> {
    metadata: MetadataMap,
    message: T,
    extensions: Extensions,
}

/// Trait implemented by RPC request types.
///
/// Types implementing this trait can be used as arguments to client RPC
/// methods without explicitly wrapping them into `tonic::Request`s. The purpose
/// is to make client calls slightly more convenient to write.
///
/// Tonic's code generation and blanket implementations handle this for you,
/// so it is not necessary to implement this trait directly.
///
/// # Example
///
/// Given the following gRPC method definition:
/// ```proto
/// rpc GetFeature(Point) returns (Feature) {}
/// ```
///
/// we can call `get_feature` in two equivalent ways:
/// ```rust
/// # pub struct Point {}
/// # pub struct Client {}
/// # impl Client {
/// #   fn get_feature(&self, r: impl tonic::IntoRequest<Point>) {}
/// # }
/// # let client = Client {};
/// use tonic::Request;
///
/// client.get_feature(Point {});
/// client.get_feature(Request::new(Point {}));
/// ```
pub trait IntoRequest<T>: sealed::Sealed {
    /// Wrap the input message `T` in a `tonic::Request`
    fn into_request(self) -> Request<T>;
}

/// Trait implemented by RPC streaming request types.
///
/// Types implementing this trait can be used as arguments to client streaming
/// RPC methods without explicitly wrapping them into `tonic::Request`s. The
/// purpose is to make client calls slightly more convenient to write.
///
/// Tonic's code generation and blanket implementations handle this for you,
/// so it is not necessary to implement this trait directly.
///
/// # Example
///
/// Given the following gRPC service method definition:
/// ```proto
/// rpc RecordRoute(stream Point) returns (RouteSummary) {}
/// ```
/// we can call `record_route` in two equivalent ways:
///
/// ```rust
/// # #[derive(Clone)]
/// # pub struct Point {};
/// # pub struct Client {};
/// # impl Client {
/// #   fn record_route(&self, r: impl tonic::IntoStreamingRequest<Message = Point>) {}
/// # }
/// # let client = Client {};
/// use tonic::Request;
///
/// let messages = vec![Point {}, Point {}];
///
/// client.record_route(Request::new(tokio_stream::iter(messages.clone())));
/// client.record_route(tokio_stream::iter(messages));
/// ```
pub trait IntoStreamingRequest: sealed::Sealed {
    /// The RPC request stream type
    type Stream: Stream<Item = Self::Message> + Send + 'static;

    /// The RPC request type
    type Message;

    /// Wrap the stream of messages in a `tonic::Request`
    fn into_streaming_request(self) -> Request<Self::Stream>;
}

impl<T> Request<T> {
    /// Create a new gRPC request.
    ///
    /// ```rust
    /// # use tonic::Request;
    /// # pub struct HelloRequest {
    /// #   pub name: String,
    /// # }
    /// Request::new(HelloRequest {
    ///    name: "Bob".into(),
    /// });
    /// ```
    pub fn new(message: T) -> Self {
        Request {
            metadata: MetadataMap::new(),
            message,
            extensions: Extensions::new(),
        }
    }

    /// Get a reference to the message
    pub fn get_ref(&self) -> &T {
        &self.message
    }

    /// Get a mutable reference to the message
    pub fn get_mut(&mut self) -> &mut T {
        &mut self.message
    }

    /// Get a reference to the custom request metadata.
    pub fn metadata(&self) -> &MetadataMap {
        &self.metadata
    }

    /// Get a mutable reference to the request metadata.
    pub fn metadata_mut(&mut self) -> &mut MetadataMap {
        &mut self.metadata
    }

    /// Consumes `self`, returning the message
    pub fn into_inner(self) -> T {
        self.message
    }

    /// Consumes `self` returning the parts of the request.
    pub fn into_parts(self) -> (MetadataMap, Extensions, T) {
        (self.metadata, self.extensions, self.message)
    }

    /// Create a new gRPC request from metadata, extensions and message.
    pub fn from_parts(metadata: MetadataMap, extensions: Extensions, message: T) -> Self {
        Self {
            metadata,
            extensions,
            message,
        }
    }

    pub(crate) fn from_http_parts(parts: http::request::Parts, message: T) -> Self {
        Request {
            metadata: MetadataMap::from_headers(parts.headers),
            message,
            extensions: parts.extensions,
        }
    }

    /// Convert an HTTP request to a gRPC request
    pub fn from_http(http: http::Request<T>) -> Self {
        let (parts, message) = http.into_parts();
        Request::from_http_parts(parts, message)
    }

    pub(crate) fn into_http(
        self,
        uri: http::Uri,
        method: http::Method,
        version: http::Version,
        sanitize_headers: SanitizeHeaders,
    ) -> http::Request<T> {
        let mut request = http::Request::new(self.message);

        *request.version_mut() = version;
        *request.method_mut() = method;
        *request.uri_mut() = uri;
        *request.headers_mut() = match sanitize_headers {
            SanitizeHeaders::Yes => self.metadata.into_sanitized_headers(),
            SanitizeHeaders::No => self.metadata.into_headers(),
        };
        *request.extensions_mut() = self.extensions;

        request
    }

    #[doc(hidden)]
    pub fn map<F, U>(self, f: F) -> Request<U>
    where
        F: FnOnce(T) -> U,
    {
        let message = f(self.message);

        Request {
            metadata: self.metadata,
            message,
            extensions: self.extensions,
        }
    }

    /// Get the local address of this connection.
    ///
    /// This will return `None` if the `IO` type used
    /// does not implement `Connected` or when using a unix domain socket.
    /// This currently only works on the server side.
    #[cfg(feature = "server")]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        let addr = self
            .extensions()
            .get::<TcpConnectInfo>()
            .and_then(|i| i.local_addr());

        #[cfg(feature = "_tls-any")]
        let addr = addr.or_else(|| {
            self.extensions()
                .get::<TlsConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.get_ref().local_addr())
        });

        addr
    }

    /// Get the remote address of this connection.
    ///
    /// This will return `None` if the `IO` type used
    /// does not implement `Connected` or when using a unix domain socket.
    /// This currently only works on the server side.
    #[cfg(feature = "server")]
    pub fn remote_addr(&self) -> Option<SocketAddr> {
        let addr = self
            .extensions()
            .get::<TcpConnectInfo>()
            .and_then(|i| i.remote_addr());

        #[cfg(feature = "_tls-any")]
        let addr = addr.or_else(|| {
            self.extensions()
                .get::<TlsConnectInfo<TcpConnectInfo>>()
                .and_then(|i| i.get_ref().remote_addr())
        });

        addr
    }

    /// Get the peer certificates of the connected client.
    ///
    /// This is used to fetch the certificates from the TLS session
    /// and is mostly used for mTLS. This currently only returns
    /// `Some` on the server side of the `transport` server with
    /// TLS enabled connections.
    #[cfg(all(feature = "server", feature = "_tls-any"))]
    pub fn peer_certs(&self) -> Option<Arc<Vec<CertificateDer<'static>>>> {
        self.extensions()
            .get::<TlsConnectInfo<TcpConnectInfo>>()
            .and_then(|i| i.peer_certs())
    }

    /// Set the max duration the request is allowed to take.
    ///
    /// Requires the server to support the `grpc-timeout` metadata, which Tonic does.
    ///
    /// The duration will be formatted according to [the spec] and use the most precise unit
    /// possible.
    ///
    /// Example:
    ///
    /// ```rust
    /// use std::time::Duration;
    /// use tonic::Request;
    ///
    /// let mut request = Request::new(());
    ///
    /// request.set_timeout(Duration::from_secs(30));
    ///
    /// let value = request.metadata().get("grpc-timeout").unwrap();
    ///
    /// assert_eq!(
    ///     value,
    ///     // equivalent to 30 seconds
    ///     "30000000u"
    /// );
    /// ```
    ///
    /// [the spec]: https://github.com/grpc/grpc/blob/master/doc/PROTOCOL-HTTP2.md
    pub fn set_timeout(&mut self, deadline: Duration) {
        let value: MetadataValue<_> = duration_to_grpc_timeout(deadline).parse().unwrap();
        self.metadata_mut()
            .insert(crate::metadata::GRPC_TIMEOUT_HEADER, value);
    }

    /// Returns a reference to the associated extensions.
    pub fn extensions(&self) -> &Extensions {
        &self.extensions
    }

    /// Returns a mutable reference to the associated extensions.
    ///
    /// # Example
    ///
    /// Extensions can be set in interceptors:
    ///
    /// ```no_run
    /// use tonic::{Request, Status};
    ///
    /// #[derive(Clone)] // Extensions must be Clone
    /// struct MyExtension {
    ///     some_piece_of_data: String,
    /// }
    ///
    /// fn intercept(mut request: Request<()>) -> Result<Request<()>, Status> {
    ///     request.extensions_mut().insert(MyExtension {
    ///         some_piece_of_data: "foo".to_string(),
    ///     });
    ///
    ///     Ok(request)
    /// }
    /// ```
    ///
    /// And picked up by RPCs:
    ///
    /// ```no_run
    /// use tonic::{async_trait, Status, Request, Response};
    /// #
    /// # struct Output {}
    /// # struct Input;
    /// # struct MyService;
    /// # struct MyExtension;
    /// # #[async_trait]
    /// # trait TestService {
    /// #     async fn handler(&self, req: Request<Input>) -> Result<Response<Output>, Status>;
    /// # }
    ///
    /// #[async_trait]
    /// impl TestService for MyService {
    ///     async fn handler(&self, req: Request<Input>) -> Result<Response<Output>, Status> {
    ///         let value: &MyExtension = req.extensions().get::<MyExtension>().unwrap();
    ///
    ///         Ok(Response::new(Output {}))
    ///     }
    /// }
    /// ```
    pub fn extensions_mut(&mut self) -> &mut Extensions {
        &mut self.extensions
    }
}

impl<T> IntoRequest<T> for T {
    fn into_request(self) -> Request<Self> {
        Request::new(self)
    }
}

impl<T> IntoRequest<T> for Request<T> {
    fn into_request(self) -> Request<T> {
        self
    }
}

impl<T> IntoStreamingRequest for T
where
    T: Stream + Send + 'static,
{
    type Stream = T;
    type Message = T::Item;

    fn into_streaming_request(self) -> Request<Self> {
        Request::new(self)
    }
}

impl<T> IntoStreamingRequest for Request<T>
where
    T: Stream + Send + 'static,
{
    type Stream = T;
    type Message = T::Item;

    fn into_streaming_request(self) -> Self {
        self
    }
}

impl<T> sealed::Sealed for T {}

mod sealed {
    pub trait Sealed {}
}

fn duration_to_grpc_timeout(duration: Duration) -> String {
    fn try_format<T: Into<u128>>(
        duration: Duration,
        unit: char,
        convert: impl FnOnce(Duration) -> T,
    ) -> Option<String> {
        // The gRPC spec specifies that the timeout most be at most 8 digits. So this is the largest a
        // value can be before we need to use a bigger unit.
        let max_size: u128 = 99_999_999; // exactly 8 digits

        let value = convert(duration).into();
        if value > max_size {
            None
        } else {
            Some(format!("{value}{unit}"))
        }
    }

    // pick the most precise unit that is less than or equal to 8 digits as per the gRPC spec
    try_format(duration, 'n', |d| d.as_nanos())
        .or_else(|| try_format(duration, 'u', |d| d.as_micros()))
        .or_else(|| try_format(duration, 'm', |d| d.as_millis()))
        .or_else(|| try_format(duration, 'S', |d| d.as_secs()))
        .or_else(|| try_format(duration, 'M', |d| d.as_secs() / 60))
        .or_else(|| {
            try_format(duration, 'H', |d| {
                let minutes = d.as_secs() / 60;
                minutes / 60
            })
        })
        // duration has to be more than 11_415 years for this to happen
        .expect("duration is unrealistically large")
}

/// When converting a `tonic::Request` into a `http::Request` should reserved
/// headers be removed?
pub(crate) enum SanitizeHeaders {
    Yes,
    No,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::{MetadataKey, MetadataValue};

    use http::Uri;

    #[test]
    fn reserved_headers_are_excluded() {
        let mut r = Request::new(1);

        for header in &MetadataMap::GRPC_RESERVED_HEADERS {
            r.metadata_mut().insert(
                MetadataKey::unchecked_from_header_name(header.clone()),
                MetadataValue::from_static("invalid"),
            );
        }

        let http_request = r.into_http(
            Uri::default(),
            http::Method::POST,
            http::Version::HTTP_2,
            SanitizeHeaders::Yes,
        );
        assert!(http_request.headers().is_empty());
    }

    #[test]
    fn duration_to_grpc_timeout_less_than_second() {
        let timeout = Duration::from_millis(500);
        let value = duration_to_grpc_timeout(timeout);
        assert_eq!(value, format!("{}u", timeout.as_micros()));
    }

    #[test]
    fn duration_to_grpc_timeout_more_than_second() {
        let timeout = Duration::from_secs(30);
        let value = duration_to_grpc_timeout(timeout);
        assert_eq!(value, format!("{}u", timeout.as_micros()));
    }

    #[test]
    fn duration_to_grpc_timeout_a_very_long_time() {
        let one_hour = Duration::from_secs(60 * 60);
        let value = duration_to_grpc_timeout(one_hour);
        assert_eq!(value, format!("{}m", one_hour.as_millis()));
    }
}
