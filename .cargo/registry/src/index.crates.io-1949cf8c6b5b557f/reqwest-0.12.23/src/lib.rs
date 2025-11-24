#![deny(missing_docs)]
#![deny(missing_debug_implementations)]
#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]
#![cfg_attr(test, deny(warnings))]

//! # reqwest
//!
//! The `reqwest` crate provides a convenient, higher-level HTTP
//! [`Client`][client].
//!
//! It handles many of the things that most people just expect an HTTP client
//! to do for them.
//!
//! - Async and [blocking] Clients
//! - Plain bodies, [JSON](#json), [urlencoded](#forms), [multipart]
//! - Customizable [redirect policy](#redirect-policies)
//! - HTTP [Proxies](#proxies)
//! - Uses [TLS](#tls) by default
//! - Cookies
//!
//! The [`reqwest::Client`][client] is asynchronous (requiring Tokio). For
//! applications wishing  to only make a few HTTP requests, the
//! [`reqwest::blocking`](blocking) API may be more convenient.
//!
//! Additional learning resources include:
//!
//! - [The Rust Cookbook](https://rust-lang-nursery.github.io/rust-cookbook/web/clients.html)
//! - [reqwest Repository Examples](https://github.com/seanmonstar/reqwest/tree/master/examples)
//!
//! ## Commercial Support
//!
//! For private advice, support, reviews, access to the maintainer, and the
//! like, reach out for [commercial support][sponsor].
//!
//! ## Making a GET request
//!
//! For a single request, you can use the [`get`][get] shortcut method.
//!
//! ```rust
//! # async fn run() -> Result<(), reqwest::Error> {
//! let body = reqwest::get("https://www.rust-lang.org")
//!     .await?
//!     .text()
//!     .await?;
//!
//! println!("body = {body:?}");
//! # Ok(())
//! # }
//! ```
//!
//! **NOTE**: If you plan to perform multiple requests, it is best to create a
//! [`Client`][client] and reuse it, taking advantage of keep-alive connection
//! pooling.
//!
//! ## Making POST requests (or setting request bodies)
//!
//! There are several ways you can set the body of a request. The basic one is
//! by using the `body()` method of a [`RequestBuilder`][builder]. This lets you set the
//! exact raw bytes of what the body should be. It accepts various types,
//! including `String` and `Vec<u8>`. If you wish to pass a custom
//! type, you can use the `reqwest::Body` constructors.
//!
//! ```rust
//! # use reqwest::Error;
//! #
//! # async fn run() -> Result<(), Error> {
//! let client = reqwest::Client::new();
//! let res = client.post("http://httpbin.org/post")
//!     .body("the exact body that is sent")
//!     .send()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### Forms
//!
//! It's very common to want to send form data in a request body. This can be
//! done with any type that can be serialized into form data.
//!
//! This can be an array of tuples, or a `HashMap`, or a custom type that
//! implements [`Serialize`][serde].
//!
//! ```rust
//! # use reqwest::Error;
//! #
//! # async fn run() -> Result<(), Error> {
//! // This will POST a body of `foo=bar&baz=quux`
//! let params = [("foo", "bar"), ("baz", "quux")];
//! let client = reqwest::Client::new();
//! let res = client.post("http://httpbin.org/post")
//!     .form(&params)
//!     .send()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ### JSON
//!
//! There is also a `json` method helper on the [`RequestBuilder`][builder] that works in
//! a similar fashion the `form` method. It can take any value that can be
//! serialized into JSON. The feature `json` is required.
//!
//! ```rust
//! # use reqwest::Error;
//! # use std::collections::HashMap;
//! #
//! # #[cfg(feature = "json")]
//! # async fn run() -> Result<(), Error> {
//! // This will POST a body of `{"lang":"rust","body":"json"}`
//! let mut map = HashMap::new();
//! map.insert("lang", "rust");
//! map.insert("body", "json");
//!
//! let client = reqwest::Client::new();
//! let res = client.post("http://httpbin.org/post")
//!     .json(&map)
//!     .send()
//!     .await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Redirect Policies
//!
//! By default, a `Client` will automatically handle HTTP redirects, having a
//! maximum redirect chain of 10 hops. To customize this behavior, a
//! [`redirect::Policy`][redirect] can be used with a `ClientBuilder`.
//!
//! ## Cookies
//!
//! The automatic storing and sending of session cookies can be enabled with
//! the [`cookie_store`][ClientBuilder::cookie_store] method on `ClientBuilder`.
//!
//! ## Proxies
//!
//! **NOTE**: System proxies are enabled by default.
//!
//! System proxies look in environment variables to set HTTP or HTTPS proxies.
//!
//! `HTTP_PROXY` or `http_proxy` provide HTTP proxies for HTTP connections while
//! `HTTPS_PROXY` or `https_proxy` provide HTTPS proxies for HTTPS connections.
//! `ALL_PROXY` or `all_proxy` provide proxies for both HTTP and HTTPS connections.
//! If both the all proxy and HTTP or HTTPS proxy variables are set the more specific
//! HTTP or HTTPS proxies take precedence.
//!
//! These can be overwritten by adding a [`Proxy`] to `ClientBuilder`
//! i.e. `let proxy = reqwest::Proxy::http("https://secure.example")?;`
//! or disabled by calling `ClientBuilder::no_proxy()`.
//!
//! `socks` feature is required if you have configured socks proxy like this:
//!
//! ```bash
//! export https_proxy=socks5://127.0.0.1:1086
//! ```
//!
//! ## TLS
//!
//! A `Client` will use transport layer security (TLS) by default to connect to
//! HTTPS destinations.
//!
//! - Additional server certificates can be configured on a `ClientBuilder`
//!   with the [`Certificate`] type.
//! - Client certificates can be added to a `ClientBuilder` with the
//!   [`Identity`] type.
//! - Various parts of TLS can also be configured or even disabled on the
//!   `ClientBuilder`.
//!
//! See more details in the [`tls`] module.
//!
//! ## WASM
//!
//! The Client implementation automatically switches to the WASM one when the target_arch is wasm32,
//! the usage is basically the same as the async api. Some of the features are disabled in wasm
//! : [`tls`], [`cookie`], [`blocking`], as well as various `ClientBuilder` methods such as `timeout()` and `connector_layer()`.
//!
//! TLS and cookies are provided through the browser environment, so reqwest can issue TLS requests with cookies,
//! but has limited configuration.
//!
//! ## Optional Features
//!
//! The following are a list of [Cargo features][cargo-features] that can be
//! enabled or disabled:
//!
//! - **http2** *(enabled by default)*: Enables HTTP/2 support.
//! - **default-tls** *(enabled by default)*: Provides TLS support to connect
//!   over HTTPS.
//! - **native-tls**: Enables TLS functionality provided by `native-tls`.
//! - **native-tls-vendored**: Enables the `vendored` feature of `native-tls`.
//! - **native-tls-alpn**: Enables the `alpn` feature of `native-tls`.
//! - **rustls-tls**: Enables TLS functionality provided by `rustls`.
//!   Equivalent to `rustls-tls-webpki-roots`.
//! - **rustls-tls-manual-roots**: Enables TLS functionality provided by `rustls`,
//!   without setting any root certificates. Roots have to be specified manually.
//! - **rustls-tls-webpki-roots**: Enables TLS functionality provided by `rustls`,
//!   while using root certificates from the `webpki-roots` crate.
//! - **rustls-tls-native-roots**: Enables TLS functionality provided by `rustls`,
//!   while using root certificates from the `rustls-native-certs` crate.
//! - **blocking**: Provides the [blocking][] client API.
//! - **charset** *(enabled by default)*: Improved support for decoding text.
//! - **cookies**: Provides cookie session support.
//! - **gzip**: Provides response body gzip decompression.
//! - **brotli**: Provides response body brotli decompression.
//! - **zstd**: Provides response body zstd decompression.
//! - **deflate**: Provides response body deflate decompression.
//! - **json**: Provides serialization and deserialization for JSON bodies.
//! - **multipart**: Provides functionality for multipart forms.
//! - **stream**: Adds support for `futures::Stream`.
//! - **socks**: Provides SOCKS5 proxy support.
//! - **hickory-dns**: Enables a hickory-dns async resolver instead of default
//!   threadpool using `getaddrinfo`.
//! - **system-proxy** *(enabled by default)*: Use Windows and macOS system
//!   proxy settings automatically.
//!
//! ## Unstable Features
//!
//! Some feature flags require additional opt-in by the application, by setting
//! a `reqwest_unstable` flag.
//!
//! - **http3** *(unstable)*: Enables support for sending HTTP/3 requests.
//!
//! These features are unstable, and experimental. Details about them may be
//! changed in patch releases.
//!
//! You can pass such a flag to the compiler via `.cargo/config`, or
//! environment variables, such as:
//!
//! ```notrust
//! RUSTFLAGS="--cfg reqwest_unstable" cargo build
//! ```
//!
//! ## Sponsors
//!
//! Support this project by becoming a [sponsor][].
//!
//! [hyper]: https://hyper.rs
//! [blocking]: ./blocking/index.html
//! [client]: ./struct.Client.html
//! [response]: ./struct.Response.html
//! [get]: ./fn.get.html
//! [builder]: ./struct.RequestBuilder.html
//! [serde]: http://serde.rs
//! [redirect]: crate::redirect
//! [Proxy]: ./struct.Proxy.html
//! [cargo-features]: https://doc.rust-lang.org/stable/cargo/reference/manifest.html#the-features-section
//! [sponsor]: https://seanmonstar.com/sponsor

