//! Proxy matchers
//!
//! This module contains different matchers to configure rules for when a proxy
//! should be used, and if so, with what arguments.
//!
//! A [`Matcher`] can be constructed either using environment variables, or
//! a [`Matcher::builder()`].
//!
//! Once constructed, the `Matcher` can be asked if it intercepts a `Uri` by
//! calling [`Matcher::intercept()`].
//!
//! An [`Intercept`] includes the destination for the proxy, and any parsed
//! authentication to be used.

use std::fmt;
use std::net::IpAddr;

use http::header::HeaderValue;
use ipnet::IpNet;
use percent_encoding::percent_decode_str;

#[cfg(docsrs)]
pub use self::builder::IntoValue;
#[cfg(not(docsrs))]
use self::builder::IntoValue;

/// A proxy matcher, usually built from environment variables.
pub struct Matcher {
    http: Option<Intercept>,
    https: Option<Intercept>,
    no: NoProxy,
}

/// A matched proxy,
///
/// This is returned by a matcher if a proxy should be used.
#[derive(Clone)]
pub struct Intercept {
    uri: http::Uri,
    auth: Auth,
}

/// A builder to create a [`Matcher`].
///
/// Construct with [`Matcher::builder()`].
#[derive(Default)]
pub struct Builder {
    is_cgi: bool,
    all: String,
    http: String,
    https: String,
    no: String,
}

#[derive(Clone)]
enum Auth {
    Empty,
    Basic(http::header::HeaderValue),
    Raw(String, String),
}

/// A filter for proxy matchers.
///
/// This type is based off the `NO_PROXY` rules used by curl.
#[derive(Clone, Debug, Default)]
struct NoProxy {
    ips: IpMatcher,
    domains: DomainMatcher,
}

#[derive(Clone, Debug, Default)]
struct DomainMatcher(Vec<String>);

#[derive(Clone, Debug, Default)]
struct IpMatcher(Vec<Ip>);

#[derive(Clone, Debug)]
enum Ip {
    Address(IpAddr),
    Network(IpNet),
}

// ===== impl Matcher =====

impl Matcher {
    /// Create a matcher reading the current environment variables.
    ///
    /// This checks for values in the following variables, treating them the
    /// same as curl does:
    ///
    /// - `ALL_PROXY`/`all_proxy`
    /// - `HTTPS_PROXY`/`https_proxy`
    /// - `HTTP_PROXY`/`http_proxy`
    /// - `NO_PROXY`/`no_proxy`
    pub fn from_env() -> Self {
        Builder::from_env().build()
    }

    /// Create a matcher from the environment or system.
    ///
    /// This checks the same environment variables as `from_env()`, and if not
    /// set, checks the system configuration for values for the OS.
    ///
    /// This constructor is always available, but if the `client-proxy-system`
    /// feature is enabled, it will check more configuration. Use this
    /// constructor if you want to allow users to optionally enable more, or
    /// use `from_env` if you do not want the values to change based on an
    /// enabled feature.
    pub fn from_system() -> Self {
        Builder::from_system().build()
    }

    /// Start a builder to configure a matcher.
    pub fn builder() -> Builder {
        Builder::default()
    }

    /// Check if the destination should be intercepted by a proxy.
    ///
    /// If the proxy rules match the destination, a new `Uri` will be returned
    /// to connect to.
    pub fn intercept(&self, dst: &http::Uri) -> Option<Intercept> {
        // TODO(perf): don't need to check `no` if below doesn't match...
        if self.no.contains(dst.host()?) {
            return None;
        }

        match dst.scheme_str() {
            Some("http") => self.http.clone(),
            Some("https") => self.https.clone(),
            _ => None,
        }
    }
}

impl fmt::Debug for Matcher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut b = f.debug_struct("Matcher");

        if let Some(ref http) = self.http {
            b.field("http", http);
        }

        if let Some(ref https) = self.https {
            b.field("https", https);
        }

        if !self.no.is_empty() {
            b.field("no", &self.no);
        }
        b.finish()
    }
}

