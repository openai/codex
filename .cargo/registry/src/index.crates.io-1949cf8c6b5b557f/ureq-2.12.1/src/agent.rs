use std::fmt;
use std::ops::Deref;
use std::sync::Arc;
use std::time::Duration;
use url::Url;

use crate::middleware::Middleware;
use crate::pool::ConnectionPool;
use crate::proxy::Proxy;
use crate::request::Request;
use crate::resolve::{ArcResolver, StdResolver};
use crate::stream::TlsConnector;

#[cfg(feature = "cookies")]
use {
    crate::cookies::{CookieStoreGuard, CookieTin},
    cookie_store::CookieStore,
};

/// Strategy for keeping `authorization` headers during redirects.
///
/// `Never` is the default strategy and never preserves `authorization` header in redirects.
/// `SameHost` send the authorization header in redirects only if the host of the redirect is
/// the same of the previous request, and both use the same scheme (or switch to a more secure one, i.e
/// we can redirect from `http` to `https`, but not the reverse).
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum RedirectAuthHeaders {
    /// Never preserve the `authorization` header on redirect. This is the default.
    Never,
    /// Preserve the `authorization` header when the redirect is to the same host. Both hosts must use
    /// the same scheme (or switch to a more secure one, i.e we can redirect from `http` to `https`,
    /// but not the reverse).
    SameHost,
}

/// Accumulates options towards building an [Agent].
pub struct AgentBuilder {
    config: AgentConfig,
    try_proxy_from_env: bool,
    max_idle_connections: usize,
    max_idle_connections_per_host: usize,
    /// Cookies saved between requests.
    /// Invariant: All cookies must have a nonempty domain and path.
    #[cfg(feature = "cookies")]
    cookie_store: Option<CookieStore>,
    resolver: ArcResolver,
    middleware: Vec<Box<dyn Middleware>>,
}

#[derive(Clone)]
pub(crate) struct TlsConfig(Arc<dyn TlsConnector>);

impl fmt::Debug for TlsConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TlsConfig").finish()
    }
}

impl Deref for TlsConfig {
    type Target = Arc<dyn TlsConnector>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Config as built by AgentBuilder and then static for the lifetime of the Agent.
#[derive(Clone, Debug)]
pub(crate) struct AgentConfig {
    pub proxy: Option<Proxy>,
    pub timeout_connect: Option<Duration>,
    pub timeout_read: Option<Duration>,
    pub timeout_write: Option<Duration>,
    pub timeout: Option<Duration>,
    pub https_only: bool,
    pub no_delay: bool,
    pub redirects: u32,
    pub redirect_auth_headers: RedirectAuthHeaders,
    pub user_agent: String,
    pub tls_config: TlsConfig,
}

/// Agents keep state between requests.
///
/// By default, no state, such as cookies, is kept between requests.
/// But by creating an agent as entry point for the request, we
/// can keep a state.
///
/// ```
/// # fn main() -> Result<(), ureq::Error> {
/// # ureq::is_test(true);
/// let mut agent = ureq::agent();
///
/// agent
///     .post("http://example.com/post/login")
///     .call()?;
///
/// let secret = agent
///     .get("http://example.com/get/my-protected-page")
///     .call()?
///     .into_string()?;
///
///   println!("Secret is: {}", secret);
/// # Ok(())
/// # }
/// ```
///
/// Agent uses an inner Arc, so cloning an Agent results in an instance
/// that shares the same underlying connection pool and other state.
#[derive(Debug, Clone)]
pub struct Agent {
    pub(crate) config: Arc<AgentConfig>,
    /// Reused agent state for repeated requests from this agent.
    pub(crate) state: Arc<AgentState>,
}

/// Container of the state
///
/// *Internal API*.
pub(crate) struct AgentState {
    /// Reused connections between requests.
    pub(crate) pool: ConnectionPool,
    /// Cookies saved between requests.
    /// Invariant: All cookies must have a nonempty domain and path.
    #[cfg(feature = "cookies")]
    pub(crate) cookie_tin: CookieTin,
    pub(crate) resolver: ArcResolver,
    pub(crate) middleware: Vec<Box<dyn Middleware>>,
}

impl Agent {
    /// Creates an Agent with default settings.
    ///
    /// Same as `AgentBuilder::new().build()`.
    pub fn new() -> Self {
        AgentBuilder::new().build()
    }

