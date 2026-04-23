#[cfg(feature = "transport-streamable-http-server")]
pub mod server_side_http;

pub mod http_header;

#[cfg(feature = "__reqwest")]
#[cfg_attr(docsrs, doc(cfg(feature = "reqwest")))]
mod reqwest;

// Note: This module provides SSE stream parsing and auto-reconnect utilities.
// It's used by the streamable HTTP client (which receives SSE-formatted responses),
// not the removed SSE transport. The name is historical.
#[cfg(feature = "client-side-sse")]
#[cfg_attr(docsrs, doc(cfg(feature = "client-side-sse")))]
pub mod client_side_sse;

#[cfg(feature = "auth")]
#[cfg_attr(docsrs, doc(cfg(feature = "auth")))]
pub mod auth;
