#![forbid(unsafe_code)]
#![warn(clippy::all)]
// new is just more readable than ..Default::default().
#![allow(clippy::new_without_default)]
// the matches! macro is obscure and not widely known.
#![allow(clippy::match_like_matches_macro)]
// we're not changing public api due to a lint.
#![allow(clippy::upper_case_acronyms)]
#![allow(clippy::result_large_err)]
#![allow(clippy::only_used_in_recursion)]
// println!("{var}") doesn't allow even the simplest expressions for var,
// such as "{foo.var}" – hence this lint forces us to have inconsistent
// formatting args. I prefer a lint that forbid "{var}".
#![allow(clippy::uninlined_format_args)]
// if we want a range, we will make a range.
#![allow(clippy::manual_range_patterns)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

//!<div align="center">
//!  <!-- Version -->
//!  <a href="https://crates.io/crates/ureq">
//!    <img src="https://img.shields.io/crates/v/ureq.svg?style=flat-square"
//!    alt="Crates.io version" />
//!  </a>
//!  <!-- Docs -->
//!  <a href="https://docs.rs/ureq">
//!    <img src="https://img.shields.io/badge/docs-latest-blue.svg?style=flat-square"
//!      alt="docs.rs docs" />
//!  </a>
//!  <!-- Downloads -->
//!  <a href="https://crates.io/crates/ureq">
//!    <img src="https://img.shields.io/crates/d/ureq.svg?style=flat-square"
//!      alt="Crates.io downloads" />
//!  </a>
//!</div>
//!
//! A simple, safe HTTP client.
//!
//! > [!NOTE]
//! > * 2.12.x is MSRV 1.71
//! > * 2.11.x is MSRV 1.67
//! >
//! > For both these lines, we will release patch version pinning dependencies as needed to
//! > retain the MSRV. If we are bumping MSRV, that will require a minor version bump.
//!
//! > [!NOTE]
//! > ureq version 2.11.0 was forced to bump MSRV from 1.63 -> 1.67. The problem is that the
//! > `time` crate 0.3.20, the last 1.63 compatible version, stopped compiling with Rust
//! > [1.80 and above](https://github.com/algesten/ureq/pull/878#issuecomment-2503176155).
//! > To release a 2.x version that is possible to compile on the latest Rust we were
//! > forced to bump MSRV.
//!
//! Ureq's first priority is being easy for you to use. It's great for
//! anyone who wants a low-overhead HTTP client that just gets the job done. Works
//! very well with HTTP APIs. Its features include cookies, JSON, HTTP proxies,
//! HTTPS, interoperability with the `http` crate, and charset decoding.
//!
//! Ureq is in pure Rust for safety and ease of understanding. It avoids using
//! `unsafe` directly. It [uses blocking I/O][blocking] instead of async I/O, because that keeps
//! the API simple and keeps dependencies to a minimum. For TLS, ureq uses
//! [rustls or native-tls](#https--tls--ssl).
//!
//! See the [changelog] for details of recent releases.
//!
//! [blocking]: #blocking-io-for-simplicity
//! [changelog]: https://github.com/algesten/ureq/blob/main/CHANGELOG.md
//!
//!
//! ## Usage
//!
//! In its simplest form, ureq looks like this:
//!
//! ```rust
//! fn main() -> Result<(), ureq::Error> {
//! # ureq::is_test(true);
//!     let body: String = ureq::get("http://example.com")
//!         .set("Example-Header", "header value")
//!         .call()?
//!         .into_string()?;
//!     Ok(())
//! }
//! ```
//!
//! For more involved tasks, you'll want to create an [Agent]. An Agent
//! holds a connection pool for reuse, and a cookie store if you use the
//! "cookies" feature. An Agent can be cheaply cloned due to an internal
//! [Arc](std::sync::Arc) and all clones of an Agent share state among each other. Creating
//! an Agent also allows setting options like the TLS configuration.
//!
//! ```no_run
//! # fn main() -> std::result::Result<(), ureq::Error> {
//! # ureq::is_test(true);
//!   use ureq::{Agent, AgentBuilder};
//!   use std::time::Duration;
//!
//!   let agent: Agent = ureq::AgentBuilder::new()
//!       .timeout_read(Duration::from_secs(5))
//!       .timeout_write(Duration::from_secs(5))
//!       .build();
//!   let body: String = agent.get("http://example.com/page")
//!       .call()?
//!       .into_string()?;
//!
//!   // Reuses the connection from previous request.
//!   let response: String = agent.put("http://example.com/upload")
//!       .set("Authorization", "example-token")
//!       .call()?
//!       .into_string()?;
//! # Ok(())
//! # }
//! ```
//!
//! Ureq supports sending and receiving json, if you enable the "json" feature:
//!
//! ```rust
//! # #[cfg(feature = "json")]
//! # fn main() -> std::result::Result<(), ureq::Error> {
//! # ureq::is_test(true);
//!   // Requires the `json` feature enabled.
//!   let resp: String = ureq::post("http://myapi.example.com/post/ingest")
//!       .set("X-My-Header", "Secret")
//!       .send_json(ureq::json!({
//!           "name": "martin",
//!           "rust": true
//!       }))?
//!       .into_string()?;
//! # Ok(())
//! # }
//! # #[cfg(not(feature = "json"))]
//! # fn main() {}
//! ```
//!
//! ## Error handling
//!
//! ureq returns errors via `Result<T, ureq::Error>`. That includes I/O errors,
//! protocol errors, and status code errors (when the server responded 4xx or
//! 5xx)
//!
//! ```rust
//! use ureq::Error;
//!
//! # fn req() {
//! match ureq::get("http://mypage.example.com/").call() {
//!     Ok(response) => { /* it worked */},
//!     Err(Error::Status(code, response)) => {
//!         /* the server returned an unexpected status
//!            code (such as 400, 500 etc) */
//!     }
//!     Err(_) => { /* some kind of io/transport error */ }
//! }
//! # }
//! # fn main() {}
//! ```
//!
//! More details on the [Error] type.
//!
//! ## Features
//!
//! To enable a minimal dependency tree, some features are off by default.
//! You can control them when including ureq as a dependency.
//!
//! `ureq = { version = "*", features = ["json", "charset"] }`
//!
//! * `tls` enables https. This is enabled by default.
//! * `native-certs` makes the default TLS implementation use the OS' trust store (see TLS doc below).
//! * `cookies` enables cookies.
//! * `json` enables [Response::into_json()] and [Request::send_json()] via serde_json.
//! * `charset` enables interpreting the charset part of the Content-Type header
//!    (e.g.  `Content-Type: text/plain; charset=iso-8859-1`). Without this, the
//!    library defaults to Rust's built in `utf-8`.
//! * `socks-proxy` enables proxy config using the `socks4://`, `socks4a://`, `socks5://` and `socks://` (equal to `socks5://`) prefix.
//! * `native-tls` enables an adapter so you can pass a `native_tls::TlsConnector` instance
//!   to `AgentBuilder::tls_connector`. Due to the risk of diamond dependencies accidentally switching on an unwanted
//!   TLS implementation, `native-tls` is never picked up as a default or used by the crate level
//!   convenience calls (`ureq::get` etc) – it must be configured on the agent. The `native-certs` feature
//!   does nothing for `native-tls`.
//! * `gzip` enables requests of gzip-compressed responses and decompresses them. This is enabled by default.
//! * `brotli` enables requests brotli-compressed responses and decompresses them.
//! * `http-interop` enables conversion methods to and from `http::Response` and `http::request::Builder` (v0.2).
//! * `http` enables conversion methods to and from `http::Response` and `http::request::Builder` (v1.0).
//!
//! # Plain requests
//!
//! Most standard methods (GET, POST, PUT etc), are supported as functions from the
//! top of the library ([get()], [post()], [put()], etc).
//!
//! These top level http method functions create a [Request] instance
//! which follows a build pattern. The builders are finished using:
//!
//! * [`.call()`][Request::call()] without a request body.
//! * [`.send()`][Request::send()] with a request body as [Read][std::io::Read] (chunked encoding support for non-known sized readers).
//! * [`.send_string()`][Request::send_string()] body as string.
//! * [`.send_bytes()`][Request::send_bytes()] body as bytes.
//! * [`.send_form()`][Request::send_form()] key-value pairs as application/x-www-form-urlencoded.
//!
//! # JSON
//!
//! By enabling the `ureq = { version = "*", features = ["json"] }` feature,
//! the library supports serde json.
//!
//! * [`request.send_json()`][Request::send_json()] send body as serde json.
//! * [`response.into_json()`][Response::into_json()] transform response to json.
//!
//! # Content-Length and Transfer-Encoding
//!
//! The library will send a Content-Length header on requests with bodies of
//! known size, in other words, those sent with
//! [`.send_string()`][Request::send_string()],
//! [`.send_bytes()`][Request::send_bytes()],
//! [`.send_form()`][Request::send_form()], or
//! [`.send_json()`][Request::send_json()]. If you send a
//! request body with [`.send()`][Request::send()],
//! which takes a [Read][std::io::Read] of unknown size, ureq will send Transfer-Encoding:
//! chunked, and encode the body accordingly. Bodyless requests
//! (GETs and HEADs) are sent with [`.call()`][Request::call()]
//! and ureq adds neither a Content-Length nor a Transfer-Encoding header.
//!
//! If you set your own Content-Length or Transfer-Encoding header before
//! sending the body, ureq will respect that header by not overriding it,
//! and by encoding the body or not, as indicated by the headers you set.
//!
//! ```
//! let resp = ureq::post("http://my-server.com/ingest")
//!     .set("Transfer-Encoding", "chunked")
//!     .send_string("Hello world");
//! ```
//!
//! # Character encoding
//!
//! By enabling the `ureq = { version = "*", features = ["charset"] }` feature,
//! the library supports sending/receiving other character sets than `utf-8`.
//!
//! For [`response.into_string()`][Response::into_string()] we read the
//! header `Content-Type: text/plain; charset=iso-8859-1` and if it contains a charset
//! specification, we try to decode the body using that encoding. In the absence of, or failing
//! to interpret the charset, we fall back on `utf-8`.
//!
//! Similarly when using [`request.send_string()`][Request::send_string()],
//! we first check if the user has set a `; charset=<whatwg charset>` and attempt
//! to encode the request body using that.
//!
//!
//! # Proxying
//!
//! ureq supports two kinds of proxies,  [`HTTP`] ([`CONNECT`]), [`SOCKS4`] and [`SOCKS5`],
//! the former is always available while the latter must be enabled using the feature
//! `ureq = { version = "*", features = ["socks-proxy"] }`.
//!
//! Proxies settings are configured on an [Agent] (using [AgentBuilder]). All request sent
//! through the agent will be proxied.
//!
//! ## Example using HTTP
//!
//! ```rust
//! fn proxy_example_1() -> std::result::Result<(), ureq::Error> {
//!     // Configure an http connect proxy. Notice we could have used
//!     // the http:// prefix here (it's optional).
//!     let proxy = ureq::Proxy::new("user:password@cool.proxy:9090")?;
//!     let agent = ureq::AgentBuilder::new()
//!         .proxy(proxy)
//!         .build();
//!
//!     // This is proxied.
//!     let resp = agent.get("http://cool.server").call()?;
//!     Ok(())
//! }
//! # fn main() {}
//! ```
//!
//! ## Example using SOCKS5
//!
//! ```rust
//! # #[cfg(feature = "socks-proxy")]
//! fn proxy_example_2() -> std::result::Result<(), ureq::Error> {
//!     // Configure a SOCKS proxy.
//!     let proxy = ureq::Proxy::new("socks5://user:password@cool.proxy:9090")?;
//!     let agent = ureq::AgentBuilder::new()
//!         .proxy(proxy)
//!         .build();
//!
//!     // This is proxied.
//!     let resp = agent.get("http://cool.server").call()?;
//!     Ok(())
//! }
//! # fn main() {}
//! ```
//!
//! # HTTPS / TLS / SSL
//!
//! On platforms that support rustls, ureq uses rustls. On other platforms, native-tls can
//! be manually configured using [`AgentBuilder::tls_connector`].
//!
//! You might want to use native-tls if you need to interoperate with servers that
//! only support less-secure TLS configurations (rustls doesn't support TLS 1.0 and 1.1, for
//! instance). You might also want to use it if you need to validate certificates for IP addresses,
//! which are not currently supported in rustls.
//!
//! Here's an example of constructing an Agent that uses native-tls. It requires the
//! "native-tls" feature to be enabled.
//!
//! ```no_run
//! # #[cfg(feature = "native-tls")]
//! # fn build() -> std::result::Result<(), Box<dyn std::error::Error>> {
//! # ureq::is_test(true);
//!   use std::sync::Arc;
//!   use ureq::Agent;
//!
//!   let agent = ureq::AgentBuilder::new()
//!       .tls_connector(Arc::new(native_tls::TlsConnector::new()?))
//!       .build();
//! # Ok(())
//! # }
//! # fn main() {}
//! ```
//!
//! ## Trusted Roots
//!
//! When you use rustls (`tls` feature), ureq defaults to trusting
//! [webpki-roots](https://docs.rs/webpki-roots/), a
//! copy of the Mozilla Root program that is bundled into your program (and so won't update if your
//! program isn't updated). You can alternately configure
//! [rustls-native-certs](https://docs.rs/rustls-native-certs/) which extracts the roots from your
//! OS' trust store. That means it will update when your OS is updated, and also that it will
//! include locally installed roots.
//!
//! When you use `native-tls`, ureq will use your OS' certificate verifier and root store.
//!
//! # Blocking I/O for simplicity
//!
//! Ureq uses blocking I/O rather than Rust's newer [asynchronous (async) I/O][async]. Async I/O
//! allows serving many concurrent requests without high costs in memory and OS threads. But
//! it comes at a cost in complexity. Async programs need to pull in a runtime (usually
//! [async-std] or [tokio]). They also need async variants of any method that might block, and of
//! [any method that might call another method that might block][what-color]. That means async
//! programs usually have a lot of dependencies - which adds to compile times, and increases
//! risk.
//!
//! The costs of async are worth paying, if you're writing an HTTP server that must serve
//! many many clients with minimal overhead. However, for HTTP _clients_, we believe that the
//! cost is usually not worth paying. The low-cost alternative to async I/O is blocking I/O,
//! which has a different price: it requires an OS thread per concurrent request. However,
//! that price is usually not high: most HTTP clients make requests sequentially, or with
//! low concurrency.
//!
//! That's why ureq uses blocking I/O and plans to stay that way. Other HTTP clients offer both
//! an async API and a blocking API, but we want to offer a blocking API without pulling in all
//! the dependencies required by an async API.
//!
//! [async]: https://rust-lang.github.io/async-book/01_getting_started/02_why_async.html
//! [async-std]: https://github.com/async-rs/async-std#async-std
//! [tokio]: https://github.com/tokio-rs/tokio#tokio
//! [what-color]: https://journal.stuffwithstuff.com/2015/02/01/what-color-is-your-function/
//! [`HTTP`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Proxy_servers_and_tunneling#http_tunneling
//! [`CONNECT`]: https://developer.mozilla.org/en-US/docs/Web/HTTP/Methods/CONNECT
//! [`SOCKS4`]: https://en.wikipedia.org/wiki/SOCKS#SOCKS4
//! [`SOCKS5`]: https://en.wikipedia.org/wiki/SOCKS#SOCKS5
//!
//! ------------------------------------------------------------------------------
//!
//! Ureq is inspired by other great HTTP clients like
//! [superagent](http://visionmedia.github.io/superagent/) and
//! [the fetch API](https://developer.mozilla.org/en-US/docs/Web/API/Fetch_API).
//!
//! If ureq is not what you're looking for, check out these other Rust HTTP clients:
//! [surf](https://crates.io/crates/surf), [reqwest](https://crates.io/crates/reqwest),
//! [isahc](https://crates.io/crates/isahc), [attohttpc](https://crates.io/crates/attohttpc),
//! [actix-web](https://crates.io/crates/actix-web), and [hyper](https://crates.io/crates/hyper).
//!

