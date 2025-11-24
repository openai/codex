use std::error::Error;
use std::fmt;
use std::sync::Arc;

use http::{header::HeaderValue, HeaderMap, Uri};
use hyper_util::client::proxy::matcher;

use crate::into_url::{IntoUrl, IntoUrlSealed};
use crate::Url;

// # Internals
//
// This module is a couple pieces:
//
// - The public builder API
// - The internal built types that our Connector knows how to use.
//
// The user creates a builder (`reqwest::Proxy`), and configures any extras.
// Once that type is passed to the `ClientBuilder`, we convert it into the
// built matcher types, making use of `hyper-util`'s matchers.

/// Configuration of a proxy that a `Client` should pass requests to.
///
/// A `Proxy` has a couple pieces to it:
///
/// - a URL of how to talk to the proxy
/// - rules on what `Client` requests should be directed to the proxy
///
/// For instance, let's look at `Proxy::http`:
///
/// ```rust
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let proxy = reqwest::Proxy::http("https://secure.example")?;
/// # Ok(())
/// # }
/// ```
///
/// This proxy will intercept all HTTP requests, and make use of the proxy
/// at `https://secure.example`. A request to `http://hyper.rs` will talk
/// to your proxy. A request to `https://hyper.rs` will not.
///
/// Multiple `Proxy` rules can be configured for a `Client`. The `Client` will
/// check each `Proxy` in the order it was added. This could mean that a
/// `Proxy` added first with eager intercept rules, such as `Proxy::all`,
/// would prevent a `Proxy` later in the list from ever working, so take care.
///
/// By enabling the `"socks"` feature it is possible to use a socks proxy:
/// ```rust
/// # fn run() -> Result<(), Box<dyn std::error::Error>> {
/// let proxy = reqwest::Proxy::http("socks5://192.168.1.1:9000")?;
/// # Ok(())
/// # }
/// ```
#[derive(Clone)]
pub struct Proxy {
    extra: Extra,
    intercept: Intercept,
    no_proxy: Option<NoProxy>,
}

/// A configuration for filtering out requests that shouldn't be proxied
#[derive(Clone, Debug, Default)]
pub struct NoProxy {
    inner: String,
}

#[derive(Clone)]
struct Extra {
    auth: Option<HeaderValue>,
    misc: Option<HeaderMap>,
}

// ===== Internal =====

pub(crate) struct Matcher {
    inner: Matcher_,
    extra: Extra,
    maybe_has_http_auth: bool,
    maybe_has_http_custom_headers: bool,
}

enum Matcher_ {
    Util(matcher::Matcher),
    Custom(Custom),
}

/// Our own type, wrapping an `Intercept`, since we may have a few additional
/// pieces attached thanks to `reqwest`s extra proxy configuration.
pub(crate) struct Intercepted {
    inner: matcher::Intercept,
    /// This is because of `reqwest::Proxy`'s design which allows configuring
    /// an explicit auth, besides what might have been in the URL (or Custom).
    extra: Extra,
}

/*
impl ProxyScheme {
    fn maybe_http_auth(&self) -> Option<&HeaderValue> {
        match self {
            ProxyScheme::Http { auth, .. } | ProxyScheme::Https { auth, .. } => auth.as_ref(),
            #[cfg(feature = "socks")]
            _ => None,
        }
    }

    fn maybe_http_custom_headers(&self) -> Option<&HeaderMap> {
        match self {
            ProxyScheme::Http { misc, .. } | ProxyScheme::Https { misc, .. } => misc.as_ref(),
            #[cfg(feature = "socks")]
            _ => None,
        }
    }
}
*/

/// Trait used for converting into a proxy scheme. This trait supports
/// parsing from a URL-like type, whilst also supporting proxy schemes
/// built directly using the factory methods.
pub trait IntoProxy {
    fn into_proxy(self) -> crate::Result<Url>;
}