// ===== impl Intercept =====

impl Intercept {
    /// Get the `http::Uri` for the target proxy.
    pub fn uri(&self) -> &http::Uri {
        &self.uri
    }

    /// Get any configured basic authorization.
    ///
    /// This should usually be used with a `Proxy-Authorization` header, to
    /// send in Basic format.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper_util::client::proxy::matcher::Matcher;
    /// # let uri = http::Uri::from_static("https://hyper.rs");
    /// let m = Matcher::builder()
    ///     .all("https://Aladdin:opensesame@localhost:8887")
    ///     .build();
    ///
    /// let proxy = m.intercept(&uri).expect("example");
    /// let auth = proxy.basic_auth().expect("example");
    /// assert_eq!(auth, "Basic QWxhZGRpbjpvcGVuc2VzYW1l");
    /// ```
    pub fn basic_auth(&self) -> Option<&HeaderValue> {
        if let Auth::Basic(ref val) = self.auth {
            Some(val)
        } else {
            None
        }
    }

    /// Get any configured raw authorization.
    ///
    /// If not detected as another scheme, this is the username and password
    /// that should be sent with whatever protocol the proxy handshake uses.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use hyper_util::client::proxy::matcher::Matcher;
    /// # let uri = http::Uri::from_static("https://hyper.rs");
    /// let m = Matcher::builder()
    ///     .all("socks5h://Aladdin:opensesame@localhost:8887")
    ///     .build();
    ///
    /// let proxy = m.intercept(&uri).expect("example");
    /// let auth = proxy.raw_auth().expect("example");
    /// assert_eq!(auth, ("Aladdin", "opensesame"));
    /// ```
    pub fn raw_auth(&self) -> Option<(&str, &str)> {
        if let Auth::Raw(ref u, ref p) = self.auth {
            Some((u.as_str(), p.as_str()))
        } else {
            None
        }
    }
}

impl fmt::Debug for Intercept {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Intercept")
            .field("uri", &self.uri)
            // dont output auth, its sensitive
            .finish()
    }
}

// ===== impl Builder =====

impl Builder {
    fn from_env() -> Self {
        Builder {
            is_cgi: std::env::var_os("REQUEST_METHOD").is_some(),
            all: get_first_env(&["ALL_PROXY", "all_proxy"]),
            http: get_first_env(&["HTTP_PROXY", "http_proxy"]),
            https: get_first_env(&["HTTPS_PROXY", "https_proxy"]),
            no: get_first_env(&["NO_PROXY", "no_proxy"]),
        }
    }

    fn from_system() -> Self {
        #[allow(unused_mut)]
        let mut builder = Self::from_env();

        #[cfg(all(feature = "client-proxy-system", target_os = "macos"))]
        mac::with_system(&mut builder);

        #[cfg(all(feature = "client-proxy-system", windows))]
        win::with_system(&mut builder);

        builder
    }

    /// Set the target proxy for all destinations.
    pub fn all<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.all = val.into_value();
        self
    }

    /// Set the target proxy for HTTP destinations.
    pub fn http<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.http = val.into_value();
        self
    }

    /// Set the target proxy for HTTPS destinations.
    pub fn https<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.https = val.into_value();
        self
    }

    /// Set the "no" proxy filter.
    ///
    /// The rules are as follows:
    /// * Entries are expected to be comma-separated (whitespace between entries is ignored)
    /// * IP addresses (both IPv4 and IPv6) are allowed, as are optional subnet masks (by adding /size,
    ///   for example "`192.168.1.0/24`").
    /// * An entry "`*`" matches all hostnames (this is the only wildcard allowed)
    /// * Any other entry is considered a domain name (and may contain a leading dot, for example `google.com`
    ///   and `.google.com` are equivalent) and would match both that domain AND all subdomains.
    ///
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would match
    /// (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URL `http://notgoogle.com/` would not match.
    pub fn no<S>(mut self, val: S) -> Self
    where
        S: IntoValue,
    {
        self.no = val.into_value();
        self
    }

    /// Construct a [`Matcher`] using the configured values.
    pub fn build(self) -> Matcher {
        if self.is_cgi {
            return Matcher {
                http: None,
                https: None,
                no: NoProxy::empty(),
            };
        }

        let all = parse_env_uri(&self.all);

        Matcher {
            http: parse_env_uri(&self.http).or_else(|| all.clone()),
            https: parse_env_uri(&self.https).or(all),
            no: NoProxy::from_string(&self.no),
        }
    }
}