    /// Make a request with the HTTP verb as a parameter.
    ///
    /// This allows making requests with verbs that don't have a dedicated
    /// method.
    ///
    /// If you've got an already-parsed [Url], try [request_url][Agent::request_url].
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// use ureq::Response;
    /// let agent = ureq::agent();
    ///
    /// let resp: Response = agent
    ///     .request("OPTIONS", "http://example.com/")
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn request(&self, method: &str, path: &str) -> Request {
        Request::new(self.clone(), method.into(), path.into())
    }

    /// Make a request using an already-parsed [Url].
    ///
    /// This is useful if you've got a parsed Url from some other source, or if
    /// you want to parse the URL and then modify it before making the request.
    /// If you'd just like to pass a String or a `&str`, try [request][Agent::request].
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// use {url::Url, ureq::Response};
    /// let agent = ureq::agent();
    ///
    /// let mut url: Url = "http://example.com/some-page".parse()?;
    /// url.set_path("/get/robots.txt");
    /// let resp: Response = agent
    ///     .request_url("GET", &url)
    ///     .call()?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn request_url(&self, method: &str, url: &Url) -> Request {
        Request::new(self.clone(), method.into(), url.to_string())
    }

    /// Make a GET request from this agent.
    pub fn get(&self, path: &str) -> Request {
        self.request("GET", path)
    }

    /// Make a HEAD request from this agent.
    pub fn head(&self, path: &str) -> Request {
        self.request("HEAD", path)
    }

    /// Make a PATCH request from this agent.
    pub fn patch(&self, path: &str) -> Request {
        self.request("PATCH", path)
    }

    /// Make a POST request from this agent.
    pub fn post(&self, path: &str) -> Request {
        self.request("POST", path)
    }

    /// Make a PUT request from this agent.
    pub fn put(&self, path: &str) -> Request {
        self.request("PUT", path)
    }

    /// Make a DELETE request from this agent.
    pub fn delete(&self, path: &str) -> Request {
        self.request("DELETE", path)
    }

    /// Read access to the cookie store.
    ///
    /// Used to persist the cookies to an external writer.
    ///
    /// ```no_run
    /// use std::io::Write;
    /// use std::fs::File;
    ///
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::agent();
    ///
    /// // Cookies set by www.google.com are stored in agent.
    /// agent.get("https://www.google.com/").call()?;
    ///
    /// // Saves (persistent) cookies
    /// let mut file = File::create("cookies.json")?;
    /// agent.cookie_store().save_json(&mut file).unwrap();
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cookies")]
    pub fn cookie_store(&self) -> CookieStoreGuard<'_> {
        self.state.cookie_tin.read_lock()
    }

    pub(crate) fn weak_state(&self) -> std::sync::Weak<AgentState> {
        Arc::downgrade(&self.state)
    }
}

const DEFAULT_MAX_IDLE_CONNECTIONS: usize = 100;
const DEFAULT_MAX_IDLE_CONNECTIONS_PER_HOST: usize = 1;

