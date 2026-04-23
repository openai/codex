#![cfg_attr(docsrs, feature(doc_cfg))]
#![cfg_attr(docsrs, allow(unused_attributes))]
#![doc = include_str!("../README.md")]

mod error;
#[allow(deprecated)]
pub use error::{Error, ErrorData, RmcpError};

/// Basic data types in MCP specification
pub mod model;
#[cfg(any(feature = "client", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "client", feature = "server"))))]
pub mod service;
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use handler::client::ClientHandler;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use handler::server::ServerHandler;
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use handler::server::wrapper::Json;
#[cfg(any(feature = "client", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "client", feature = "server"))))]
pub use service::{Peer, Service, ServiceError, ServiceExt};
#[cfg(feature = "client")]
#[cfg_attr(docsrs, doc(cfg(feature = "client")))]
pub use service::{RoleClient, serve_client};
#[cfg(feature = "server")]
#[cfg_attr(docsrs, doc(cfg(feature = "server")))]
pub use service::{RoleServer, serve_server};

pub mod handler;
pub mod task_manager;
pub mod transport;

// re-export
#[cfg(all(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "macros", feature = "server"))))]
pub use pastey::paste;
#[cfg(all(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(all(feature = "macros", feature = "server"))))]
pub use rmcp_macros::*;
#[cfg(any(feature = "macros", feature = "server"))]
#[cfg_attr(docsrs, doc(cfg(any(feature = "macros", feature = "server"))))]
pub use schemars;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use serde;
#[cfg(feature = "macros")]
#[cfg_attr(docsrs, doc(cfg(feature = "macros")))]
pub use serde_json;
