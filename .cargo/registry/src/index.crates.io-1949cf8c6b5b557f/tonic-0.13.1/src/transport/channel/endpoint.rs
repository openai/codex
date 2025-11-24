#[cfg(feature = "_tls-any")]
use super::service::TlsConnector;
use super::service::{self, Executor, SharedExec};
use super::uds_connector::UdsConnector;
use super::Channel;
#[cfg(feature = "_tls-any")]
use super::ClientTlsConfig;
#[cfg(feature = "_tls-any")]
use crate::transport::error;
use crate::transport::Error;
use bytes::Bytes;
use http::{uri::Uri, HeaderValue};
use hyper::rt;
use hyper_util::client::legacy::connect::HttpConnector;
use std::{fmt, future::Future, net::IpAddr, pin::Pin, str, str::FromStr, time::Duration};
use tower_service::Service;

#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) enum EndpointType {
    Uri(Uri),
    Uds(String),
}

/// Channel builder.
///
/// This struct is used to build and configure HTTP/2 channels.
#[derive(Clone)]
pub struct Endpoint {
    pub(crate) uri: EndpointType,
    fallback_uri: Uri,
    pub(crate) origin: Option<Uri>,
    pub(crate) user_agent: Option<HeaderValue>,
    pub(crate) timeout: Option<Duration>,
    pub(crate) concurrency_limit: Option<usize>,
    pub(crate) rate_limit: Option<(u64, Duration)>,
    #[cfg(feature = "_tls-any")]
    pub(crate) tls: Option<TlsConnector>,
    pub(crate) buffer_size: Option<usize>,
    pub(crate) init_stream_window_size: Option<u32>,
    pub(crate) init_connection_window_size: Option<u32>,
    pub(crate) tcp_keepalive: Option<Duration>,
    pub(crate) tcp_nodelay: bool,
    pub(crate) http2_keep_alive_interval: Option<Duration>,
    pub(crate) http2_keep_alive_timeout: Option<Duration>,
    pub(crate) http2_keep_alive_while_idle: Option<bool>,
    pub(crate) http2_max_header_list_size: Option<u32>,
    pub(crate) connect_timeout: Option<Duration>,
    pub(crate) http2_adaptive_window: Option<bool>,
    pub(crate) local_address: Option<IpAddr>,
    pub(crate) executor: SharedExec,
}

impl Endpoint {
    // FIXME: determine if we want to expose this or not. This is really
    // just used in codegen for a shortcut.
    #[doc(hidden)]
    pub fn new<D>(dst: D) -> Result<Self, Error>
    where
        D: TryInto<Self>,
        D::Error: Into<crate::BoxError>,
    {
        let me = dst.try_into().map_err(|e| Error::from_source(e.into()))?;
        #[cfg(feature = "_tls-any")]
        if let EndpointType::Uri(uri) = &me.uri {
            if me.tls.is_none() && uri.scheme() == Some(&http::uri::Scheme::HTTPS) {
                return me.tls_config(ClientTlsConfig::new().with_enabled_roots());
            }
        }
        Ok(me)
    }

    fn new_uri(uri: Uri) -> Self {
        Self {
            uri: EndpointType::Uri(uri.clone()),
            fallback_uri: uri,
            origin: None,
            user_agent: None,
            concurrency_limit: None,
            rate_limit: None,
            timeout: None,
            #[cfg(feature = "_tls-any")]
            tls: None,
            buffer_size: None,
            init_stream_window_size: None,
            init_connection_window_size: None,
            tcp_keepalive: None,
            tcp_nodelay: true,
            http2_keep_alive_interval: None,
            http2_keep_alive_timeout: None,
            http2_keep_alive_while_idle: None,
            http2_max_header_list_size: None,
            connect_timeout: None,
            http2_adaptive_window: None,
            executor: SharedExec::tokio(),
            local_address: None,
        }
    }

    fn new_uds(uds_filepath: &str) -> Self {
        Self {
            uri: EndpointType::Uds(uds_filepath.to_string()),
            fallback_uri: Uri::from_static("http://tonic"),
            origin: None,
            user_agent: None,
            concurrency_limit: None,
            rate_limit: None,
            timeout: None,
            #[cfg(feature = "_tls-any")]
            tls: None,
            buffer_size: None,
            init_stream_window_size: None,
            init_connection_window_size: None,
            tcp_keepalive: None,
            tcp_nodelay: true,
            http2_keep_alive_interval: None,
            http2_keep_alive_timeout: None,
            http2_keep_alive_while_idle: None,
            http2_max_header_list_size: None,
            connect_timeout: None,
            http2_adaptive_window: None,
            executor: SharedExec::tokio(),
            local_address: None,
        }
    }