#[cfg(all(feature = "http3", not(reqwest_unstable)))]
compile_error!(
    "\
    The `http3` feature is unstable, and requires the \
    `RUSTFLAGS='--cfg reqwest_unstable'` environment variable to be set.\
"
);

// Ignore `unused_crate_dependencies` warnings.
// Used in many features that they're not worth making it optional.
use futures_core as _;
use sync_wrapper as _;

macro_rules! if_wasm {
    ($($item:item)*) => {$(
        #[cfg(target_arch = "wasm32")]
        $item
    )*}
}

macro_rules! if_hyper {
    ($($item:item)*) => {$(
        #[cfg(not(target_arch = "wasm32"))]
        $item
    )*}
}

pub use http::header;
pub use http::Method;
pub use http::{StatusCode, Version};
pub use url::Url;

// universal mods
#[macro_use]
mod error;
// TODO: remove `if_hyper` if wasm has been migrated to new config system.
if_hyper! {
    mod config;
}
mod into_url;
mod response;

pub use self::error::{Error, Result};
pub use self::into_url::IntoUrl;
pub use self::response::ResponseBuilderExt;

/// Shortcut method to quickly make a `GET` request.
///
/// See also the methods on the [`reqwest::Response`](./struct.Response.html)
/// type.
///
/// **NOTE**: This function creates a new internal `Client` on each call,
/// and so should not be used if making many requests. Create a
/// [`Client`](./struct.Client.html) instead.
///
/// # Examples
///
/// ```rust
/// # async fn run() -> Result<(), reqwest::Error> {
/// let body = reqwest::get("https://www.rust-lang.org").await?
///     .text().await?;
/// # Ok(())
/// # }
/// ```
///
/// # Errors
///
/// This function fails if:
///
/// - native TLS backend cannot be initialized
/// - supplied `Url` cannot be parsed
/// - there was an error while sending request
/// - redirect limit was exhausted
pub async fn get<T: IntoUrl>(url: T) -> crate::Result<Response> {
    Client::builder().build()?.get(url).send().await
}

