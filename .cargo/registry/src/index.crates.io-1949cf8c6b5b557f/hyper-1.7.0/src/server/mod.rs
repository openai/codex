//! HTTP Server
//!
//! A "server" is usually created by listening on a port for new connections,
//! parse HTTP requests, and hand them off to a `Service`.
//!
//! How exactly you choose to listen for connections is not something hyper
//! concerns itself with. After you have a connection, you can handle HTTP over
//! it with the types in the [`conn`] module.
pub mod conn;