fn get_first_env(names: &[&str]) -> String {
    for name in names {
        if let Ok(val) = std::env::var(name) {
            return val;
        }
    }

    String::new()
}

fn parse_env_uri(val: &str) -> Option<Intercept> {
    let uri = val.parse::<http::Uri>().ok()?;
    let mut builder = http::Uri::builder();
    let mut is_httpish = false;
    let mut auth = Auth::Empty;

    builder = builder.scheme(match uri.scheme() {
        Some(s) => {
            if s == &http::uri::Scheme::HTTP || s == &http::uri::Scheme::HTTPS {
                is_httpish = true;
                s.clone()
            } else if matches!(s.as_str(), "socks4" | "socks4a" | "socks5" | "socks5h") {
                s.clone()
            } else {
                // can't use this proxy scheme
                return None;
            }
        }
        // if no scheme provided, assume they meant 'http'
        None => {
            is_httpish = true;
            http::uri::Scheme::HTTP
        }
    });

    let authority = uri.authority()?;

    if let Some((userinfo, host_port)) = authority.as_str().split_once('@') {
        let (user, pass) = userinfo.split_once(':')?;
        let user = percent_decode_str(user).decode_utf8_lossy();
        let pass = percent_decode_str(pass).decode_utf8_lossy();
        if is_httpish {
            auth = Auth::Basic(encode_basic_auth(&user, Some(&pass)));
        } else {
            auth = Auth::Raw(user.into(), pass.into());
        }
        builder = builder.authority(host_port);
    } else {
        builder = builder.authority(authority.clone());
    }

    // removing any path, but we MUST specify one or the builder errors
    builder = builder.path_and_query("/");

    let dst = builder.build().ok()?;

    Some(Intercept { uri: dst, auth })
}

fn encode_basic_auth(user: &str, pass: Option<&str>) -> HeaderValue {
    use base64::prelude::BASE64_STANDARD;
    use base64::write::EncoderWriter;
    use std::io::Write;

    let mut buf = b"Basic ".to_vec();
    {
        let mut encoder = EncoderWriter::new(&mut buf, &BASE64_STANDARD);
        let _ = write!(encoder, "{user}:");
        if let Some(password) = pass {
            let _ = write!(encoder, "{password}");
        }
    }
    let mut header = HeaderValue::from_bytes(&buf).expect("base64 is always valid HeaderValue");
    header.set_sensitive(true);
    header
}

impl NoProxy {
    /*
    fn from_env() -> NoProxy {
        let raw = std::env::var("NO_PROXY")
            .or_else(|_| std::env::var("no_proxy"))
            .unwrap_or_default();

        Self::from_string(&raw)
    }
    */

