#[cfg(any(feature = "native-tls", feature = "__rustls",))]
use std::any::Any;
use std::convert::TryInto;
use std::fmt;
use std::future::Future;
use std::net::IpAddr;
use std::net::SocketAddr;
use std::sync::Arc;
use std::task::{ready, Poll};
use std::thread;
use std::time::Duration;

use http::header::HeaderValue;
use log::{error, trace};
use tokio::sync::{mpsc, oneshot};
use tower::Layer;
use tower::Service;

use super::request::{Request, RequestBuilder};
use super::response::Response;
use super::wait;
use crate::connect::sealed::{Conn, Unnameable};
#[cfg(unix)]
use crate::connect::uds::UnixSocketProvider;
use crate::connect::BoxedConnectorService;
use crate::dns::Resolve;
use crate::error::BoxError;
#[cfg(feature = "__tls")]
use crate::tls;
#[cfg(feature = "__rustls")]
use crate::tls::CertificateRevocationList;
#[cfg(feature = "__tls")]
use crate::Certificate;
#[cfg(any(feature = "native-tls", feature = "__rustls"))]
use crate::Identity;
use crate::{async_impl, header, redirect, IntoUrl, Method, Proxy};

/// A `Client` to make Requests with.
///
/// The Client has various configuration values to tweak, but the defaults
/// are set to what is usually the most commonly desired value. To configure a
/// `Client`, use `Client::builder()`.
///
/// The `Client` holds a connection pool internally, so it is advised that
/// you create one and **reuse** it.
///
/// # Examples
///
/// ```rust
/// use reqwest::blocking::Client;
/// #
/// # fn run() -> Result<(), reqwest::Error> {
/// let client = Client::new();
/// let resp = client.get("http://httpbin.org/").send()?;
/// #   drop(resp);
/// #   Ok(())
/// # }
///
/// ```
#[derive(Clone)]
pub struct Client {
    inner: ClientHandle,
}

/// A `ClientBuilder` can be used to create a `Client` with  custom configuration.
///
/// # Example
///
/// ```
/// # fn run() -> Result<(), reqwest::Error> {
/// use std::time::Duration;
///
/// let client = reqwest::blocking::Client::builder()
///     .timeout(Duration::from_secs(10))
///     .build()?;
/// # Ok(())
/// # }
/// ```
#[must_use]
pub struct ClientBuilder {
    inner: async_impl::ClientBuilder,
    timeout: Timeout,
}

impl Default for ClientBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientBuilder {
    /// Constructs a new `ClientBuilder`.
    ///
    /// This is the same as `Client::builder()`.
    pub fn new() -> Self {
        ClientBuilder {
            inner: async_impl::ClientBuilder::new(),
            timeout: Timeout::default(),
        }
    }
}

impl ClientBuilder {
    /// Returns a `Client` that uses this `ClientBuilder` configuration.
    ///
    /// # Errors
    ///
    /// This method fails if TLS backend cannot be initialized, or the resolver
    /// cannot load the system configuration.
    ///
    /// # Panics
    ///
    /// This method panics if called from within an async runtime. See docs on
    /// [`reqwest::blocking`][crate::blocking] for details.
    pub fn build(self) -> crate::Result<Client> {
        ClientHandle::new(self).map(|handle| Client { inner: handle })
    }

    // Higher-level options

    /// Sets the `User-Agent` header to be used by this client.
    ///
    /// # Example
    ///
    /// ```rust
    /// # fn doc() -> Result<(), reqwest::Error> {
    /// // Name your user agent after your app?
    /// static APP_USER_AGENT: &str = concat!(
    ///     env!("CARGO_PKG_NAME"),
    ///     "/",
    ///     env!("CARGO_PKG_VERSION"),
    /// );
    ///
    /// let client = reqwest::blocking::Client::builder()
    ///     .user_agent(APP_USER_AGENT)
    ///     .build()?;
    /// let res = client.get("https://www.rust-lang.org").send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn user_agent<V>(self, value: V) -> ClientBuilder
    where
        V: TryInto<HeaderValue>,
        V::Error: Into<http::Error>,
    {
        self.with_inner(move |inner| inner.user_agent(value))
    }