impl AgentBuilder {
    pub fn new() -> Self {
        AgentBuilder {
            config: AgentConfig {
                proxy: None,
                timeout_connect: Some(Duration::from_secs(30)),
                timeout_read: None,
                timeout_write: None,
                timeout: None,
                https_only: false,
                no_delay: true,
                redirects: 5,
                redirect_auth_headers: RedirectAuthHeaders::Never,
                user_agent: format!("ureq/{}", env!("CARGO_PKG_VERSION")),
                tls_config: TlsConfig(crate::default_tls_config()),
            },
            #[cfg(feature = "proxy-from-env")]
            try_proxy_from_env: true,
            #[cfg(not(feature = "proxy-from-env"))]
            try_proxy_from_env: false,
            max_idle_connections: DEFAULT_MAX_IDLE_CONNECTIONS,
            max_idle_connections_per_host: DEFAULT_MAX_IDLE_CONNECTIONS_PER_HOST,
            resolver: StdResolver.into(),
            #[cfg(feature = "cookies")]
            cookie_store: None,
            middleware: vec![],
        }
    }

    /// Create a new agent.
    // Note: This could take &self as the first argument, allowing one
    // AgentBuilder to be used multiple times, except CookieStore does
    // not implement clone, so we have to give ownership to the newly
    // built Agent.
    pub fn build(mut self) -> Agent {
        if self.config.proxy.is_none() && self.try_proxy_from_env {
            if let Some(proxy) = Proxy::try_from_system() {
                self.config.proxy = Some(proxy);
            }
        }
        Agent {
            config: Arc::new(self.config),
            state: Arc::new(AgentState {
                pool: ConnectionPool::new_with_limits(
                    self.max_idle_connections,
                    self.max_idle_connections_per_host,
                ),
                #[cfg(feature = "cookies")]
                cookie_tin: CookieTin::new(self.cookie_store.unwrap_or_else(CookieStore::default)),
                resolver: self.resolver,
                middleware: self.middleware,
            }),
        }
    }

    /// Set the proxy server to use for all connections from this Agent.
    ///
    /// Example:
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let proxy = ureq::Proxy::new("user:password@cool.proxy:9090")?;
    /// let agent = ureq::AgentBuilder::new()
    ///     .proxy(proxy)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// Adding a proxy will disable `try_proxy_from_env`.
    pub fn proxy(mut self, proxy: Proxy) -> Self {
        self.config.proxy = Some(proxy);
        self
    }

    /// Attempt to detect proxy settings from the environment, i.e. HTTP_PROXY
    ///
    /// The default is `false` without the `proxy-from-env` feature flag, i.e.
    /// not detecting proxy from env, due to the potential security risk and to
    /// maintain backward compatibility.
    ///
    /// If the `proxy` is set on the builder, this setting has no effect.
    pub fn try_proxy_from_env(mut self, do_try: bool) -> Self {
        self.try_proxy_from_env = do_try;
        self
    }

    /// Enforce the client to only perform HTTPS requests.
    /// This setting also makes the client refuse HTTPS to HTTP redirects.
    /// Default is false
    ///
    /// Example:
    /// ```
    /// let agent = ureq::AgentBuilder::new()
    ///     .https_only(true)
    ///     .build();
    /// ```
    pub fn https_only(mut self, enforce: bool) -> Self {
        self.config.https_only = enforce;
        self
    }

    /// Sets the maximum number of connections allowed in the connection pool.
    /// By default, this is set to 100. Setting this to zero would disable
    /// connection pooling.
    ///
    /// ```
    /// let agent = ureq::AgentBuilder::new()
    ///     .max_idle_connections(200)
    ///     .build();
    /// ```
    pub fn max_idle_connections(mut self, max: usize) -> Self {
        self.max_idle_connections = max;
        self
    }

    /// Sets the maximum number of connections per host to keep in the
    /// connection pool. By default, this is set to 1. Setting this to zero
    /// would disable connection pooling.
    ///
    /// ```
    /// let agent = ureq::AgentBuilder::new()
    ///     .max_idle_connections_per_host(200)
    ///     .build();
    /// ```
    pub fn max_idle_connections_per_host(mut self, max: usize) -> Self {
        self.max_idle_connections_per_host = max;
        self
    }