    /// Convert an `Endpoint` from a static string.
    ///
    /// # Panics
    ///
    /// This function panics if the argument is an invalid URI.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// Endpoint::from_static("https://example.com");
    /// ```
    pub fn from_static(s: &'static str) -> Self {
        if s.starts_with("unix:") {
            let uds_filepath = s
                .strip_prefix("unix://")
                .or_else(|| s.strip_prefix("unix:"))
                .expect("Invalid unix domain socket URI");
            Self::new_uds(uds_filepath)
        } else {
            let uri = Uri::from_static(s);
            Self::new_uri(uri)
        }
    }

    /// Convert an `Endpoint` from shared bytes.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// Endpoint::from_shared("https://example.com".to_string());
    /// ```
    pub fn from_shared(s: impl Into<Bytes>) -> Result<Self, Error> {
        let s = str::from_utf8(&s.into())
            .map_err(|e| Error::new_invalid_uri().with(e))?
            .to_string();
        if s.starts_with("unix:") {
            let uds_filepath = s
                .strip_prefix("unix://")
                .or_else(|| s.strip_prefix("unix:"))
                .ok_or(Error::new_invalid_uri())?;
            Ok(Self::new_uds(uds_filepath))
        } else {
            let uri = Uri::from_maybe_shared(s).map_err(|e| Error::new_invalid_uri().with(e))?;
            Ok(Self::from(uri))
        }
    }

    /// Set a custom user-agent header.
    ///
    /// `user_agent` will be prepended to Tonic's default user-agent string (`tonic/x.x.x`).
    /// It must be a value that can be converted into a valid  `http::HeaderValue` or building
    /// the endpoint will fail.
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.user_agent("Greeter").expect("Greeter should be a valid header value");
    /// // user-agent: "Greeter tonic/x.x.x"
    /// ```
    pub fn user_agent<T>(self, user_agent: T) -> Result<Self, Error>
    where
        T: TryInto<HeaderValue>,
    {
        user_agent
            .try_into()
            .map(|ua| Endpoint {
                user_agent: Some(ua),
                ..self
            })
            .map_err(|_| Error::new_invalid_user_agent())
    }

    /// Set a custom origin.
    ///
    /// Override the `origin`, mainly useful when you are reaching a Server/LoadBalancer
    /// which serves multiple services at the same time.
    /// It will play the role of SNI (Server Name Indication).
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # let mut builder = Endpoint::from_static("https://proxy.com");
    /// builder.origin("https://example.com".parse().expect("http://example.com must be a valid URI"));
    /// // origin: "https://example.com"
    /// ```
    pub fn origin(self, origin: Uri) -> Self {
        Endpoint {
            origin: Some(origin),
            ..self
        }
    }

    /// Apply a timeout to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use std::time::Duration;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.timeout(Duration::from_secs(5));
    /// ```
    ///
    /// # Notes
    ///
    /// This does **not** set the timeout metadata (`grpc-timeout` header) on
    /// the request, meaning the server will not be informed of this timeout,
    /// for that use [`Request::set_timeout`].
    ///
    /// [`Request::set_timeout`]: crate::Request::set_timeout
    pub fn timeout(self, dur: Duration) -> Self {
        Endpoint {
            timeout: Some(dur),
            ..self
        }
    }

    /// Apply a timeout to connecting to the uri.
    ///
    /// Defaults to no timeout.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use std::time::Duration;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.connect_timeout(Duration::from_secs(5));
    /// ```
    pub fn connect_timeout(self, dur: Duration) -> Self {
        Endpoint {
            connect_timeout: Some(dur),
            ..self
        }
    }

    /// Set whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    ///
    /// Default is no keepalive (`None`)
    ///
    pub fn tcp_keepalive(self, tcp_keepalive: Option<Duration>) -> Self {
        Endpoint {
            tcp_keepalive,
            ..self
        }
    }

    /// Apply a concurrency limit to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.concurrency_limit(256);
    /// ```
    pub fn concurrency_limit(self, limit: usize) -> Self {
        Endpoint {
            concurrency_limit: Some(limit),
            ..self
        }
    }

    /// Apply a rate limit to each request.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use std::time::Duration;
    /// # let mut builder = Endpoint::from_static("https://example.com");
    /// builder.rate_limit(32, Duration::from_secs(1));
    /// ```
    pub fn rate_limit(self, limit: u64, duration: Duration) -> Self {
        Endpoint {
            rate_limit: Some((limit, duration)),
            ..self
        }
    }

