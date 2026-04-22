//! HTTP client capability implementations shared by local and remote environments.
//!
//! This module is the facade for the environment-owned [`crate::HttpClient`]
//! capability:
//! - [`DirectHttpClient`] executes requests directly with `reqwest`
//! - [`ExecServerClient`] forwards requests over the JSON-RPC transport
//! - [`HttpResponseBodyStream`] presents buffered local bodies and streamed
//!   remote `http/request/bodyDelta` notifications through one byte-stream API
//!
//! Runtime split:
//! - orchestrator process: holds an `Arc<dyn HttpClient>` and chooses local or
//!   remote execution
//! - remote runtime: serves the `http/request` RPC and runs the concrete local
//!   HTTP request there when the orchestrator uses [`ExecServerClient`]

#[path = "http_body_stream.rs"]
pub(crate) mod body_stream;
#[path = "direct_http_client.rs"]
mod direct_http_client;
#[path = "rpc_http_client.rs"]
mod rpc_http_client;

pub use body_stream::HttpResponseBodyStream;
pub(crate) use direct_http_client::DirectHttpClient;
pub(crate) use direct_http_client::DirectHttpRequestRunner;
pub(crate) use direct_http_client::PendingDirectHttpBodyStream;