    /// Configures a custom resolver to be used by this agent. By default,
    /// address-resolution is done by std::net::ToSocketAddrs. This allows you
    /// to override that resolution with your own alternative. Useful for
    /// testing and special-cases like DNS-based load balancing.
    ///
    /// A `Fn(&str) -> io::Result<Vec<SocketAddr>>` is a valid resolver,
    /// passing a closure is a simple way to override. Note that you might need
    /// explicit type `&str` on the closure argument for type inference to
    /// succeed.
    /// ```
    /// use std::net::ToSocketAddrs;
    ///
    /// let mut agent = ureq::AgentBuilder::new()
    ///    .resolver(|addr: &str| match addr {
    ///       "example.com" => Ok(vec![([127,0,0,1], 8096).into()]),
    ///       addr => addr.to_socket_addrs().map(Iterator::collect),
    ///    })
    ///    .build();
    /// ```
    pub fn resolver(mut self, resolver: impl crate::Resolver + 'static) -> Self {
        self.resolver = resolver.into();
        self
    }

    /// Timeout for the socket connection to be successful.
    /// If both this and `.timeout()` are both set, `.timeout_connect()`
    /// takes precedence.
    ///
    /// The default is 30 seconds.
    ///
    /// ```
    /// use std::time::Duration;
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::builder()
    ///     .timeout_connect(Duration::from_secs(1))
    ///     .build();
    /// let result = agent.get("http://httpbin.org/delay/20").call();
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout_connect(mut self, timeout: Duration) -> Self {
        self.config.timeout_connect = Some(timeout);
        self
    }

    /// Timeout for the individual reads of the socket.
    /// If both this and `.timeout()` are both set, `.timeout()`
    /// takes precedence.
    ///
    /// The default is no timeout. In other words, requests may block forever on reads by default.
    ///
    /// ```
    /// use std::time::Duration;
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::builder()
    ///     .timeout_read(Duration::from_secs(1))
    ///     .build();
    /// let result = agent.get("http://httpbin.org/delay/20").call();
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout_read(mut self, timeout: Duration) -> Self {
        self.config.timeout_read = Some(timeout);
        self
    }

    /// Timeout for the individual writes to the socket.
    /// If both this and `.timeout()` are both set, `.timeout()`
    /// takes precedence.
    ///
    /// The default is no timeout. In other words, requests may block forever on writes by default.
    ///
    /// ```
    /// use std::time::Duration;
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::builder()
    ///     .timeout_read(Duration::from_secs(1))
    ///     .build();
    /// let result = agent.get("http://httpbin.org/delay/20").call();
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout_write(mut self, timeout: Duration) -> Self {
        self.config.timeout_write = Some(timeout);
        self
    }

