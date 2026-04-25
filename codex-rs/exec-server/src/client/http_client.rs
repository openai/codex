//! HTTP client capability implementations shared by local and remote environments.
//!
//! This module is the facade for the environment-owned [`crate::HttpClient`]
//! capability:
//! - [`ReqwestHttpClient`] executes requests directly with `reqwest`
//! - [`ExecServerClient`] forwards requests over the JSON-RPC transport
//! - [`HttpResponseBodyStream`] presents buffered local bodies and streamed
//!   remote `http/request/bodyDelta` notifications through one byte-stream API
//!
//! Runtime split:
//! - orchestrator process: holds an `Arc<dyn HttpClient>` and chooses local or
//!   remote execution
//! - remote runtime: serves the `http/request` RPC and runs the concrete local
//!   HTTP request there when the orchestrator uses [`ExecServerClient`]

#[path = "reqwest_http_client.rs"]
mod reqwest_http_client;
#[path = "http_response_body_stream.rs"]
pub(crate) mod response_body_stream;
#[path = "rpc_http_client.rs"]
mod rpc_http_client;

pub(crate) use reqwest_http_client::PendingReqwestHttpBodyStream;
pub use reqwest_http_client::ReqwestHttpClient;
pub(crate) use reqwest_http_client::ReqwestHttpRequestRunner;
pub use response_body_stream::HttpResponseBodyStream;