    fn empty() -> NoProxy {
        NoProxy {
            ips: IpMatcher(Vec::new()),
            domains: DomainMatcher(Vec::new()),
        }
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
    /// For example, if `"NO_PROXY=google.com, 192.168.1.0/24"` was set, all of the following would match
    /// (and therefore would bypass the proxy):
    /// * `http://google.com/`
    /// * `http://www.google.com/`
    /// * `http://192.168.1.42/`
    ///
    /// The URL `http://notgoogle.com/` would not match.
    pub fn from_string(no_proxy_list: &str) -> Self {
        let mut ips = Vec::new();
        let mut domains = Vec::new();
        let parts = no_proxy_list.split(',').map(str::trim);
        for part in parts {
            match part.parse::<IpNet>() {
                // If we can parse an IP net or address, then use it, otherwise, assume it is a domain
                Ok(ip) => ips.push(Ip::Network(ip)),
                Err(_) => match part.parse::<IpAddr>() {
                    Ok(addr) => ips.push(Ip::Address(addr)),
                    Err(_) => {
                        if !part.trim().is_empty() {
                            domains.push(part.to_owned())
                        }
                    }
                },
            }
        }
        NoProxy {
            ips: IpMatcher(ips),
            domains: DomainMatcher(domains),
        }
    }

    /// Return true if this matches the host (domain or IP).
    pub fn contains(&self, host: &str) -> bool {
        // According to RFC3986, raw IPv6 hosts will be wrapped in []. So we need to strip those off
        // the end in order to parse correctly
        let host = if host.starts_with('[') {
            let x: &[_] = &['[', ']'];
            host.trim_matches(x)
        } else {
            host
        };
        match host.parse::<IpAddr>() {
            // If we can parse an IP addr, then use it, otherwise, assume it is a domain
            Ok(ip) => self.ips.contains(ip),
            Err(_) => self.domains.contains(host),
        }
    }

    fn is_empty(&self) -> bool {
        self.ips.0.is_empty() && self.domains.0.is_empty()
    }
}

impl IpMatcher {
    fn contains(&self, addr: IpAddr) -> bool {
        for ip in &self.0 {
            match ip {
                Ip::Address(address) => {
                    if &addr == address {
                        return true;
                    }
                }
                Ip::Network(net) => {
                    if net.contains(&addr) {
                        return true;
                    }
                }
            }
        }
        false
    }
}

impl DomainMatcher {
    // The following links may be useful to understand the origin of these rules:
    // * https://curl.se/libcurl/c/CURLOPT_NOPROXY.html
    // * https://github.com/curl/curl/issues/1208
    fn contains(&self, domain: &str) -> bool {
        let domain_len = domain.len();
        for d in &self.0 {
            if d == domain || d.strip_prefix('.') == Some(domain) {
                return true;
            } else if domain.ends_with(d) {
                if d.starts_with('.') {
                    // If the first character of d is a dot, that means the first character of domain
                    // must also be a dot, so we are looking at a subdomain of d and that matches
                    return true;
                } else if domain.as_bytes().get(domain_len - d.len() - 1) == Some(&b'.') {
                    // Given that d is a prefix of domain, if the prior character in domain is a dot
                    // then that means we must be matching a subdomain of d, and that matches
                    return true;
                }
            } else if d == "*" {
                return true;
            }
        }
        false
    }
}

mod builder {
    /// A type that can used as a `Builder` value.
    ///
    /// Private and sealed, only visible in docs.
    pub trait IntoValue {
        #[doc(hidden)]
        fn into_value(self) -> String;
    }

    impl IntoValue for String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self
        }
    }

    impl IntoValue for &String {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }

    impl IntoValue for &str {
        #[doc(hidden)]
        fn into_value(self) -> String {
            self.into()
        }
    }
}

#[cfg(feature = "client-proxy-system")]
#[cfg(target_os = "macos")]
mod mac {
    use system_configuration::core_foundation::base::{CFType, TCFType, TCFTypeRef};
    use system_configuration::core_foundation::dictionary::CFDictionary;
    use system_configuration::core_foundation::number::CFNumber;
    use system_configuration::core_foundation::string::{CFString, CFStringRef};
    use system_configuration::dynamic_store::SCDynamicStoreBuilder;
    use system_configuration::sys::schema_definitions::{
        kSCPropNetProxiesHTTPEnable, kSCPropNetProxiesHTTPPort, kSCPropNetProxiesHTTPProxy,
        kSCPropNetProxiesHTTPSEnable, kSCPropNetProxiesHTTPSPort, kSCPropNetProxiesHTTPSProxy,
    };