impl<S: IntoUrl> IntoProxy for S {
    fn into_proxy(self) -> crate::Result<Url> {
        match self.as_str().into_url() {
            Ok(mut url) => {
                // If the scheme is a SOCKS protocol and no port is specified, set the default
                if url.port().is_none()
                    && matches!(url.scheme(), "socks4" | "socks4a" | "socks5" | "socks5h")
                {
                    let _ = url.set_port(Some(1080));
                }
                Ok(url)
            }
            Err(e) => {
                let mut presumed_to_have_scheme = true;
                let mut source = e.source();
                while let Some(err) = source {
                    if let Some(parse_error) = err.downcast_ref::<url::ParseError>() {
                        if *parse_error == url::ParseError::RelativeUrlWithoutBase {
                            presumed_to_have_scheme = false;
                            break;
                        }
                    } else if err.downcast_ref::<crate::error::BadScheme>().is_some() {
                        presumed_to_have_scheme = false;
                        break;
                    }
                    source = err.source();
                }
                if presumed_to_have_scheme {
                    return Err(crate::error::builder(e));
                }
                // the issue could have been caused by a missing scheme, so we try adding http://
                let try_this = format!("http://{}", self.as_str());
                try_this.into_url().map_err(|_| {
                    // return the original error
                    crate::error::builder(e)
                })
            }
        }
    }
}

// These bounds are accidentally leaked by the blanket impl of IntoProxy
// for all types that implement IntoUrl. So, this function exists to detect
// if we were to break those bounds for a user.
fn _implied_bounds() {
    fn prox<T: IntoProxy>(_t: T) {}

    fn url<T: IntoUrl>(t: T) {
        prox(t);
    }
}

