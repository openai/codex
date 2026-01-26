//! MCP protocol types.

use std::fmt;

use serde::Deserialize;
use serde::Serialize;

/// Authentication status for an MCP server connection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum McpAuthStatus {
    /// OAuth authentication is not supported by this server.
    #[default]
    Unsupported,
    /// OAuth is supported but user is not logged in.
    NotLoggedIn,
    /// Using a bearer token for authentication.
    BearerToken,
    /// Using OAuth for authentication.
    OAuth,
}

impl fmt::Display for McpAuthStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unsupported => f.write_str("Unsupported"),
            Self::NotLoggedIn => f.write_str("Not logged in"),
            Self::BearerToken => f.write_str("Bearer token"),
            Self::OAuth => f.write_str("OAuth"),
        }
    }
}