    pub(super) fn with_system(builder: &mut super::Builder) {
        let store = SCDynamicStoreBuilder::new("").build();

        let proxies_map = if let Some(proxies_map) = store.get_proxies() {
            proxies_map
        } else {
            return;
        };

        if builder.http.is_empty() {
            let http_proxy_config = parse_setting_from_dynamic_store(
                &proxies_map,
                unsafe { kSCPropNetProxiesHTTPEnable },
                unsafe { kSCPropNetProxiesHTTPProxy },
                unsafe { kSCPropNetProxiesHTTPPort },
            );
            if let Some(http) = http_proxy_config {
                builder.http = http;
            }
        }

        if builder.https.is_empty() {
            let https_proxy_config = parse_setting_from_dynamic_store(
                &proxies_map,
                unsafe { kSCPropNetProxiesHTTPSEnable },
                unsafe { kSCPropNetProxiesHTTPSProxy },
                unsafe { kSCPropNetProxiesHTTPSPort },
            );

            if let Some(https) = https_proxy_config {
                builder.https = https;
            }
        }
    }

    fn parse_setting_from_dynamic_store(
        proxies_map: &CFDictionary<CFString, CFType>,
        enabled_key: CFStringRef,
        host_key: CFStringRef,
        port_key: CFStringRef,
    ) -> Option<String> {
        let proxy_enabled = proxies_map
            .find(enabled_key)
            .and_then(|flag| flag.downcast::<CFNumber>())
            .and_then(|flag| flag.to_i32())
            .unwrap_or(0)
            == 1;

        if proxy_enabled {
            let proxy_host = proxies_map
                .find(host_key)
                .and_then(|host| host.downcast::<CFString>())
                .map(|host| host.to_string());
            let proxy_port = proxies_map
                .find(port_key)
                .and_then(|port| port.downcast::<CFNumber>())
                .and_then(|port| port.to_i32());

            return match (proxy_host, proxy_port) {
                (Some(proxy_host), Some(proxy_port)) => Some(format!("{proxy_host}:{proxy_port}")),
                (Some(proxy_host), None) => Some(proxy_host),
                (None, Some(_)) => None,
                (None, None) => None,
            };
        }

        None
    }
}

