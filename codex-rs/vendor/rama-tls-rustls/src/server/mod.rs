//! TLS server support for Rama.
//!
//! This module provides a [`TlsAcceptorLayer`] to accept TLS connections and a [`TlsAcceptorService`] to handle them.
//!
//! # Examples
//!
//! See the [Examples Directory](https://github.com/plabayo/rama/tree/main/examples):
//!
//! - [/examples/tls_rustls_termination.rs](https://github.com/plabayo/rama/tree/main/examples/tls_rustls_termination.rs):
//!   Spawns a mini handmade http server, as well as a TLS termination proxy, forwarding the
//!   plain text stream to the first.
//! - [/examples/mtls_tunnel_and_services.rs](https://github.com/plabayo/rama/tree/main/examples/mtls_tunnel_and_services.rs):
//!   Example of how to do mTls (manual Tls, where the client also needs a certificate) using rama,
//!   as well as how one might use this concept to provide a tunnel service build with these concepts;

mod service;
#[doc(inline)]
pub use service::TlsAcceptorService;

mod layer;
#[doc(inline)]
pub use layer::TlsAcceptorLayer;

mod acceptor_data;
#[doc(inline)]
pub use acceptor_data::{
    DynamicConfigProvider, TlsAcceptorData, TlsAcceptorDataBuilder, self_signed_server_auth,
};
