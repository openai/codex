//! All bits and pieces concerning the HTTP mock server are in this module.
//!
//! `bare_server::BareMockServer` is the "front-end" to drive behaviour for the `hyper` HTTP
//! server running in the background, defined in the `hyper` sub-module.
//!
//! `bare_server::BareMockServer` is not exposed directly: crate users only get to interact with
//! `exposed_server::MockServer`.
//! `exposed_server::MockServer` is either a wrapper around a `BareMockServer` retrieved from an
//! object pool or a wrapper around an exclusive `BareMockServer`.
//! We use the pool when the user does not care about the port the mock server listens to, while
//! we provision a dedicated one if they specify their own `TcpListener` with `start_on`.
//! Check the `pool` submodule for more details on our pooling strategy.
mod bare_server;
mod builder;
mod exposed_server;
mod hyper;
mod pool;

pub use bare_server::MockGuard;
pub use builder::MockServerBuilder;
pub use exposed_server::MockServer;
