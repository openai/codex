//! # hyper-tls
//!
//! An HTTPS connector to be used with [hyper][].
//!
//! [hyper]: https://hyper.rs
//!
//! ## Example
//!
//! ```no_run
//! use bytes::Bytes;
//! use http_body_util::Empty;
//! use hyper_tls::HttpsConnector;
//! use hyper_util::{client::legacy::Client, rt::TokioExecutor};
//!
//! #[tokio::main(flavor = "current_thread")]
//! async fn main() -> Result<(), Box<dyn std::error::Error>>{
//!     let https = HttpsConnector::new();
//!     let client = Client::builder(TokioExecutor::new()).build::<_, Empty<Bytes>>(https);
//!
//!     let res = client.get("https://hyper.rs".parse()?).await?;
//!     assert_eq!(res.status(), 200);
//!     Ok(())
//! }
//! ```
//!
//! ## Crate Features
//!
//! - `alpn`: Enables `native-tls/alpn`, and if `h2` is negotiated, tells hyper.
#![doc(html_root_url = "https://docs.rs/hyper-tls/0.6.0")]
#![cfg_attr(test, deny(warnings))]
#![deny(missing_docs)]
#![deny(missing_debug_implementations)]

#[doc(hidden)]
pub extern crate native_tls;

pub use client::{HttpsConnecting, HttpsConnector};
pub use stream::{MaybeHttpsStream, TlsStream};

mod client;
mod stream;