fn _assert_impls() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    fn assert_clone<T: Clone>() {}

    assert_send::<Client>();
    assert_sync::<Client>();
    assert_clone::<Client>();

    assert_send::<Request>();
    assert_send::<RequestBuilder>();

    #[cfg(not(target_arch = "wasm32"))]
    {
        assert_send::<Response>();
    }

    assert_send::<Error>();
    assert_sync::<Error>();

    assert_send::<Body>();
    assert_sync::<Body>();
}

if_hyper! {
    #[cfg(test)]
    #[macro_use]
    extern crate doc_comment;

    #[cfg(test)]
    doctest!("../README.md");

    pub use self::async_impl::{
        Body, Client, ClientBuilder, Request, RequestBuilder, Response, Upgraded,
    };
    pub use self::proxy::{Proxy,NoProxy};
    #[cfg(feature = "__tls")]
    // Re-exports, to be removed in a future release
    pub use tls::{Certificate, Identity};
    #[cfg(feature = "multipart")]
    pub use self::async_impl::multipart;


    mod async_impl;
    #[cfg(feature = "blocking")]
    pub mod blocking;
    mod connect;
    #[cfg(feature = "cookies")]
    pub mod cookie;
    pub mod dns;
    mod proxy;
    pub mod redirect;
    pub mod retry;
    #[cfg(feature = "__tls")]
    pub mod tls;
    mod util;

    #[cfg(docsrs)]
    pub use connect::uds::UnixSocketProvider;
}

if_wasm! {
    mod wasm;
    mod util;

    pub use self::wasm::{Body, Client, ClientBuilder, Request, RequestBuilder, Response};
    #[cfg(feature = "multipart")]
    pub use self::wasm::multipart;
}