    /// Timeout for the overall request, including DNS resolution, connection
    /// time, redirects, and reading the response body. Slow DNS resolution
    /// may cause a request to exceed the timeout, because the DNS request
    /// cannot be interrupted with the available APIs.
    ///
    /// This takes precedence over `.timeout_read()` and `.timeout_write()`, but
    /// not `.timeout_connect()`.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// // wait max 1 second for whole request to complete.
    /// let agent = ureq::builder()
    ///     .timeout(std::time::Duration::from_secs(1))
    ///     .build();
    /// let result = agent.get("http://httpbin.org/delay/20").call();
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.config.timeout = Some(timeout);
        self
    }

    /// Whether no_delay will be set on the tcp socket.
    /// Setting this to true disables Nagle's algorithm.
    ///
    /// Defaults to true.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::builder()
    ///     .no_delay(false)
    ///     .build();
    /// let result = agent.get("http://httpbin.org/get").call();
    /// # Ok(())
    /// # }
    /// ```
    pub fn no_delay(mut self, no_delay: bool) -> Self {
        self.config.no_delay = no_delay;
        self
    }

    /// How many redirects to follow.
    ///
    /// Defaults to `5`. Set to `0` to avoid redirects and instead
    /// get a response object with the 3xx status code.
    ///
    /// If the redirect count hits this limit (and it's > 0), TooManyRedirects is returned.
    ///
    /// WARNING: for 307 and 308 redirects, this value is ignored for methods that have a body.
    /// You must handle 307 redirects yourself when sending a PUT, POST, PATCH, or DELETE request.
    ///
    /// ```no_run
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let result = ureq::builder()
    ///     .redirects(1)
    ///     .build()
    ///     # ;
    /// # let result = ureq::agent()
    ///     .get("http://httpbin.org/status/301")
    ///     .call()?;
    /// assert_ne!(result.status(), 301);
    ///
    /// let result = ureq::post("http://httpbin.org/status/307")
    ///     .send_bytes(b"some data")?;
    /// assert_eq!(result.status(), 307);
    /// # Ok(())
    /// # }
    /// ```
    pub fn redirects(mut self, n: u32) -> Self {
        self.config.redirects = n;
        self
    }

    /// Set the strategy for propagation of authorization headers in redirects.
    ///
    /// Defaults to [`RedirectAuthHeaders::Never`].
    ///
    pub fn redirect_auth_headers(mut self, v: RedirectAuthHeaders) -> Self {
        self.config.redirect_auth_headers = v;
        self
    }

    /// The user-agent header to associate with all requests from this agent by default.
    ///
    /// Defaults to `ureq/[VERSION]`. You can override the user-agent on an individual request by
    /// setting the `User-Agent` header when constructing the request.
    ///
    /// ```no_run
    /// # #[cfg(feature = "json")]
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// let agent = ureq::builder()
    ///     .user_agent("ferris/1.0")
    ///     .build();
    ///
    /// // Uses agent's header
    /// let result: serde_json::Value =
    ///     agent.get("http://httpbin.org/headers").call()?.into_json()?;
    /// assert_eq!(&result["headers"]["User-Agent"], "ferris/1.0");
    ///
    /// // Overrides user-agent set on the agent
    /// let result: serde_json::Value = agent.get("http://httpbin.org/headers")
    ///     .set("User-Agent", "super-ferris/2.0")
    ///     .call()?.into_json()?;
    /// assert_eq!(&result["headers"]["User-Agent"], "super-ferris/2.0");
    /// # Ok(())
    /// # }
    /// # #[cfg(not(feature = "json"))]
    /// # fn main() {}
    /// ```
    pub fn user_agent(mut self, user_agent: &str) -> Self {
        self.config.user_agent = user_agent.into();
        self
    }

    /// Configure TLS options for rustls to use when making HTTPS connections from this Agent.
    ///
    /// This overrides any previous call to tls_config or tls_connector.
    ///
    /// ```
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// use std::sync::Arc;
    /// let mut root_store = rustls::RootCertStore {
    ///   roots: webpki_roots::TLS_SERVER_ROOTS.iter().cloned().collect(),
    /// };
    ///
    /// let tls_config = rustls::ClientConfig::builder()
    ///     .with_root_certificates(root_store)
    ///     .with_no_client_auth();
    /// let agent = ureq::builder()
    ///     .tls_config(Arc::new(tls_config))
    ///     .build();
    /// # Ok(())
    /// # }
    #[cfg(feature = "tls")]
    pub fn tls_config(mut self, tls_config: Arc<rustls::ClientConfig>) -> Self {
        self.config.tls_config = TlsConfig(Arc::new(tls_config));
        self
    }

    /// Configure TLS options for a backend other than rustls. The parameter can be a
    /// any type which implements the [`TlsConnector`] trait. If you enable the native-tls
    /// feature, we provide `impl TlsConnector for native_tls::TlsConnector` so you can pass
    /// [`Arc<native_tls::TlsConnector>`](https://docs.rs/native-tls/0.2.7/native_tls/struct.TlsConnector.html).
    ///
    /// This overrides any previous call to tls_config or tls_connector.
    ///
    /// ```
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// # ureq::is_test(true);
    /// use std::sync::Arc;
    /// # #[cfg(feature = "native-tls")]
    /// let tls_connector = Arc::new(native_tls::TlsConnector::new()?);
    /// # #[cfg(feature = "native-tls")]
    /// let agent = ureq::builder()
    ///     .tls_connector(tls_connector.clone())
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    pub fn tls_connector<T: TlsConnector + 'static>(mut self, tls_config: Arc<T>) -> Self {
        self.config.tls_config = TlsConfig(tls_config);
        self
    }

    /// Provide the cookie store to be used for all requests using this agent.
    ///
    /// This is useful in two cases. First when there is a need to persist cookies
    /// to some backing store, and second when there's a need to prepare the agent
    /// with some pre-existing cookies.
    ///
    /// Example
    /// ```no_run
    /// # fn main() -> Result<(), ureq::Error> {
    /// # ureq::is_test(true);
    /// use cookie_store::CookieStore;
    /// use std::fs::File;
    /// use std::io::BufReader;
    /// let file = File::open("cookies.json")?;
    /// let read = BufReader::new(file);
    ///
    /// // Read persisted cookies from cookies.json
    /// let my_store = CookieStore::load_json(read).unwrap();
    ///
    /// // Cookies will be used for requests done through agent.
    /// let agent = ureq::builder()
    ///     .cookie_store(my_store)
    ///     .build();
    /// # Ok(())
    /// # }
    /// ```
    #[cfg(feature = "cookies")]
    pub fn cookie_store(mut self, cookie_store: CookieStore) -> Self {
        self.cookie_store = Some(cookie_store);
        self
    }

    /// Add middleware handler to this agent.
    ///
    /// All requests made by the agent will use this middleware. Middleware is invoked
    /// in the order they are added to the builder.
    pub fn middleware(mut self, m: impl Middleware) -> Self {
        self.middleware.push(Box::new(m));
        self
    }
}

