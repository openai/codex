//! HTTP client capability implementations shared by local and remote environments.
//!
//! This module is the facade for the environment-owned [`crate::HttpClient`]
//! capability:
//! - [`LocalHttpClient`] executes requests directly with `reqwest`
//! - [`ExecServerClient`] forwards requests over the remote JSON-RPC transport
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
#[path = "local_http_client.rs"]
mod local_http_client;
#[path = "remote_http_client.rs"]
mod remote_http_client;

pub use body_stream::HttpResponseBodyStream;
pub(crate) use local_http_client::HttpRequestRunner;
pub(crate) use local_http_client::LocalHttpClient;
pub(crate) use local_http_client::PendingHttpBodyStream;
