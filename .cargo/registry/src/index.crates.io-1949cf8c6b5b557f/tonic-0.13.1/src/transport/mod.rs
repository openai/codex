//! Batteries included server and client.
//!
//! This module provides a set of batteries included, fully featured and
//! fast set of HTTP/2 server and client's. These components each provide a
//! `rustls` tls backend when the respective feature flag is enabled, and
//! provides builders to configure transport behavior.
//!
//! # Features
//!
//! - TLS support via [rustls].
//! - Load balancing
//! - Timeouts
//! - Concurrency Limits
//! - Rate limiting
//!
//! # Examples
//!
//! ## Client
//!
//! ```no_run
//! # #[cfg(feature = "rustls")]
//! # use tonic::transport::{Channel, Certificate, ClientTlsConfig};
//! # use std::time::Duration;
//! # use tonic::client::GrpcService;;
//! # use http::Request;
//! # #[cfg(feature = "rustls")]
//! # async fn do_thing() -> Result<(), Box<dyn std::error::Error>> {
//! let cert = std::fs::read_to_string("ca.pem")?;
//!
//! let mut channel = Channel::from_static("https://example.com")
//!     .tls_config(ClientTlsConfig::new()
//!         .ca_certificate(Certificate::from_pem(&cert))
//!         .domain_name("example.com".to_string()))?
//!     .timeout(Duration::from_secs(5))
//!     .rate_limit(5, Duration::from_secs(1))
//!     .concurrency_limit(256)
//!     .connect()
//!     .await?;
//!
//! channel.call(Request::new(tonic::body::empty_body())).await?;
//! # Ok(())
//! # }
//! ```
//!
//! ## Server
//!
//! ```no_run
//! # use std::convert::Infallible;
//! # #[cfg(feature = "rustls")]
//! # use tonic::transport::{Server, Identity, ServerTlsConfig};
//! # use tonic::body::Body;
//! # use tower::Service;
//! # #[cfg(feature = "rustls")]
//! # async fn do_thing() -> Result<(), Box<dyn std::error::Error>> {
//! # #[derive(Clone)]
//! # pub struct Svc;
//! # impl Service<hyper::Request<Body>> for Svc {
//! #   type Response = hyper::Response<Body>;
//! #   type Error = Infallible;
//! #   type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
//! #   fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
//! #       Ok(()).into()
//! #  }
//! #   fn call(&mut self, _req: hyper::Request<Body>) -> Self::Future {
//! #       unimplemented!()
//! #   }
//! # }
//! # impl tonic::server::NamedService for Svc {
//! # const NAME: &'static str = "some_svc";
//! # }
//! # let my_svc = Svc;
//! let cert = std::fs::read_to_string("server.pem")?;
//! let key = std::fs::read_to_string("server.key")?;
//!
//! let addr = "[::1]:50051".parse()?;
//!
//! Server::builder()
//!     .tls_config(ServerTlsConfig::new()
//!         .identity(Identity::from_pem(&cert, &key)))?
//!     .concurrency_limit_per_connection(256)
//!     .add_service(my_svc)
//!     .serve(addr)
//!     .await?;
//!
//! # Ok(())
//! # }
//! ```
//!
//! [rustls]: https://docs.rs/rustls/0.16.0/rustls/

#[cfg(feature = "channel")]
pub mod channel;
#[cfg(feature = "server")]
pub mod server;

mod error;
mod service;
#[cfg(feature = "_tls-any")]
mod tls;

#[doc(inline)]
#[cfg(feature = "channel")]
pub use self::channel::{Channel, Endpoint};
pub use self::error::Error;
#[doc(inline)]
#[cfg(feature = "server")]
pub use self::server::Server;

#[cfg(feature = "_tls-any")]
pub use self::tls::Certificate;
pub use hyper::{body::Body, Uri};
#[cfg(feature = "_tls-any")]
pub use tokio_rustls::rustls::pki_types::CertificateDer;

#[cfg(all(feature = "channel", feature = "_tls-any"))]
pub use self::channel::ClientTlsConfig;
#[cfg(all(feature = "server", feature = "_tls-any"))]
pub use self::server::ServerTlsConfig;
#[cfg(feature = "_tls-any")]
pub use self::tls::Identity;