/// Re-exported rustls crate
///
/// Use this re-export to always get a compatible version of `ClientConfig`.
#[cfg(feature = "tls")]
pub use rustls;

/// Re-exported native-tls crate
///
/// Use this re-export to always get a compatible version of `TlsConnector`.
#[cfg(feature = "native-tls")]
pub use native_tls;

mod agent;
mod body;
mod chunked;
mod error;
mod header;
mod middleware;
mod pool;
mod proxy;
mod request;
mod resolve;
mod response;
mod stream;
mod unit;

// rustls is our default tls engine. If the feature is on, it will be
// used for the shortcut calls the top of the crate (`ureq::get` etc).
#[cfg(feature = "tls")]
mod rtls;

// native-tls is a feature that must be configured via the AgentBuilder.
// it is never picked up as a default (and never used by `ureq::get` etc).
#[cfg(feature = "native-tls")]
mod ntls;

// If we have rustls compiled, that is the default.
#[cfg(feature = "tls")]
pub(crate) fn default_tls_config() -> std::sync::Arc<dyn TlsConnector> {
    rtls::default_tls_config()
}

// Without rustls compiled, we just fail on https when using the shortcut
// calls at the top of the crate (`ureq::get` etc).
#[cfg(not(feature = "tls"))]
pub(crate) fn default_tls_config() -> std::sync::Arc<dyn TlsConnector> {
    use std::sync::Arc;

    struct NoTlsConfig;

    impl TlsConnector for NoTlsConfig {
        fn connect(
            &self,
            _dns_name: &str,
            _io: Box<dyn ReadWrite>,
        ) -> Result<Box<dyn ReadWrite>, crate::error::Error> {
            Err(ErrorKind::UnknownScheme
                .msg("cannot make HTTPS request because no TLS backend is configured"))
        }
    }

    Arc::new(NoTlsConfig)
}