impl Proxy {
    /// Proxy all HTTP traffic to the passed URL.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::Client::builder()
    ///     .proxy(reqwest::Proxy::http("https://my.prox")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn http<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::Http(proxy_scheme.into_proxy()?)))
    }

    /// Proxy all HTTPS traffic to the passed URL.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::Client::builder()
    ///     .proxy(reqwest::Proxy::https("https://example.prox:4545")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn https<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::Https(proxy_scheme.into_proxy()?)))
    }

    /// Proxy **all** traffic to the passed URL.
    ///
    /// "All" refers to `https` and `http` URLs. Other schemes are not
    /// recognized by reqwest.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let client = reqwest::Client::builder()
    ///     .proxy(reqwest::Proxy::all("http://pro.xy")?)
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn all<U: IntoProxy>(proxy_scheme: U) -> crate::Result<Proxy> {
        Ok(Proxy::new(Intercept::All(proxy_scheme.into_proxy()?)))
    }

    /// Provide a custom function to determine what traffic to proxy to where.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let target = reqwest::Url::parse("https://my.prox")?;
    /// let client = reqwest::Client::builder()
    ///     .proxy(reqwest::Proxy::custom(move |url| {
    ///         if url.host_str() == Some("hyper.rs") {
    ///             Some(target.clone())
    ///         } else {
    ///             None
    ///         }
    ///     }))
    ///     .build()?;
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn custom<F, U: IntoProxy>(fun: F) -> Proxy
    where
        F: Fn(&Url) -> Option<U> + Send + Sync + 'static,
    {
        Proxy::new(Intercept::Custom(Custom {
            func: Arc::new(move |url| fun(url).map(IntoProxy::into_proxy)),
            no_proxy: None,
        }))
    }

    fn new(intercept: Intercept) -> Proxy {
        Proxy {
            extra: Extra {
                auth: None,
                misc: None,
            },
            intercept,
            no_proxy: None,
        }
    }

    /// Set the `Proxy-Authorization` header using Basic auth.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = reqwest::Proxy::https("http://localhost:1234")?
    ///     .basic_auth("Aladdin", "open sesame");
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn basic_auth(mut self, username: &str, password: &str) -> Proxy {
        match self.intercept {
            Intercept::All(ref mut s)
            | Intercept::Http(ref mut s)
            | Intercept::Https(ref mut s) => url_auth(s, username, password),
            Intercept::Custom(_) => {
                let header = encode_basic_auth(username, password);
                self.extra.auth = Some(header);
            }
        }

        self
    }

    /// Set the `Proxy-Authorization` header to a specified value.
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # use reqwest::header::*;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = reqwest::Proxy::https("http://localhost:1234")?
    ///     .custom_http_auth(HeaderValue::from_static("justletmeinalreadyplease"));
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn custom_http_auth(mut self, header_value: HeaderValue) -> Proxy {
        self.extra.auth = Some(header_value);
        self
    }

    /// Adds a Custom Headers to Proxy
    /// Adds custom headers to this Proxy
    ///
    /// # Example
    /// ```
    /// # extern crate reqwest;
    /// # use reqwest::header::*;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut headers = HeaderMap::new();
    /// headers.insert(USER_AGENT, "reqwest".parse().unwrap());
    /// let proxy = reqwest::Proxy::https("http://localhost:1234")?
    ///     .headers(headers);
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn headers(mut self, headers: HeaderMap) -> Proxy {
        match self.intercept {
            Intercept::All(_) | Intercept::Http(_) | Intercept::Https(_) | Intercept::Custom(_) => {
                self.extra.misc = Some(headers);
            }
        }

        self
    }

    /// Adds a `No Proxy` exclusion list to this Proxy
    ///
    /// # Example
    ///
    /// ```
    /// # extern crate reqwest;
    /// # fn run() -> Result<(), Box<dyn std::error::Error>> {
    /// let proxy = reqwest::Proxy::https("http://localhost:1234")?
    ///     .no_proxy(reqwest::NoProxy::from_string("direct.tld, sub.direct2.tld"));
    /// # Ok(())
    /// # }
    /// # fn main() {}
    /// ```
    pub fn no_proxy(mut self, no_proxy: Option<NoProxy>) -> Proxy {
        self.no_proxy = no_proxy;
        self
    }

    pub(crate) fn into_matcher(self) -> Matcher {
        let Proxy {
            intercept,
            extra,
            no_proxy,
        } = self;

        let maybe_has_http_auth;
        let maybe_has_http_custom_headers;

        let inner = match intercept {
            Intercept::All(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(
                    matcher::Matcher::builder()
                        .all(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                )
            }
            Intercept::Http(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(
                    matcher::Matcher::builder()
                        .http(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                )
            }
            Intercept::Https(url) => {
                maybe_has_http_auth = cache_maybe_has_http_auth(&url, &extra.auth);
                maybe_has_http_custom_headers =
                    cache_maybe_has_http_custom_headers(&url, &extra.misc);
                Matcher_::Util(
                    matcher::Matcher::builder()
                        .https(String::from(url))
                        .no(no_proxy.as_ref().map(|n| n.inner.as_ref()).unwrap_or(""))
                        .build(),
                )
            }
            Intercept::Custom(mut custom) => {
                maybe_has_http_auth = true; // never know
                maybe_has_http_custom_headers = true;
                custom.no_proxy = no_proxy;
                Matcher_::Custom(custom)
            }
        };

        Matcher {
            inner,
            extra,
            maybe_has_http_auth,
            maybe_has_http_custom_headers,
        }
    }

    /*
    pub(crate) fn maybe_has_http_auth(&self) -> bool {
        match &self.intercept {
            Intercept::All(p) | Intercept::Http(p) => p.maybe_http_auth().is_some(),
            // Custom *may* match 'http', so assume so.
            Intercept::Custom(_) => true,
            Intercept::System(system) => system
                .get("http")
                .and_then(|s| s.maybe_http_auth())
                .is_some(),
            Intercept::Https(_) => false,
        }
    }

    pub(crate) fn http_basic_auth<D: Dst>(&self, uri: &D) -> Option<HeaderValue> {
        match &self.intercept {
            Intercept::All(p) | Intercept::Http(p) => p.maybe_http_auth().cloned(),
            Intercept::System(system) => system
                .get("http")
                .and_then(|s| s.maybe_http_auth().cloned()),
            Intercept::Custom(custom) => {
                custom.call(uri).and_then(|s| s.maybe_http_auth().cloned())
            }
            Intercept::Https(_) => None,
        }
    }
    */
}

fn cache_maybe_has_http_auth(url: &Url, extra: &Option<HeaderValue>) -> bool {
    url.scheme() == "http" && (url.password().is_some() || extra.is_some())
}

fn cache_maybe_has_http_custom_headers(url: &Url, extra: &Option<HeaderMap>) -> bool {
    url.scheme() == "http" && extra.is_some()
}

impl fmt::Debug for Proxy {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_tuple("Proxy")
            .field(&self.intercept)
            .field(&self.no_proxy)
            .finish()
    }
}

impl NoProxy {
    /// Returns a new no-proxy configuration based on environment variables (or `None` if no variables are set)
    /// see [self::NoProxy::from_string()] for the string format
    pub fn from_env() -> Option<NoProxy> {
        let raw = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .ok()?;

        // Per the docs, this returns `None` if no environment variable is set. We can only reach
        // here if an env var is set, so we return `Some(NoProxy::default)` if `from_string`
        // returns None, which occurs with an empty string.
        Some(Self::from_string(&raw).unwrap_or_default())
    }