    /// Sets the default headers for every request.
    ///
    /// # Example
    ///
    /// ```rust
    /// use reqwest::header;
    /// # fn build_client() -> Result<(), reqwest::Error> {
    /// let mut headers = header::HeaderMap::new();
    /// headers.insert("X-MY-HEADER", header::HeaderValue::from_static("value"));
    /// headers.insert(header::AUTHORIZATION, header::HeaderValue::from_static("secret"));
    ///
    /// // Consider marking security-sensitive headers with `set_sensitive`.
    /// let mut auth_value = header::HeaderValue::from_static("secret");
    /// auth_value.set_sensitive(true);
    /// headers.insert(header::AUTHORIZATION, auth_value);
    ///
    /// // get a client builder
    /// let client = reqwest::blocking::Client::builder()
    ///     .default_headers(headers)
    ///     .build()?;
    /// let res = client.get("https://www.rust-lang.org").send()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn default_headers(self, headers: header::HeaderMap) -> ClientBuilder {
        self.with_inner(move |inner| inner.default_headers(headers))
    }

    /// Enable a persistent cookie store for the client.
    ///
    /// Cookies received in responses will be preserved and included in
    /// additional requests.
    ///
    /// By default, no cookie store is used.
    ///
    /// # Optional
    ///
    /// This requires the optional `cookies` feature to be enabled.
    #[cfg(feature = "cookies")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
    pub fn cookie_store(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.cookie_store(enable))
    }

    /// Set the persistent cookie store for the client.
    ///
    /// Cookies received in responses will be passed to this store, and
    /// additional requests will query this store for cookies.
    ///
    /// By default, no cookie store is used.
    ///
    /// # Optional
    ///
    /// This requires the optional `cookies` feature to be enabled.
    #[cfg(feature = "cookies")]
    #[cfg_attr(docsrs, doc(cfg(feature = "cookies")))]
    pub fn cookie_provider<C: crate::cookie::CookieStore + 'static>(
        self,
        cookie_store: Arc<C>,
    ) -> ClientBuilder {
        self.with_inner(|inner| inner.cookie_provider(cookie_store))
    }

    /// Enable auto gzip decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto gzip decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `gzip`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if it's headers contain a `Content-Encoding` value that
    ///   equals to `gzip`, both values `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `gzip` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `gzip` feature to be enabled
    #[cfg(feature = "gzip")]
    #[cfg_attr(docsrs, doc(cfg(feature = "gzip")))]
    pub fn gzip(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.gzip(enable))
    }

    /// Enable auto brotli decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto brotli decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `br`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if it's headers contain a `Content-Encoding` value that
    ///   equals to `br`, both values `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `brotli` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `brotli` feature to be enabled
    #[cfg(feature = "brotli")]
    #[cfg_attr(docsrs, doc(cfg(feature = "brotli")))]
    pub fn brotli(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.brotli(enable))
    }

    /// Enable auto zstd decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto zstd decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `zstd`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if its headers contain a `Content-Encoding` value of
    ///   `zstd`, both `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `zstd` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `zstd` feature to be enabled
    #[cfg(feature = "zstd")]
    #[cfg_attr(docsrs, doc(cfg(feature = "zstd")))]
    pub fn zstd(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.zstd(enable))
    }

    /// Enable auto deflate decompression by checking the `Content-Encoding` response header.
    ///
    /// If auto deflate decompression is turned on:
    ///
    /// - When sending a request and if the request's headers do not already contain
    ///   an `Accept-Encoding` **and** `Range` values, the `Accept-Encoding` header is set to `deflate`.
    ///   The request body is **not** automatically compressed.
    /// - When receiving a response, if it's headers contain a `Content-Encoding` value that
    ///   equals to `deflate`, both values `Content-Encoding` and `Content-Length` are removed from the
    ///   headers' set. The response body is automatically decompressed.
    ///
    /// If the `deflate` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `deflate` feature to be enabled
    #[cfg(feature = "deflate")]
    #[cfg_attr(docsrs, doc(cfg(feature = "deflate")))]
    pub fn deflate(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.deflate(enable))
    }

    /// Disable auto response body gzip decompression.
    ///
    /// This method exists even if the optional `gzip` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use gzip decompression
    /// even if another dependency were to enable the optional `gzip` feature.
    pub fn no_gzip(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_gzip())
    }

    /// Disable auto response body brotli decompression.
    ///
    /// This method exists even if the optional `brotli` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use brotli decompression
    /// even if another dependency were to enable the optional `brotli` feature.
    pub fn no_brotli(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_brotli())
    }

    /// Disable auto response body zstd decompression.
    ///
    /// This method exists even if the optional `zstd` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use zstd decompression
    /// even if another dependency were to enable the optional `zstd` feature.
    pub fn no_zstd(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_zstd())
    }

    /// Disable auto response body deflate decompression.
    ///
    /// This method exists even if the optional `deflate` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use deflate decompression
    /// even if another dependency were to enable the optional `deflate` feature.
    pub fn no_deflate(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_deflate())
    }

    // Redirect options

    /// Set a `redirect::Policy` for this client.
    ///
    /// Default will follow redirects up to a maximum of 10.
    pub fn redirect(self, policy: redirect::Policy) -> ClientBuilder {
        self.with_inner(move |inner| inner.redirect(policy))
    }

    /// Set a request retry policy.
    ///
    /// Default behavior is to retry protocol NACKs.
    pub fn retry(self, policy: crate::retry::Builder) -> ClientBuilder {
        self.with_inner(move |inner| inner.retry(policy))
    }

    /// Enable or disable automatic setting of the `Referer` header.
    ///
    /// Default is `true`.
    pub fn referer(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.referer(enable))
    }

    // Proxy options

    /// Add a `Proxy` to the list of proxies the `Client` will use.
    ///
    /// # Note
    ///
    /// Adding a proxy will disable the automatic usage of the "system" proxy.
    pub fn proxy(self, proxy: Proxy) -> ClientBuilder {
        self.with_inner(move |inner| inner.proxy(proxy))
    }

    /// Clear all `Proxies`, so `Client` will use no proxy anymore.
    ///
    /// # Note
    /// To add a proxy exclusion list, use [Proxy::no_proxy()]
    /// on all desired proxies instead.
    ///
    /// This also disables the automatic usage of the "system" proxy.
    pub fn no_proxy(self) -> ClientBuilder {
        self.with_inner(move |inner| inner.no_proxy())
    }

    // Timeout options

    /// Set a timeout for connect, read and write operations of a `Client`.
    ///
    /// Default is 30 seconds.
    ///
    /// Pass `None` to disable timeout.
    pub fn timeout<T>(mut self, timeout: T) -> ClientBuilder
    where
        T: Into<Option<Duration>>,
    {
        self.timeout = Timeout(timeout.into());
        self
    }

    /// Set a timeout for only the connect phase of a `Client`.
    ///
    /// Default is `None`.
    pub fn connect_timeout<T>(self, timeout: T) -> ClientBuilder
    where
        T: Into<Option<Duration>>,
    {
        let timeout = timeout.into();
        if let Some(dur) = timeout {
            self.with_inner(|inner| inner.connect_timeout(dur))
        } else {
            self
        }
    }

    /// Set whether connections should emit verbose logs.
    ///
    /// Enabling this option will emit [log][] messages at the `TRACE` level
    /// for read and write operations on connections.
    ///
    /// [log]: https://crates.io/crates/log
    pub fn connection_verbose(self, verbose: bool) -> ClientBuilder {
        self.with_inner(move |inner| inner.connection_verbose(verbose))
    }

    // HTTP options

    /// Set an optional timeout for idle sockets being kept-alive.
    ///
    /// Pass `None` to disable timeout.
    ///
    /// Default is 90 seconds.
    pub fn pool_idle_timeout<D>(self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.with_inner(|inner| inner.pool_idle_timeout(val))
    }

    /// Sets the maximum idle connection per host allowed in the pool.
    pub fn pool_max_idle_per_host(self, max: usize) -> ClientBuilder {
        self.with_inner(move |inner| inner.pool_max_idle_per_host(max))
    }

    /// Send headers as title case instead of lowercase.
    pub fn http1_title_case_headers(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http1_title_case_headers())
    }

    /// Set whether HTTP/1 connections will accept obsolete line folding for
    /// header values.
    ///
    /// Newline codepoints (`\r` and `\n`) will be transformed to spaces when
    /// parsing.
    pub fn http1_allow_obsolete_multiline_headers_in_responses(self, value: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.http1_allow_obsolete_multiline_headers_in_responses(value))
    }

    /// Sets whether invalid header lines should be silently ignored in HTTP/1 responses.
    pub fn http1_ignore_invalid_headers_in_responses(self, value: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.http1_ignore_invalid_headers_in_responses(value))
    }

    /// Set whether HTTP/1 connections will accept spaces between header
    /// names and the colon that follow them in responses.
    ///
    /// Newline codepoints (\r and \n) will be transformed to spaces when
    /// parsing.
    pub fn http1_allow_spaces_after_header_name_in_responses(self, value: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.http1_allow_spaces_after_header_name_in_responses(value))
    }

    /// Only use HTTP/1.
    pub fn http1_only(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http1_only())
    }

    /// Allow HTTP/0.9 responses
    pub fn http09_responses(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http09_responses())
    }

    /// Only use HTTP/2.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_prior_knowledge(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_prior_knowledge())
    }

    /// Sets the `SETTINGS_INITIAL_WINDOW_SIZE` option for HTTP2 stream-level flow control.
    ///
    /// Default is currently 65,535 but may change internally to optimize for common uses.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_initial_stream_window_size(self, sz: impl Into<Option<u32>>) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_initial_stream_window_size(sz))
    }

    /// Sets the max connection-level flow control for HTTP2
    ///
    /// Default is currently 65,535 but may change internally to optimize for common uses.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_initial_connection_window_size(self, sz: impl Into<Option<u32>>) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_initial_connection_window_size(sz))
    }

    /// Sets whether to use an adaptive flow control.
    ///
    /// Enabling this will override the limits set in `http2_initial_stream_window_size` and
    /// `http2_initial_connection_window_size`.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_adaptive_window(self, enabled: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_adaptive_window(enabled))
    }

    /// Sets the maximum frame size to use for HTTP2.
    ///
    /// Default is currently 16,384 but may change internally to optimize for common uses.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_frame_size(self, sz: impl Into<Option<u32>>) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_max_frame_size(sz))
    }

    /// Sets the maximum size of received header frames for HTTP2.
    ///
    /// Default is currently 16KB, but can change.
    #[cfg(feature = "http2")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http2")))]
    pub fn http2_max_header_list_size(self, max_header_size_bytes: u32) -> ClientBuilder {
        self.with_inner(|inner| inner.http2_max_header_list_size(max_header_size_bytes))
    }

    /// This requires the optional `http3` feature to be
    /// enabled.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(feature = "http3")))]
    pub fn http3_prior_knowledge(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_prior_knowledge())
    }

    /// Maximum duration of inactivity to accept before timing out the QUIC connection.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_max_idle_timeout(self, value: Duration) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_max_idle_timeout(value))
    }

    /// Maximum number of bytes the peer may transmit without acknowledgement on any one stream
    /// before becoming blocked.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    ///
    /// # Panics
    ///
    /// Panics if the value is over 2^62.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_stream_receive_window(self, value: u64) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_stream_receive_window(value))
    }

    /// Maximum number of bytes the peer may transmit across all streams of a connection before
    /// becoming blocked.
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    ///
    /// # Panics
    ///
    /// Panics if the value is over 2^62.
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_conn_receive_window(self, value: u64) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_conn_receive_window(value))
    }

    /// Maximum number of bytes to transmit to a peer without acknowledgment
    ///
    /// Please see docs in [`TransportConfig`] in [`quinn`].
    ///
    /// [`TransportConfig`]: https://docs.rs/quinn/latest/quinn/struct.TransportConfig.html
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_send_window(self, value: u64) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_send_window(value))
    }

    /// Override the default congestion control algorithm to use [BBR]
    ///
    /// The current default congestion control algorithm is [CUBIC]. This method overrides the
    /// default.
    ///
    /// [BBR]: https://datatracker.ietf.org/doc/html/draft-ietf-ccwg-bbr
    /// [CUBIC]: https://datatracker.ietf.org/doc/html/rfc8312
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_congestion_bbr(self) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_congestion_bbr())
    }

    /// Set the maximum HTTP/3 header size this client is willing to accept.
    ///
    /// See [header size constraints] section of the specification for details.
    ///
    /// [header size constraints]: https://www.rfc-editor.org/rfc/rfc9114.html#name-header-size-constraints
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.max_field_section_size
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_max_field_section_size(self, value: u64) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_max_field_section_size(value))
    }

    /// Enable whether to send HTTP/3 protocol grease on the connections.
    ///
    /// HTTP/3 uses the concept of "grease"
    ///
    /// to prevent potential interoperability issues in the future.
    /// In HTTP/3, the concept of grease is used to ensure that the protocol can evolve
    /// and accommodate future changes without breaking existing implementations.
    ///
    /// Please see docs in [`Builder`] in [`h3`].
    ///
    /// [`Builder`]: https://docs.rs/h3/latest/h3/client/struct.Builder.html#method.send_grease
    #[cfg(feature = "http3")]
    #[cfg_attr(docsrs, doc(cfg(all(reqwest_unstable, feature = "http3",))))]
    pub fn http3_send_grease(self, enabled: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.http3_send_grease(enabled))
    }

    // TCP options

    /// Set whether sockets have `TCP_NODELAY` enabled.
    ///
    /// Default is `true`.
    pub fn tcp_nodelay(self, enabled: bool) -> ClientBuilder {
        self.with_inner(move |inner| inner.tcp_nodelay(enabled))
    }

    /// Bind to a local IP Address.
    ///
    /// # Example
    ///
    /// ```
    /// use std::net::IpAddr;
    /// let local_addr = IpAddr::from([12, 4, 1, 8]);
    /// let client = reqwest::blocking::Client::builder()
    ///     .local_address(local_addr)
    ///     .build().unwrap();
    /// ```
    pub fn local_address<T>(self, addr: T) -> ClientBuilder
    where
        T: Into<Option<IpAddr>>,
    {
        self.with_inner(move |inner| inner.local_address(addr))
    }

    /// Bind to an interface by `SO_BINDTODEVICE`.
    ///
    /// # Example
    ///
    /// ```
    /// let interface = "lo";
    /// let client = reqwest::blocking::Client::builder()
    ///     .interface(interface)
    ///     .build().unwrap();
    /// ```
    #[cfg(any(
        target_os = "android",
        target_os = "fuchsia",
        target_os = "illumos",
        target_os = "ios",
        target_os = "linux",
        target_os = "macos",
        target_os = "solaris",
        target_os = "tvos",
        target_os = "visionos",
        target_os = "watchos",
    ))]
    pub fn interface(self, interface: &str) -> ClientBuilder {
        self.with_inner(move |inner| inner.interface(interface))
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied duration.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive<D>(self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.with_inner(move |inner| inner.tcp_keepalive(val))
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied interval.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive_interval<D>(self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.with_inner(move |inner| inner.tcp_keepalive_interval(val))
    }

    /// Set that all sockets have `SO_KEEPALIVE` set with the supplied retry count.
    ///
    /// If `None`, the option will not be set.
    pub fn tcp_keepalive_retries<C>(self, retries: C) -> ClientBuilder
    where
        C: Into<Option<u32>>,
    {
        self.with_inner(move |inner| inner.tcp_keepalive_retries(retries))
    }

    /// Set that all sockets have `TCP_USER_TIMEOUT` set with the supplied duration.
    ///
    /// This option controls how long transmitted data may remain unacknowledged before
    /// the connection is force-closed.
    ///
    /// The current default is `None` (option disabled).
    #[cfg(any(target_os = "android", target_os = "fuchsia", target_os = "linux"))]
    pub fn tcp_user_timeout<D>(self, val: D) -> ClientBuilder
    where
        D: Into<Option<Duration>>,
    {
        self.with_inner(move |inner| inner.tcp_user_timeout(val))
    }

    // Alt Transports

    /// Set that all connections will use this Unix socket.
    ///
    /// If a request URI uses the `https` scheme, TLS will still be used over
    /// the Unix socket.
    ///
    /// # Note
    ///
    /// This option is not compatible with any of the TCP or Proxy options.
    /// Setting this will ignore all those options previously set.
    ///
    /// Likewise, DNS resolution will not be done on the domain name.
    #[cfg(unix)]
    pub fn unix_socket(self, path: impl UnixSocketProvider) -> ClientBuilder {
        self.with_inner(move |inner| inner.unix_socket(path))
    }

    // TLS options

    /// Add a custom root certificate.
    ///
    /// This allows connecting to a server that has a self-signed
    /// certificate for example. This **does not** replace the existing
    /// trusted store.
    ///
    /// # Example
    ///
    /// ```
    /// # use std::fs::File;
    /// # use std::io::Read;
    /// # fn build_client() -> Result<(), Box<dyn std::error::Error>> {
    /// // read a local binary DER encoded certificate
    /// let der = std::fs::read("my-cert.der")?;
    ///
    /// // create a certificate
    /// let cert = reqwest::Certificate::from_der(&der)?;
    ///
    /// // get a client builder
    /// let client = reqwest::blocking::Client::builder()
    ///     .add_root_certificate(cert)
    ///     .build()?;
    /// # drop(client);
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn add_root_certificate(self, cert: Certificate) -> ClientBuilder {
        self.with_inner(move |inner| inner.add_root_certificate(cert))
    }

    /// Add a certificate revocation list.
    ///
    ///
    /// # Optional
    ///
    /// This requires the `rustls-tls(-...)` Cargo feature enabled.
    #[cfg(feature = "__rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls")))]
    pub fn add_crl(self, crl: CertificateRevocationList) -> ClientBuilder {
        self.with_inner(move |inner| inner.add_crl(crl))
    }

    /// Add multiple certificate revocation lists.
    ///
    ///
    /// # Optional
    ///
    /// This requires the `rustls-tls(-...)` Cargo feature enabled.
    #[cfg(feature = "__rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls")))]
    pub fn add_crls(
        self,
        crls: impl IntoIterator<Item = CertificateRevocationList>,
    ) -> ClientBuilder {
        self.with_inner(move |inner| inner.add_crls(crls))
    }

    /// Controls the use of built-in system certificates during certificate validation.
    ///
    /// Defaults to `true` -- built-in system certs will be used.
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn tls_built_in_root_certs(self, tls_built_in_root_certs: bool) -> ClientBuilder {
        self.with_inner(move |inner| inner.tls_built_in_root_certs(tls_built_in_root_certs))
    }

    /// Sets whether to load webpki root certs with rustls.
    ///
    /// If the feature is enabled, this value is `true` by default.
    #[cfg(feature = "rustls-tls-webpki-roots-no-provider")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls-webpki-roots-no-provider")))]
    pub fn tls_built_in_webpki_certs(self, enabled: bool) -> ClientBuilder {
        self.with_inner(move |inner| inner.tls_built_in_webpki_certs(enabled))
    }

    /// Sets whether to load native root certs with rustls.
    ///
    /// If the feature is enabled, this value is `true` by default.
    #[cfg(feature = "rustls-tls-native-roots-no-provider")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls-native-roots-no-provider")))]
    pub fn tls_built_in_native_certs(self, enabled: bool) -> ClientBuilder {
        self.with_inner(move |inner| inner.tls_built_in_native_certs(enabled))
    }

    /// Sets the identity to be used for client certificate authentication.
    ///
    /// # Optional
    ///
    /// This requires the optional `native-tls` or `rustls-tls(-...)` feature to be
    /// enabled.
    #[cfg(any(feature = "native-tls", feature = "__rustls"))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "native-tls", feature = "rustls-tls"))))]
    pub fn identity(self, identity: Identity) -> ClientBuilder {
        self.with_inner(move |inner| inner.identity(identity))
    }

    /// Controls the use of hostname verification.
    ///
    /// Defaults to `false`.
    ///
    /// # Warning
    ///
    /// You should think very carefully before you use this method. If
    /// hostname verification is not used, any valid certificate for any
    /// site will be trusted for use from any other. This introduces a
    /// significant vulnerability to man-in-the-middle attacks.
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn danger_accept_invalid_hostnames(self, accept_invalid_hostname: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.danger_accept_invalid_hostnames(accept_invalid_hostname))
    }

    /// Controls the use of certificate validation.
    ///
    /// Defaults to `false`.
    ///
    /// # Warning
    ///
    /// You should think very carefully before using this method. If
    /// invalid certificates are trusted, *any* certificate for *any* site
    /// will be trusted for use. This includes expired certificates. This
    /// introduces significant vulnerabilities, and should only be used
    /// as a last resort.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn danger_accept_invalid_certs(self, accept_invalid_certs: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.danger_accept_invalid_certs(accept_invalid_certs))
    }

    /// Controls the use of TLS server name indication.
    ///
    /// Defaults to `true`.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn tls_sni(self, tls_sni: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.tls_sni(tls_sni))
    }

    /// Set the minimum required TLS version for connections.
    ///
    /// By default, the TLS backend's own default is used.
    ///
    /// # Errors
    ///
    /// A value of `tls::Version::TLS_1_3` will cause an error with the
    /// `native-tls`/`default-tls` backend. This does not mean the version
    /// isn't supported, just that it can't be set as a minimum due to
    /// technical limitations.
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn min_tls_version(self, version: tls::Version) -> ClientBuilder {
        self.with_inner(|inner| inner.min_tls_version(version))
    }

    /// Set the maximum allowed TLS version for connections.
    ///
    /// By default, there's no maximum.
    ///
    /// # Errors
    ///
    /// A value of `tls::Version::TLS_1_3` will cause an error with the
    /// `native-tls`/`default-tls` backend. This does not mean the version
    /// isn't supported, just that it can't be set as a maximum due to
    /// technical limitations.
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn max_tls_version(self, version: tls::Version) -> ClientBuilder {
        self.with_inner(|inner| inner.max_tls_version(version))
    }

    /// Force using the native TLS backend.
    ///
    /// Since multiple TLS backends can be optionally enabled, this option will
    /// force the `native-tls` backend to be used for this `Client`.
    ///
    /// # Optional
    ///
    /// This requires the optional `native-tls` feature to be enabled.
    #[cfg(feature = "native-tls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "native-tls")))]
    pub fn use_native_tls(self) -> ClientBuilder {
        self.with_inner(move |inner| inner.use_native_tls())
    }

    /// Force using the Rustls TLS backend.
    ///
    /// Since multiple TLS backends can be optionally enabled, this option will
    /// force the `rustls` backend to be used for this `Client`.
    ///
    /// # Optional
    ///
    /// This requires the optional `rustls-tls(-...)` feature to be enabled.
    #[cfg(feature = "__rustls")]
    #[cfg_attr(docsrs, doc(cfg(feature = "rustls-tls")))]
    pub fn use_rustls_tls(self) -> ClientBuilder {
        self.with_inner(move |inner| inner.use_rustls_tls())
    }

    /// Add TLS information as `TlsInfo` extension to responses.
    ///
    /// # Optional
    ///
    /// This requires the optional `default-tls`, `native-tls`, or `rustls-tls(-...)`
    /// feature to be enabled.
    #[cfg(feature = "__tls")]
    #[cfg_attr(
        docsrs,
        doc(cfg(any(
            feature = "default-tls",
            feature = "native-tls",
            feature = "rustls-tls"
        )))
    )]
    pub fn tls_info(self, tls_info: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.tls_info(tls_info))
    }

    /// Use a preconfigured TLS backend.
    ///
    /// If the passed `Any` argument is not a TLS backend that reqwest
    /// understands, the `ClientBuilder` will error when calling `build`.
    ///
    /// # Advanced
    ///
    /// This is an advanced option, and can be somewhat brittle. Usage requires
    /// keeping the preconfigured TLS argument version in sync with reqwest,
    /// since version mismatches will result in an "unknown" TLS backend.
    ///
    /// If possible, it's preferable to use the methods on `ClientBuilder`
    /// to configure reqwest's TLS.
    ///
    /// # Optional
    ///
    /// This requires one of the optional features `native-tls` or
    /// `rustls-tls(-...)` to be enabled.
    #[cfg(any(feature = "native-tls", feature = "__rustls",))]
    #[cfg_attr(docsrs, doc(cfg(any(feature = "native-tls", feature = "rustls-tls"))))]
    pub fn use_preconfigured_tls(self, tls: impl Any) -> ClientBuilder {
        self.with_inner(move |inner| inner.use_preconfigured_tls(tls))
    }

    /// Enables the [hickory-dns](hickory_resolver) async resolver instead of a default threadpool using `getaddrinfo`.
    ///
    /// If the `hickory-dns` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `hickory-dns` feature to be enabled
    #[cfg(feature = "hickory-dns")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hickory-dns")))]
    #[deprecated(note = "use `hickory_dns` instead", since = "0.12.0")]
    pub fn trust_dns(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.hickory_dns(enable))
    }

    /// Enables the [hickory-dns](hickory_resolver) async resolver instead of a default threadpool using `getaddrinfo`.
    ///
    /// If the `hickory-dns` feature is turned on, the default option is enabled.
    ///
    /// # Optional
    ///
    /// This requires the optional `hickory-dns` feature to be enabled
    #[cfg(feature = "hickory-dns")]
    #[cfg_attr(docsrs, doc(cfg(feature = "hickory-dns")))]
    pub fn hickory_dns(self, enable: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.hickory_dns(enable))
    }

    /// Disables the hickory-dns async resolver.
    ///
    /// This method exists even if the optional `hickory-dns` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use the hickory-dns async resolver
    /// even if another dependency were to enable the optional `hickory-dns` feature.
    #[deprecated(note = "use `no_hickory_dns` instead", since = "0.12.0")]
    pub fn no_trust_dns(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_hickory_dns())
    }

    /// Disables the hickory-dns async resolver.
    ///
    /// This method exists even if the optional `hickory-dns` feature is not enabled.
    /// This can be used to ensure a `Client` doesn't use the hickory-dns async resolver
    /// even if another dependency were to enable the optional `hickory-dns` feature.
    pub fn no_hickory_dns(self) -> ClientBuilder {
        self.with_inner(|inner| inner.no_hickory_dns())
    }

    /// Restrict the Client to be used with HTTPS only requests.
    ///
    /// Defaults to false.
    pub fn https_only(self, enabled: bool) -> ClientBuilder {
        self.with_inner(|inner| inner.https_only(enabled))
    }

    /// Override DNS resolution for specific domains to a particular IP address.
    ///
    /// Set the port to `0` to use the conventional port for the given scheme (e.g. 80 for http).
    /// Ports in the URL itself will always be used instead of the port in the overridden addr.
    pub fn resolve(self, domain: &str, addr: SocketAddr) -> ClientBuilder {
        self.resolve_to_addrs(domain, &[addr])
    }

    /// Override DNS resolution for specific domains to particular IP addresses.
    ///
    /// Set the port to `0` to use the conventional port for the given scheme (e.g. 80 for http).
    /// Ports in the URL itself will always be used instead of the port in the overridden addr.
    pub fn resolve_to_addrs(self, domain: &str, addrs: &[SocketAddr]) -> ClientBuilder {
        self.with_inner(|inner| inner.resolve_to_addrs(domain, addrs))
    }

    /// Override the DNS resolver implementation.
    ///
    /// Pass an `Arc` wrapping a trait object implementing `Resolve`.
    /// Overrides for specific names passed to `resolve` and `resolve_to_addrs` will
    /// still be applied on top of this resolver.
    pub fn dns_resolver<R: Resolve + 'static>(self, resolver: Arc<R>) -> ClientBuilder {
        self.with_inner(|inner| inner.dns_resolver(resolver))
    }

    /// Adds a new Tower [`Layer`](https://docs.rs/tower/latest/tower/trait.Layer.html) to the
    /// base connector [`Service`](https://docs.rs/tower/latest/tower/trait.Service.html) which
    /// is responsible for connection establishment.
    ///
    /// Each subsequent invocation of this function will wrap previous layers.
    ///
    /// Example usage:
    /// ```
    /// use std::time::Duration;
    ///
    /// let client = reqwest::blocking::Client::builder()
    ///                      // resolved to outermost layer, meaning while we are waiting on concurrency limit
    ///                      .connect_timeout(Duration::from_millis(200))
    ///                      // underneath the concurrency check, so only after concurrency limit lets us through
    ///                      .connector_layer(tower::timeout::TimeoutLayer::new(Duration::from_millis(50)))
    ///                      .connector_layer(tower::limit::concurrency::ConcurrencyLimitLayer::new(2))
    ///                      .build()
    ///                      .unwrap();
    /// ```
    pub fn connector_layer<L>(self, layer: L) -> ClientBuilder
    where
        L: Layer<BoxedConnectorService> + Clone + Send + Sync + 'static,
        L::Service:
            Service<Unnameable, Response = Conn, Error = BoxError> + Clone + Send + Sync + 'static,
        <L::Service as Service<Unnameable>>::Future: Send + 'static,
    {
        self.with_inner(|inner| inner.connector_layer(layer))
    }

    // private

    fn with_inner<F>(mut self, func: F) -> ClientBuilder
    where
        F: FnOnce(async_impl::ClientBuilder) -> async_impl::ClientBuilder,
    {
        self.inner = func(self.inner);
        self
    }
}