#[cfg(feature = "cookies")]
mod cookies;

#[cfg(feature = "json")]
pub use serde_json::json;
use url::Url;

#[cfg(test)]
mod test;
#[doc(hidden)]
mod testserver;

#[cfg(feature = "http-interop")]
// 0.2 version dependency (deprecated)
mod http_interop;

#[cfg(feature = "http-crate")]
// 1.0 version dependency.
mod http_crate;

pub use crate::agent::Agent;
pub use crate::agent::AgentBuilder;
pub use crate::agent::RedirectAuthHeaders;
pub use crate::error::{Error, ErrorKind, OrAnyStatus, Transport};
pub use crate::middleware::{Middleware, MiddlewareNext};
pub use crate::proxy::Proxy;
pub use crate::request::{Request, RequestUrl};
pub use crate::resolve::Resolver;
pub use crate::response::Response;
pub use crate::stream::{ReadWrite, TlsConnector};

// re-export
#[cfg(feature = "cookies")]
pub use cookie::Cookie;

#[cfg(feature = "json")]
pub use {serde, serde_json};

#[cfg(feature = "json")]
#[deprecated(note = "use ureq::serde_json::Map instead")]
pub type SerdeMap<K, V> = serde_json::Map<K, V>;

#[cfg(feature = "json")]
#[deprecated(note = "use ureq::serde_json::Value instead")]
pub type SerdeValue = serde_json::Value;

