//! # hyper-rustls
//!
//! A pure-Rust HTTPS connector for [hyper](https://hyper.rs), based on
//! [Rustls](https://github.com/rustls/rustls).
//!
//! ## Example client
//!
//! ```no_run
//! # #[cfg(all(feature = "rustls-native-certs", feature = "http1"))]
//! # fn main() {
//! use http::StatusCode;
//! use http_body_util::Empty;
//! use hyper::body::Bytes;
//! use hyper_util::client::legacy::Client;
//! use hyper_util::rt::TokioExecutor;
//!
//! let mut rt = tokio::runtime::Runtime::new().unwrap();
//! let url = ("https://hyper.rs").parse().unwrap();
//! let https = hyper_rustls::HttpsConnectorBuilder::new()
//!     .with_native_roots()
//!     .expect("no native root CA certificates found")
//!     .https_only()
//!     .enable_http1()
//!     .build();
//!
//! let client: Client<_, Empty<Bytes>> = Client::builder(TokioExecutor::new()).build(https);
//!
//! let res = rt.block_on(client.get(url)).unwrap();
//! assert_eq!(res.status(), StatusCode::OK);
//! # }
//! # #[cfg(not(all(feature = "rustls-native-certs", feature = "http1")))]
//! # fn main() {}
//! ```

#![warn(missing_docs, unreachable_pub, clippy::use_self)]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]

mod config;
mod connector;
mod stream;

#[cfg(feature = "logging")]
mod log {
    #[cfg(any(feature = "rustls-native-certs", feature = "webpki-roots"))]
    pub(crate) use log::debug;
    #[cfg(feature = "rustls-native-certs")]
    pub(crate) use log::warn;
}

#[cfg(not(feature = "logging"))]
mod log {
    #[cfg(any(feature = "rustls-native-certs", feature = "webpki-roots"))]
    macro_rules! debug    ( ($($tt:tt)*) => {{}} );
    #[cfg(any(feature = "rustls-native-certs", feature = "webpki-roots"))]
    pub(crate) use debug;
    #[cfg(feature = "rustls-native-certs")]
    macro_rules! warn_    ( ($($tt:tt)*) => {{}} );
    #[cfg(feature = "rustls-native-certs")]
    pub(crate) use warn_ as warn;
}

pub use crate::config::ConfigBuilderExt;
pub use crate::connector::builder::ConnectorBuilder as HttpsConnectorBuilder;
pub use crate::connector::{
    DefaultServerNameResolver, FixedServerNameResolver, HttpsConnector, ResolveServerName,
};
pub use crate::stream::MaybeHttpsStream;

/// The various states of the [`HttpsConnectorBuilder`]
pub mod builderstates {
    #[cfg(feature = "http2")]
    pub use crate::connector::builder::WantsProtocols3;
    pub use crate::connector::builder::{
        WantsProtocols1, WantsProtocols2, WantsSchemes, WantsTlsConfig,
    };
}