impl fmt::Debug for AgentBuilder {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentBuilder")
            .field("config", &self.config)
            .field("max_idle_connections", &self.max_idle_connections)
            .field(
                "max_idle_connections_per_host",
                &self.max_idle_connections_per_host,
            )
            .field("resolver", &self.resolver)
            // self.cookies missing because it's feature flagged.
            // self.middleware missing because we don't want to force Debug on Middleware trait.
            .finish_non_exhaustive()
    }
}

impl fmt::Debug for AgentState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AgentState")
            .field("pool", &self.pool)
            .field("resolver", &self.resolver)
            // self.cookie_tin missing because it's feature flagged.
            // self.middleware missing because we don't want to force Debug on Middleware trait.
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    ///////////////////// AGENT TESTS //////////////////////////////

    #[test]
    fn agent_implements_send_and_sync() {
        let _agent: Box<dyn Send> = Box::new(AgentBuilder::new().build());
        let _agent: Box<dyn Sync> = Box::new(AgentBuilder::new().build());
    }

    #[test]
    fn agent_config_debug() {
        let agent = AgentBuilder::new().build();
        let debug_format = format!("{:?}", agent);
        assert!(debug_format.contains("Agent"));
        assert!(debug_format.contains("config:"));
        assert!(debug_format.contains("proxy:"));
        assert!(debug_format.contains("timeout_connect:"));
        assert!(debug_format.contains("timeout_read:"));
        assert!(debug_format.contains("timeout_write:"));
        assert!(debug_format.contains("timeout:"));
        assert!(debug_format.contains("https_only:"));
        assert!(debug_format.contains("no_delay:"));
        assert!(debug_format.contains("redirects:"));
        assert!(debug_format.contains("redirect_auth_headers:"));
        assert!(debug_format.contains("user_agent:"));
        assert!(debug_format.contains("tls_config:"));
        assert!(debug_format.contains("state:"));
        assert!(debug_format.contains("pool:"));
    }
}