#[cfg(feature = "json")]
#[deprecated(note = "use ureq::serde_json::to_value instead")]
pub fn serde_to_value<T: serde::Serialize>(
    value: T,
) -> std::result::Result<serde_json::Value, serde_json::Error> {
    serde_json::to_value(value)
}

use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};

/// Creates an [AgentBuilder].
pub fn builder() -> AgentBuilder {
    AgentBuilder::new()
}

// is_test returns false so long as it has only ever been called with false.
// If it has ever been called with true, it will always return true after that.
// This is a public but hidden function used to allow doctests to use the test_agent.
// Note that we use this approach for doctests rather the #[cfg(test)], because
// doctests are run against a copy of the crate build without cfg(test) set.
// We also can't use #[cfg(doctest)] to do this, because cfg(doctest) is only set
// when collecting doctests, not when building the crate.
#[doc(hidden)]
pub fn is_test(is: bool) -> bool {
    static IS_TEST: Lazy<AtomicBool> = Lazy::new(|| AtomicBool::new(false));
    if is {
        IS_TEST.store(true, Ordering::SeqCst);
    }
    IS_TEST.load(Ordering::SeqCst)
}

/// Agents are used to hold configuration and keep state between requests.
pub fn agent() -> Agent {
    #[cfg(not(test))]
    if is_test(false) {
        testserver::test_agent()
    } else {
        AgentBuilder::new().build()
    }
    #[cfg(test)]
    testserver::test_agent()
}