    /// Sets the [`SETTINGS_INITIAL_WINDOW_SIZE`][spec] option for HTTP2
    /// stream-level flow control.
    ///
    /// Default is 65,535
    ///
    /// [spec]: https://httpwg.org/specs/rfc9113.html#InitialWindowSize
    pub fn initial_stream_window_size(self, sz: impl Into<Option<u32>>) -> Self {
        Endpoint {
            init_stream_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is 65,535
    pub fn initial_connection_window_size(self, sz: impl Into<Option<u32>>) -> Self {
        Endpoint {
            init_connection_window_size: sz.into(),
            ..self
        }
    }

    /// Sets the tower service default internal buffer size
    ///
    /// Default is 1024
    pub fn buffer_size(self, sz: impl Into<Option<usize>>) -> Self {
        Endpoint {
            buffer_size: sz.into(),
            ..self
        }
    }

    /// Configures TLS for the endpoint.
    #[cfg(feature = "_tls-any")]
    pub fn tls_config(self, tls_config: ClientTlsConfig) -> Result<Self, Error> {
        match &self.uri {
            EndpointType::Uri(uri) => Ok(Endpoint {
                tls: Some(
                    tls_config
                        .into_tls_connector(uri)
                        .map_err(Error::from_source)?,
                ),
                ..self
            }),
            EndpointType::Uds(_) => Err(Error::new(error::Kind::InvalidTlsConfigForUds)),
        }
    }

    /// Set the value of `TCP_NODELAY` option for accepted connections. Enabled by default.
    pub fn tcp_nodelay(self, enabled: bool) -> Self {
        Endpoint {
            tcp_nodelay: enabled,
            ..self
        }
    }

    /// Set http2 KEEP_ALIVE_INTERVAL. Uses `hyper`'s default otherwise.
    pub fn http2_keep_alive_interval(self, interval: Duration) -> Self {
        Endpoint {
            http2_keep_alive_interval: Some(interval),
            ..self
        }
    }

    /// Set http2 KEEP_ALIVE_TIMEOUT. Uses `hyper`'s default otherwise.
    pub fn keep_alive_timeout(self, duration: Duration) -> Self {
        Endpoint {
            http2_keep_alive_timeout: Some(duration),
            ..self
        }
    }

    /// Set http2 KEEP_ALIVE_WHILE_IDLE. Uses `hyper`'s default otherwise.
    pub fn keep_alive_while_idle(self, enabled: bool) -> Self {
        Endpoint {
            http2_keep_alive_while_idle: Some(enabled),
            ..self
        }
    }

    /// Sets whether to use an adaptive flow control. Uses `hyper`'s default otherwise.
    pub fn http2_adaptive_window(self, enabled: bool) -> Self {
        Endpoint {
            http2_adaptive_window: Some(enabled),
            ..self
        }
    }

    /// Sets the max size of received header frames.
    ///
    /// This will default to whatever the default in hyper is. As of v1.4.1, it is 16 KiB.
    pub fn http2_max_header_list_size(self, size: u32) -> Self {
        Endpoint {
            http2_max_header_list_size: Some(size),
            ..self
        }
    }

    /// Sets the executor used to spawn async tasks.
    ///
    /// Uses `tokio::spawn` by default.
    pub fn executor<E>(mut self, executor: E) -> Self
    where
        E: Executor<Pin<Box<dyn Future<Output = ()> + Send>>> + Send + Sync + 'static,
    {
        self.executor = SharedExec::new(executor);
        self
    }

    pub(crate) fn connector<C>(&self, c: C) -> service::Connector<C> {
        service::Connector::new(
            c,
            #[cfg(feature = "_tls-any")]
            self.tls.clone(),
        )
    }

    /// Set the local address.
    ///
    /// This sets the IP address the client will use. By default we let hyper select the IP address.
    pub fn local_address(self, addr: Option<IpAddr>) -> Self {
        Endpoint {
            local_address: addr,
            ..self
        }
    }

    pub(crate) fn http_connector(&self) -> service::Connector<HttpConnector> {
        let mut http = HttpConnector::new();
        http.enforce_http(false);
        http.set_nodelay(self.tcp_nodelay);
        http.set_keepalive(self.tcp_keepalive);
        http.set_connect_timeout(self.connect_timeout);
        http.set_local_address(self.local_address);
        self.connector(http)
    }

    pub(crate) fn uds_connector(&self, uds_filepath: &str) -> service::Connector<UdsConnector> {
        self.connector(UdsConnector::new(uds_filepath))
    }

    /// Create a channel from this config.
    pub async fn connect(&self) -> Result<Channel, Error> {
        match &self.uri {
            EndpointType::Uri(_) => Channel::connect(self.http_connector(), self.clone()).await,
            EndpointType::Uds(uds_filepath) => {
                Channel::connect(self.uds_connector(uds_filepath.as_str()), self.clone()).await
            }
        }
    }

    /// Create a channel from this config.
    ///
    /// The channel returned by this method does not attempt to connect to the endpoint until first
    /// use.
    pub fn connect_lazy(&self) -> Channel {
        match &self.uri {
            EndpointType::Uri(_) => Channel::new(self.http_connector(), self.clone()),
            EndpointType::Uds(uds_filepath) => {
                Channel::new(self.uds_connector(uds_filepath.as_str()), self.clone())
            }
        }
    }

    /// Connect with a custom connector.
    ///
    /// This allows you to build a [Channel](struct.Channel.html) that uses a non-HTTP transport.
    /// See the `uds` example for an example on how to use this function to build channel that
    /// uses a Unix socket transport.
    ///
    /// The [`connect_timeout`](Endpoint::connect_timeout) will still be applied.
    pub async fn connect_with_connector<C>(&self, connector: C) -> Result<Channel, Error>
    where
        C: Service<Uri> + Send + 'static,
        C::Response: rt::Read + rt::Write + Send + Unpin,
        C::Future: Send,
        crate::BoxError: From<C::Error> + Send,
    {
        let connector = self.connector(connector);

        if let Some(connect_timeout) = self.connect_timeout {
            let mut connector = hyper_timeout::TimeoutConnector::new(connector);
            connector.set_connect_timeout(Some(connect_timeout));
            Channel::connect(connector, self.clone()).await
        } else {
            Channel::connect(connector, self.clone()).await
        }
    }

    /// Connect with a custom connector lazily.
    ///
    /// This allows you to build a [Channel](struct.Channel.html) that uses a non-HTTP transport
    /// connect to it lazily.
    ///
    /// See the `uds` example for an example on how to use this function to build channel that
    /// uses a Unix socket transport.
    pub fn connect_with_connector_lazy<C>(&self, connector: C) -> Channel
    where
        C: Service<Uri> + Send + 'static,
        C::Response: rt::Read + rt::Write + Send + Unpin,
        C::Future: Send,
        crate::BoxError: From<C::Error> + Send,
    {
        let connector = self.connector(connector);
        if let Some(connect_timeout) = self.connect_timeout {
            let mut connector = hyper_timeout::TimeoutConnector::new(connector);
            connector.set_connect_timeout(Some(connect_timeout));
            Channel::new(connector, self.clone())
        } else {
            Channel::new(connector, self.clone())
        }
    }

    /// Get the endpoint uri.
    ///
    /// ```
    /// # use tonic::transport::Endpoint;
    /// # use http::Uri;
    /// let endpoint = Endpoint::from_static("https://example.com");
    ///
    /// assert_eq!(endpoint.uri(), &Uri::from_static("https://example.com"));
    /// ```
    pub fn uri(&self) -> &Uri {
        match &self.uri {
            EndpointType::Uri(uri) => uri,
            EndpointType::Uds(_) => &self.fallback_uri,
        }
    }

    /// Get the value of `TCP_NODELAY` option for accepted connections.
    pub fn get_tcp_nodelay(&self) -> bool {
        self.tcp_nodelay
    }

    /// Get the connect timeout.
    pub fn get_connect_timeout(&self) -> Option<Duration> {
        self.connect_timeout
    }

    /// Get whether TCP keepalive messages are enabled on accepted connections.
    ///
    /// If `None` is specified, keepalive is disabled, otherwise the duration
    /// specified will be the time to remain idle before sending TCP keepalive
    /// probes.
    pub fn get_tcp_keepalive(&self) -> Option<Duration> {
        self.tcp_keepalive
    }
}

impl From<Uri> for Endpoint {
    fn from(uri: Uri) -> Self {
        Self::new_uri(uri)
    }
}

impl TryFrom<Bytes> for Endpoint {
    type Error = Error;

    fn try_from(t: Bytes) -> Result<Self, Self::Error> {
        Self::from_shared(t)
    }
}

impl TryFrom<String> for Endpoint {
    type Error = Error;

    fn try_from(t: String) -> Result<Self, Self::Error> {
        Self::from_shared(t.into_bytes())
    }
}

impl TryFrom<&'static str> for Endpoint {
    type Error = Error;

    fn try_from(t: &'static str) -> Result<Self, Self::Error> {
        Self::from_shared(t.as_bytes())
    }
}

impl fmt::Debug for Endpoint {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Endpoint").finish()
    }
}

impl FromStr for Endpoint {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Self::try_from(s.to_string())
    }
}