    /// Returns a new no-proxy configuration based on a `no_proxy` string (or `None` if no variables
    /// are set)
    /// The rules are as follows:
    /// * The environment variable `NO_PROXY` is checked, if it is not set, `no_proxy` is checked
    /// * If neither environment variable is set, `None` is returned
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding /size,
    ///   for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example `google.com`
    ///   and `.google.com` are equivalent) and would match both that domain AND all subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all the following would match
    /// (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URL `http://notgoogle.com/` would not match.
    pub fn from_string(no_proxy_list: &str) -> Option<Self> {
        // lazy parsed, to not make the type public in hyper-util
        Some(NoProxy {
            inner: no_proxy_list.into(),
        })
    }
}

impl Matcher {
    pub(crate) fn system() -> Self {
        Self {
            inner: Matcher_::Util(matcher::Matcher::from_system()),
            extra: Extra {
                auth: None,
                misc: None,
            },
            // maybe env vars have auth!
            maybe_has_http_auth: true,
            maybe_has_http_custom_headers: true,
        }
    }

    pub(crate) fn intercept(&self, dst: &Uri) -> Option<Intercepted> {
        let inner = match self.inner {
            Matcher_::Util(ref m) => m.intercept(dst),
            Matcher_::Custom(ref c) => c.call(dst),
        };

        inner.map(|inner| Intercepted {
            inner,
            extra: self.extra.clone(),
        })
    }

    /// Return whether this matcher might provide HTTP (not s) auth.
    ///
    /// This is very specific. If this proxy needs auth to be part of a Forward
    /// request (instead of a tunnel), this should return true.
    ///
    /// If it's not sure, this should return true.
    ///
    /// This is meant as a hint to allow skipping a more expensive check
    /// (calling `intercept()`) if it will never need auth when Forwarding.
    pub(crate) fn maybe_has_http_auth(&self) -> bool {
        self.maybe_has_http_auth
    }

    pub(crate) fn http_non_tunnel_basic_auth(&self, dst: &Uri) -> Option<HeaderValue> {
        if let Some(proxy) = self.intercept(dst) {
            if proxy.uri().scheme_str() == Some("http") {
                return proxy.basic_auth().cloned();
            }
        }

        None
    }

    pub(crate) fn maybe_has_http_custom_headers(&self) -> bool {
        self.maybe_has_http_custom_headers
    }

    pub(crate) fn http_non_tunnel_custom_headers(&self, dst: &Uri) -> Option<HeaderMap> {
        if let Some(proxy) = self.intercept(dst) {
            if proxy.uri().scheme_str() == Some("http") {
                return proxy.custom_headers().cloned();
            }
        }

        None
    }
}

impl fmt::Debug for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.inner {
            Matcher_::Util(ref m) => m.fmt(f),
            Matcher_::Custom(ref m) => m.fmt(f),
        }
    }
}

impl Intercepted {
    pub(crate) fn uri(&self) -> &http::Uri {
        self.inner.uri()
    }

    pub(crate) fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Some(ref val) = self.extra.auth {
            return Some(val);
        }
        self.inner.basic_auth()
    }

    pub(crate) fn custom_headers(&self) -> Option<&HeaderMap> {
        if let Some(ref val) = self.extra.misc {
            return Some(val);
        }
        None
    }

    #[cfg(feature = "socks")]
    pub(crate) fn raw_auth(&self) -> Option<(&str, &str)> {
        self.inner.raw_auth()
    }
}

impl fmt::Debug for Intercepted {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.uri().fmt(f)
    }
}