/// Make a request with the HTTP verb as a parameter.
///
/// This allows making requests with verbs that don't have a dedicated
/// method.
///
/// If you've got an already-parsed [`Url`], try [`request_url()`].
///
/// ```
/// # fn main() -> Result<(), ureq::Error> {
/// # ureq::is_test(true);
/// let resp: ureq::Response = ureq::request("OPTIONS", "http://example.com/")
///     .call()?;
/// # Ok(())
/// # }
/// ```
pub fn request(method: &str, path: &str) -> Request {
    agent().request(method, path)
}
/// Make a request using an already-parsed [Url].
///
/// This is useful if you've got a parsed [`Url`] from some other source, or if
/// you want to parse the URL and then modify it before making the request.
/// If you'd just like to pass a [`String`] or a [`&str`], try [`request()`].
///
/// ```
/// # fn main() -> Result<(), ureq::Error> {
/// # ureq::is_test(true);
/// use url::Url;
/// let agent = ureq::agent();
///
/// let mut url: Url = "http://example.com/some-page".parse()?;
/// url.set_path("/get/robots.txt");
/// let resp: ureq::Response = ureq::request_url("GET", &url)
///     .call()?;
/// # Ok(())
/// # }
/// ```
pub fn request_url(method: &str, url: &Url) -> Request {
    agent().request_url(method, url)
}

