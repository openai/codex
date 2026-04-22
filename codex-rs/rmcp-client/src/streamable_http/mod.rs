//! Streamable HTTP transport pieces used by `RmcpClient`.
//!
//! The orchestrator-side RMCP adapter lives here and is layered on top of the
//! shared `HttpClient` capability exposed by `codex-exec-server`.

pub(crate) mod common;
pub(crate) mod http_client_adapter;