/*
impl ProxyScheme {
    /// Use a username and password when connecting to the proxy server
    fn with_basic_auth<T: Into<String>, U: Into<String>>(
        mut self,
        username: T,
        password: U,
    ) -> Self {
        self.set_basic_auth(username, password);
        self
    }

    fn set_basic_auth<T: Into<String>, U: Into<String>>(&mut self, username: T, password: U) {
        match *self {
            ProxyScheme::Http { ref mut auth, .. } => {
                let header = encode_basic_auth(&username.into(), &password.into());
                *auth = Some(header);
            }
            ProxyScheme::Https { ref mut auth, .. } => {
                let header = encode_basic_auth(&username.into(), &password.into());
                *auth = Some(header);
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks4 { .. } => {
                panic!("Socks4 is not supported for this method")
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks5 { ref mut auth, .. } => {
                *auth = Some((username.into(), password.into()));
            }
        }
    }

    fn set_custom_http_auth(&mut self, header_value: HeaderValue) {
        match *self {
            ProxyScheme::Http { ref mut auth, .. } => {
                *auth = Some(header_value);
            }
            ProxyScheme::Https { ref mut auth, .. } => {
                *auth = Some(header_value);
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks4 { .. } => {
                panic!("Socks4 is not supported for this method")
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks5 { .. } => {
                panic!("Socks5 is not supported for this method")
            }
        }
    }

    fn set_custom_headers(&mut self, headers: HeaderMap) {
        match *self {
            ProxyScheme::Http { ref mut misc, .. } => {
                misc.get_or_insert_with(HeaderMap::new).extend(headers)
            }
            ProxyScheme::Https { ref mut misc, .. } => {
                misc.get_or_insert_with(HeaderMap::new).extend(headers)
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks4 { .. } => {
                panic!("Socks4 is not supported for this method")
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks5 { .. } => {
                panic!("Socks5 is not supported for this method")
            }
        }
    }

    fn if_no_auth(mut self, update: &Option<HeaderValue>) -> Self {
        match self {
            ProxyScheme::Http { ref mut auth, .. } => {
                if auth.is_none() {
                    *auth = update.clone();
                }
            }
            ProxyScheme::Https { ref mut auth, .. } => {
                if auth.is_none() {
                    *auth = update.clone();
                }
            }
            #[cfg(feature = "socks")]
            ProxyScheme::Socks4 { .. } => {}
            #[cfg(feature = "socks")]
            ProxyScheme::Socks5 { .. } => {}
        }

        self
    }

    /// Convert a URL into a proxy scheme
    ///
    /// Supported schemes: HTTP, HTTPS, (SOCKS4, SOCKS5, SOCKS5H if `socks` feature is enabled).
    // Private for now...
    fn parse(url: Url) -> crate::Result<Self> {
        use url::Position;

        // Resolve URL to a host and port
        #[cfg(feature = "socks")]
        let to_addr = || {
            let addrs = url
                .socket_addrs(|| match url.scheme() {
                    "socks4" | "socks4a" | "socks5" | "socks5h" => Some(1080),
                    _ => None,
                })
                .map_err(crate::error::builder)?;
            addrs
                .into_iter()
                .next()
                .ok_or_else(|| crate::error::builder("unknown proxy scheme"))
        };

        let mut scheme = match url.scheme() {
            "http" => Self::http(&url[Position::BeforeHost..Position::AfterPort])?,
            "https" => Self::https(&url[Position::BeforeHost..Position::AfterPort])?,
            #[cfg(feature = "socks")]
            "socks4" => Self::socks4(to_addr()?)?,
            #[cfg(feature = "socks")]
            "socks4a" => Self::socks4a(to_addr()?)?,
            #[cfg(feature = "socks")]
            "socks5" => Self::socks5(to_addr()?)?,
            #[cfg(feature = "socks")]
            "socks5h" => Self::socks5h(to_addr()?)?,
            _ => return Err(crate::error::builder("unknown proxy scheme")),
        };

        if let Some(pwd) = url.password() {
            let decoded_username = percent_decode(url.username().as_bytes()).decode_utf8_lossy();
            let decoded_password = percent_decode(pwd.as_bytes()).decode_utf8_lossy();
            scheme = scheme.with_basic_auth(decoded_username, decoded_password);
        }

        Ok(scheme)
    }
}
*/

#[derive(Clone, Debug)]
enum Intercept {
    All(Url),
    Http(Url),
    Https(Url),
    Custom(Custom),
}

fn url_auth(url: &mut Url, username: &str, password: &str) {
    url.set_username(username).expect("is a base");
    url.set_password(Some(password)).expect("is a base");
}

#[derive(Clone)]
struct Custom {
    func: Arc<dyn Fn(&Url) -> Option<crate::Result<Url>> + Send + Sync + 'static>,
    no_proxy: Option<NoProxy>,
}

impl Custom {
    fn call(&self, uri: &http::Uri) -> Option<matcher::Intercept> {
        let url = format!(
            "{}://{}{}{}",
            uri.scheme()?,
            uri.host()?,
            uri.port().map_or("", |_| ":"),
            uri.port().map_or(String::new(), |p| p.to_string())
        )
        .parse()
        .expect("should be valid Url");

        (self.func)(&url)
            .and_then(|result| result.ok())
            .and_then(|target| {
                let m = matcher::Matcher::builder()
                    .all(String::from(target))
                    .build();

                m.intercept(uri)
            })
        //.map(|scheme| scheme.if_no_auth(&self.auth))
    }
}