/// Make a GET request.
pub fn get(path: &str) -> Request {
    request("GET", path)
}

/// Make a HEAD request.
pub fn head(path: &str) -> Request {
    request("HEAD", path)
}

/// Make a PATCH request.
pub fn patch(path: &str) -> Request {
    request("PATCH", path)
}

/// Make a POST request.
pub fn post(path: &str) -> Request {
    request("POST", path)
}

/// Make a PUT request.
pub fn put(path: &str) -> Request {
    request("PUT", path)
}

/// Make a DELETE request.
pub fn delete(path: &str) -> Request {
    request("DELETE", path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_http_google() {
        let agent = Agent::new();

        let resp = agent.get("http://www.google.com/").call().unwrap();
        assert_eq!(
            "text/html;charset=ISO-8859-1",
            resp.header("content-type").unwrap().replace("; ", ";")
        );
        assert_eq!("text/html", resp.content_type());
    }

    #[test]
    #[cfg(feature = "tls")]
    fn connect_https_google_rustls() {
        let agent = Agent::new();

        let resp = agent.get("https://www.google.com/").call().unwrap();
        assert_eq!(
            "text/html;charset=ISO-8859-1",
            resp.header("content-type").unwrap().replace("; ", ";")
        );
        assert_eq!("text/html", resp.content_type());
    }

    #[test]
    #[cfg(feature = "native-tls")]
    fn connect_https_google_native_tls() {
        use std::sync::Arc;

        let tls_config = native_tls::TlsConnector::new().unwrap();
        let agent = builder().tls_connector(Arc::new(tls_config)).build();

        let resp = agent.get("https://www.google.com/").call().unwrap();
        assert_eq!(
            "text/html;charset=ISO-8859-1",
            resp.header("content-type").unwrap().replace("; ", ";")
        );
        assert_eq!("text/html", resp.content_type());
    }

    #[test]
    fn connect_https_invalid_name() {
        let result = get("https://example.com{REQUEST_URI}/").call();
        let e = ErrorKind::Dns;
        assert_eq!(result.unwrap_err().kind(), e);
    }
}
