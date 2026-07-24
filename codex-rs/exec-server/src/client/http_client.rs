//! HTTP client capability implementations shared by local and remote environments.
//!
//! This module is the facade for the environment-owned [`crate::HttpClient`]
//! capability:
//! - [`RouteAwareHttpClient`] executes requests through the shared transport
//! - [`ExecServerClient`] forwards requests over the JSON-RPC transport
//! - [`HttpResponseBodyStream`] presents buffered local bodies and streamed
//!   remote `http/request/bodyDelta` notifications through one byte-stream API
//!
//! Runtime split:
//! - orchestrator process: holds an `Arc<dyn HttpClient>` and chooses local or
//!   remote execution
//! - remote runtime: serves the `http/request` RPC and runs the concrete local
//!   HTTP request there when the orchestrator uses [`ExecServerClient`]

#[path = "http_response_body_stream.rs"]
pub(crate) mod response_body_stream;
#[path = "route_aware_http_client.rs"]
mod route_aware_http_client;
#[path = "rpc_http_client.rs"]
mod rpc_http_client;

pub use response_body_stream::HttpResponseBodyStream;
pub(crate) use route_aware_http_client::PendingRouteAwareHttpBodyStream;
pub use route_aware_http_client::RouteAwareHttpClient;
pub(crate) use route_aware_http_client::RouteAwareHttpRequestRunner;