impl From<async_impl::ClientBuilder> for ClientBuilder {
    fn from(builder: async_impl::ClientBuilder) -> Self {
        Self {
            inner: builder,
            timeout: Timeout::default(),
        }
    }
}

impl Default for Client {
    fn default() -> Self {
        Self::new()
    }
}

impl Client {
    /// Constructs a new `Client`.
    ///
    /// # Panic
    ///
    /// This method panics if TLS backend cannot be initialized, or the resolver
    /// cannot load the system configuration.
    ///
    /// Use `Client::builder()` if you wish to handle the failure as an `Error`
    /// instead of panicking.
    ///
    /// This method also panics if called from within an async runtime. See docs
    /// on [`reqwest::blocking`][crate::blocking] for details.
    pub fn new() -> Client {
        ClientBuilder::new().build().expect("Client::new()")
    }

    /// Creates a `ClientBuilder` to configure a `Client`.
    ///
    /// This is the same as `ClientBuilder::new()`.
    pub fn builder() -> ClientBuilder {
        ClientBuilder::new()
    }

    /// Convenience method to make a `GET` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn get<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::GET, url)
    }

    /// Convenience method to make a `POST` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn post<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::POST, url)
    }

    /// Convenience method to make a `PUT` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn put<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PUT, url)
    }

    /// Convenience method to make a `PATCH` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn patch<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::PATCH, url)
    }

    /// Convenience method to make a `DELETE` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn delete<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::DELETE, url)
    }

    /// Convenience method to make a `HEAD` request to a URL.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn head<U: IntoUrl>(&self, url: U) -> RequestBuilder {
        self.request(Method::HEAD, url)
    }

    /// Start building a `Request` with the `Method` and `Url`.
    ///
    /// Returns a `RequestBuilder`, which will allow setting headers and
    /// request body before sending.
    ///
    /// # Errors
    ///
    /// This method fails whenever supplied `Url` cannot be parsed.
    pub fn request<U: IntoUrl>(&self, method: Method, url: U) -> RequestBuilder {
        let req = url.into_url().map(move |url| Request::new(method, url));
        RequestBuilder::new(self.clone(), req)
    }

    /// Executes a `Request`.
    ///
    /// A `Request` can be built manually with `Request::new()` or obtained
    /// from a RequestBuilder with `RequestBuilder::build()`.
    ///
    /// You should prefer to use the `RequestBuilder` and
    /// `RequestBuilder::send()`.
    ///
    /// # Errors
    ///
    /// This method fails if there was an error while sending request,
    /// or redirect limit was exhausted.
    pub fn execute(&self, request: Request) -> crate::Result<Response> {
        self.inner.execute_request(request)
    }
}