impl fmt::Debug for Custom {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str("_")
    }
}

pub(crate) fn encode_basic_auth(username: &str, password: &str) -> HeaderValue {
    crate::util::basic_auth(username, Some(password))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn url(s: &str) -> http::Uri {
        s.parse().unwrap()
    }

    fn intercepted_uri(p: &Matcher, s: &str) -> Uri {
        p.intercept(&s.parse().unwrap()).unwrap().uri().clone()
    }

    #[test]
    fn test_http() {
        let target = "http://example.domain/";
        let p = Proxy::http(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let other = "https://hyper.rs";

        assert_eq!(intercepted_uri(&p, http), target);
        assert!(p.intercept(&url(other)).is_none());
    }

    #[test]
    fn test_https() {
        let target = "http://example.domain/";
        let p = Proxy::https(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let other = "https://hyper.rs";

        assert!(p.intercept(&url(http)).is_none());
        assert_eq!(intercepted_uri(&p, other), target);
    }

    #[test]
    fn test_all() {
        let target = "http://example.domain/";
        let p = Proxy::all(target).unwrap().into_matcher();

        let http = "http://hyper.rs";
        let https = "https://hyper.rs";
        // no longer supported
        //let other = "x-youve-never-heard-of-me-mr-proxy://hyper.rs";

        assert_eq!(intercepted_uri(&p, http), target);
        assert_eq!(intercepted_uri(&p, https), target);
        //assert_eq!(intercepted_uri(&p, other), target);
    }

    #[test]
    fn test_custom() {
        let target1 = "http://example.domain/";
        let target2 = "https://example.domain/";
        let p = Proxy::custom(move |url| {
            if url.host_str() == Some("hyper.rs") {
                target1.parse().ok()
            } else if url.scheme() == "http" {
                target2.parse().ok()
            } else {
                None::<Url>
            }
        })
        .into_matcher();

        let http = "http://seanmonstar.com";
        let https = "https://hyper.rs";
        let other = "x-youve-never-heard-of-me-mr-proxy://seanmonstar.com";

        assert_eq!(intercepted_uri(&p, http), target2);
        assert_eq!(intercepted_uri(&p, https), target1);
        assert!(p.intercept(&url(other)).is_none());
    }

    #[test]
    fn test_standard_with_custom_auth_header() {
        let target = "http://example.domain/";
        let p = Proxy::all(target)
            .unwrap()
            .custom_http_auth(http::HeaderValue::from_static("testme"))
            .into_matcher();

        let got = p.intercept(&url("http://anywhere.local")).unwrap();
        let auth = got.basic_auth().unwrap();
        assert_eq!(auth, "testme");
    }

    #[test]
    fn test_custom_with_custom_auth_header() {
        let target = "http://example.domain/";
        let p = Proxy::custom(move |_| target.parse::<Url>().ok())
            .custom_http_auth(http::HeaderValue::from_static("testme"))
            .into_matcher();

        let got = p.intercept(&url("http://anywhere.local")).unwrap();
        let auth = got.basic_auth().unwrap();
        assert_eq!(auth, "testme");
    }

    #[test]
    fn test_maybe_has_http_auth() {
        let m = Proxy::all("https://letme:in@yo.local")
            .unwrap()
            .into_matcher();
        assert!(!m.maybe_has_http_auth(), "https always tunnels");

        let m = Proxy::all("http://letme:in@yo.local")
            .unwrap()
            .into_matcher();
        assert!(m.maybe_has_http_auth(), "http forwards");
    }

    #[test]
    fn test_socks_proxy_default_port() {
        {
            let m = Proxy::all("socks5://example.com").unwrap().into_matcher();

            let http = "http://hyper.rs";
            let https = "https://hyper.rs";

            assert_eq!(intercepted_uri(&m, http).port_u16(), Some(1080));
            assert_eq!(intercepted_uri(&m, https).port_u16(), Some(1080));

            // custom port
            let m = Proxy::all("socks5://example.com:1234")
                .unwrap()
                .into_matcher();

            assert_eq!(intercepted_uri(&m, http).port_u16(), Some(1234));
            assert_eq!(intercepted_uri(&m, https).port_u16(), Some(1234));
        }
    }
}
