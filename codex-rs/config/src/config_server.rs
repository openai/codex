//! Trusted enrollment settings for a customer-hosted configuration server.
//!

use serde::Deserialize;
use serde::Serialize;
use std::fmt;

pub use codex_model_provider_info::OAuthSessionConfig as ConfigServerOAuth;

// Deployment-specific prefixes belong in base_url.
const BUNDLE_PATH: &str = "codex/config/bundle";

#[derive(Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(try_from = "ConfigServerInput")]
pub struct ConfigServer {
    pub base_url: String,
    // Boxing keeps config-loader futures small.
    pub oauth: Option<Box<ConfigServerOAuth>>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfigServerInput {
    base_url: String,
    oauth: Option<Box<ConfigServerOAuth>>,
}

impl TryFrom<ConfigServerInput> for ConfigServer {
    type Error = String;

    fn try_from(input: ConfigServerInput) -> Result<Self, Self::Error> {
        let service = Self {
            base_url: input.base_url,
            oauth: input.oauth,
        };
        service.endpoint_url()?;
        Ok(service)
    }
}

impl ConfigServer {
    /// Validates the trusted settings and appends the fixed bundle route to the base URL.
    pub fn endpoint_url(&self) -> Result<String, String> {
        let validate_https_url = |value: &str| {
            codex_http_client::validate_endpoint_url(value, "config_server")
                .ok()
                .filter(|url| url.scheme() == "https")
                .ok_or("config_server URLs must use HTTPS without userinfo or fragments")
        };
        let mut endpoint = validate_https_url(&self.base_url)?;
        if endpoint.query().is_some() {
            return Err("config_server.base_url must not contain a query".into());
        }
        endpoint
            .path_segments_mut()
            .map_err(|_| "config_server.base_url cannot be used as a base URL")?
            .pop_if_empty()
            .extend(BUNDLE_PATH.split('/'));
        if let Some(oauth) = &self.oauth {
            validate_https_url(&oauth.authorization_url)?;
            validate_https_url(&oauth.token_url)?;
            if let Some(url) = &oauth.device_authorization_url {
                validate_https_url(url)?;
            }
            if oauth.client_id.trim().is_empty() || oauth.redirect_port == Some(0) {
                return Err("config_server OAuth requires a client ID and a nonzero redirect port when specified".into());
            }
        }
        Ok(endpoint.into())
    }
}

impl fmt::Debug for ConfigServer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ConfigServer").finish_non_exhaustive()
    }
}