impl fmt::Debug for Client {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("Client")
            //.field("gzip", &self.inner.gzip)
            //.field("redirect_policy", &self.inner.redirect_policy)
            //.field("referer", &self.inner.referer)
            .finish()
    }
}

impl fmt::Debug for ClientBuilder {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.inner.fmt(f)
    }
}

#[derive(Clone)]
struct ClientHandle {
    timeout: Timeout,
    inner: Arc<InnerClientHandle>,
}

type OneshotResponse = oneshot::Sender<crate::Result<async_impl::Response>>;
type ThreadSender = mpsc::UnboundedSender<(async_impl::Request, OneshotResponse)>;

struct InnerClientHandle {
    tx: Option<ThreadSender>,
    thread: Option<thread::JoinHandle<()>>,
}

impl Drop for InnerClientHandle {
    fn drop(&mut self) {
        let id = self
            .thread
            .as_ref()
            .map(|h| h.thread().id())
            .expect("thread not dropped yet");

        trace!("closing runtime thread ({id:?})");
        self.tx.take();
        trace!("signaled close for runtime thread ({id:?})");
        self.thread.take().map(|h| h.join());
        trace!("closed runtime thread ({id:?})");
    }
}

impl ClientHandle {
    fn new(builder: ClientBuilder) -> crate::Result<ClientHandle> {
        let timeout = builder.timeout;
        let builder = builder.inner;
        let (tx, rx) = mpsc::unbounded_channel::<(async_impl::Request, OneshotResponse)>();
        let (spawn_tx, spawn_rx) = oneshot::channel::<crate::Result<()>>();
        let handle = thread::Builder::new()
            .name("reqwest-internal-sync-runtime".into())
            .spawn(move || {
                use tokio::runtime;
                let rt = match runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(crate::error::builder)
                {
                    Err(e) => {
                        if let Err(e) = spawn_tx.send(Err(e)) {
                            error!("Failed to communicate runtime creation failure: {e:?}");
                        }
                        return;
                    }
                    Ok(v) => v,
                };

                let f = async move {
                    let client = match builder.build() {
                        Err(e) => {
                            if let Err(e) = spawn_tx.send(Err(e)) {
                                error!("Failed to communicate client creation failure: {e:?}");
                            }
                            return;
                        }
                        Ok(v) => v,
                    };
                    if let Err(e) = spawn_tx.send(Ok(())) {
                        error!("Failed to communicate successful startup: {e:?}");
                        return;
                    }

                    let mut rx = rx;

                    while let Some((req, req_tx)) = rx.recv().await {
                        let req_fut = client.execute(req);
                        tokio::spawn(forward(req_fut, req_tx));
                    }

                    trace!("({:?}) Receiver is shutdown", thread::current().id());
                };

                trace!("({:?}) start runtime::block_on", thread::current().id());
                rt.block_on(f);
                trace!("({:?}) end runtime::block_on", thread::current().id());
                drop(rt);
                trace!("({:?}) finished", thread::current().id());
            })
            .map_err(crate::error::builder)?;

        // Wait for the runtime thread to start up...
        match wait::timeout(spawn_rx, None) {
            Ok(Ok(())) => (),
            Ok(Err(err)) => return Err(err),
            Err(_canceled) => event_loop_panicked(),
        }

        let inner_handle = Arc::new(InnerClientHandle {
            tx: Some(tx),
            thread: Some(handle),
        });

        Ok(ClientHandle {
            timeout,
            inner: inner_handle,
        })
    }

