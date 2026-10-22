//! Public-client OAuth settings shared by managed configuration and service sign-in.

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

/// Settings identifying a configured OAuth grant, independent of token delivery.
#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthSessionConfig {
    pub authorization_url: String,
    pub token_url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub device_authorization_url: Option<String>,
    pub client_id: String,
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_port: Option<u16>,
}

impl fmt::Debug for OAuthSessionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Issuer settings may contain credentials in arbitrary query parameters.
        f.debug_struct("OAuthSessionConfig").finish_non_exhaustive()
    }
}