#[cfg(feature = "client-proxy-system")]
#[cfg(windows)]
mod win {
    pub(super) fn with_system(builder: &mut super::Builder) {
        let settings = if let Ok(settings) = windows_registry::CURRENT_USER
            .open("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        {
            settings
        } else {
            return;
        };

        if settings.get_u32("ProxyEnable").unwrap_or(0) == 0 {
            return;
        }

        if let Ok(val) = settings.get_string("ProxyServer") {
            if builder.http.is_empty() {
                builder.http = val.clone();
            }
            if builder.https.is_empty() {
                builder.https = val;
            }
        }

        if builder.no.is_empty() {
            if let Ok(val) = settings.get_string("ProxyOverride") {
                builder.no = val
                    .split(';')
                    .map(|s| s.trim())
                    .collect::<Vec<&str>>()
                    .join(",")
                    .replace("*.", "");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_matcher() {
        let domains = vec![".foo.bar".into(), "bar.foo".into()];
        let matcher = DomainMatcher(domains);

        // domains match with leading `.`
        assert!(matcher.contains("foo.bar"));
        // subdomains match with leading `.`
        assert!(matcher.contains("www.foo.bar"));

        // domains match with no leading `.`
        assert!(matcher.contains("bar.foo"));
        // subdomains match with no leading `.`
        assert!(matcher.contains("www.bar.foo"));

        // non-subdomain string prefixes don't match
        assert!(!matcher.contains("notfoo.bar"));
        assert!(!matcher.contains("notbar.foo"));
    }

    #[test]
    fn test_no_proxy_wildcard() {
        let no_proxy = NoProxy::from_string("*");
        assert!(no_proxy.contains("any.where"));
    }

    #[test]
    fn test_no_proxy_ip_ranges() {
        let no_proxy =
            NoProxy::from_string(".foo.bar, bar.baz,10.42.1.1/24,::1,10.124.7.8,2001::/17");

        let should_not_match = [
            // random url, not in no_proxy
            "hyper.rs",
            // make sure that random non-subdomain string prefixes don't match
            "notfoo.bar",
            // make sure that random non-subdomain string prefixes don't match
            "notbar.baz",
            // ipv4 address out of range
            "10.43.1.1",
            // ipv4 address out of range
            "10.124.7.7",
            // ipv6 address out of range
            "[ffff:db8:a0b:12f0::1]",
            // ipv6 address out of range
            "[2005:db8:a0b:12f0::1]",
        ];

        for host in &should_not_match {
            assert!(!no_proxy.contains(host), "should not contain {host:?}");
        }

        let should_match = [
            // make sure subdomains (with leading .) match
            "hello.foo.bar",
            // make sure exact matches (without leading .) match (also makes sure spaces between entries work)
            "bar.baz",
            // make sure subdomains (without leading . in no_proxy) match
            "foo.bar.baz",
            // make sure subdomains (without leading . in no_proxy) match - this differs from cURL
            "foo.bar",
            // ipv4 address match within range
            "10.42.1.100",
            // ipv6 address exact match
            "[::1]",
            // ipv6 address match within range
            "[2001:db8:a0b:12f0::1]",
            // ipv4 address exact match
            "10.124.7.8",
        ];

        for host in &should_match {
            assert!(no_proxy.contains(host), "should contain {host:?}");
        }
    }

    macro_rules! p {
        ($($n:ident = $v:expr,)*) => ({Builder {
            $($n: $v.into(),)*
            ..Builder::default()
        }.build()});
    }

    fn intercept(p: &Matcher, u: &str) -> Intercept {
        p.intercept(&u.parse().unwrap()).unwrap()
    }

    #[test]
    fn test_all_proxy() {
        let p = p! {
            all = "http://om.nom",
        };

        assert_eq!("http://om.nom", intercept(&p, "http://example.com").uri());

        assert_eq!("http://om.nom", intercept(&p, "https://example.com").uri());
    }

    #[test]
    fn test_specific_overrides_all() {
        let p = p! {
            all = "http://no.pe",
            http = "http://y.ep",
        };

        assert_eq!("http://no.pe", intercept(&p, "https://example.com").uri());

        // the http rule is "more specific" than the all rule
        assert_eq!("http://y.ep", intercept(&p, "http://example.com").uri());
    }

    #[test]
    fn test_parse_no_scheme_defaults_to_http() {
        let p = p! {
            https = "y.ep",
            http = "127.0.0.1:8887",
        };

        assert_eq!(intercept(&p, "https://example.local").uri(), "http://y.ep");
        assert_eq!(
            intercept(&p, "http://example.local").uri(),
            "http://127.0.0.1:8887"
        );
    }

    #[test]
    fn test_parse_http_auth() {
        let p = p! {
            all = "http://Aladdin:opensesame@y.ep",
        };

        let proxy = intercept(&p, "https://example.local");
        assert_eq!(proxy.uri(), "http://y.ep");
        assert_eq!(
            proxy.basic_auth().expect("basic_auth"),
            "Basic QWxhZGRpbjpvcGVuc2VzYW1l"
        );
    }

    #[test]
    fn test_parse_http_auth_without_scheme() {
        let p = p! {
            all = "Aladdin:opensesame@y.ep",
        };

        let proxy = intercept(&p, "https://example.local");
        assert_eq!(proxy.uri(), "http://y.ep");
        assert_eq!(
            proxy.basic_auth().expect("basic_auth"),
            "Basic QWxhZGRpbjpvcGVuc2VzYW1l"
        );
    }

    #[test]
    fn test_dont_parse_http_when_is_cgi() {
        let mut builder = Matcher::builder();
        builder.is_cgi = true;
        builder.http = "http://never.gonna.let.you.go".into();
        let m = builder.build();

        assert!(m.intercept(&"http://rick.roll".parse().unwrap()).is_none());
    }
}