    fn execute_request(&self, req: Request) -> crate::Result<Response> {
        let (tx, rx) = oneshot::channel();
        let (req, body) = req.into_async();
        let url = req.url().clone();
        let timeout = req.timeout().copied().or(self.timeout.0);

        self.inner
            .tx
            .as_ref()
            .expect("core thread exited early")
            .send((req, tx))
            .expect("core thread panicked");

        let result: Result<crate::Result<async_impl::Response>, wait::Waited<crate::Error>> =
            if let Some(body) = body {
                let f = async move {
                    body.send().await?;
                    rx.await.map_err(|_canceled| event_loop_panicked())
                };
                wait::timeout(f, timeout)
            } else {
                let f = async move { rx.await.map_err(|_canceled| event_loop_panicked()) };
                wait::timeout(f, timeout)
            };

        match result {
            Ok(Err(err)) => Err(err.with_url(url)),
            Ok(Ok(res)) => Ok(Response::new(
                res,
                timeout,
                KeepCoreThreadAlive(Some(self.inner.clone())),
            )),
            Err(wait::Waited::TimedOut(e)) => Err(crate::error::request(e).with_url(url)),
            Err(wait::Waited::Inner(err)) => Err(err.with_url(url)),
        }
    }
}

async fn forward<F>(fut: F, mut tx: OneshotResponse)
where
    F: Future<Output = crate::Result<async_impl::Response>>,
{
    futures_util::pin_mut!(fut);

    // "select" on the sender being canceled, and the future completing
    let res = std::future::poll_fn(|cx| {
        match fut.as_mut().poll(cx) {
            Poll::Ready(val) => Poll::Ready(Some(val)),
            Poll::Pending => {
                // check if the callback is canceled
                ready!(tx.poll_closed(cx));
                Poll::Ready(None)
            }
        }
    })
    .await;

    if let Some(res) = res {
        let _ = tx.send(res);
    }
    // else request is canceled
}

#[derive(Clone, Copy)]
struct Timeout(Option<Duration>);

impl Default for Timeout {
    fn default() -> Timeout {
        // default mentioned in ClientBuilder::timeout() doc comment
        Timeout(Some(Duration::from_secs(30)))
    }
}

pub(crate) struct KeepCoreThreadAlive(#[allow(dead_code)] Option<Arc<InnerClientHandle>>);

impl KeepCoreThreadAlive {
    pub(crate) fn empty() -> KeepCoreThreadAlive {
        KeepCoreThreadAlive(None)
    }
}

#[cold]
#[inline(never)]
fn event_loop_panicked() -> ! {
    // The only possible reason there would be a Canceled error
    // is if the thread running the event loop panicked. We could return
    // an Err here, like a BrokenPipe, but the Client is not
    // recoverable. Additionally, the panic in the other thread
    // is not normal, and should likely be propagated.
    panic!("event loop thread panicked");
}
