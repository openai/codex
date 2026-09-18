//! Secondary OAuth credentials delivered alongside the provider's primary authentication.

use std::fmt;

use http::header::HeaderName;
use schemars::JsonSchema;
use serde::Deserialize;
use serde::Serialize;
use url::Host;
use url::Url;

use crate::ModelProviderInfo;

// Reserve authentication, routing/framing, and internal protocol headers.
const RESERVED_GATEWAY_OAUTH_HEADERS: &[&str] = &[
    "authorization",
    "proxy-authorization",
    "cookie",
    "host",
    "content-length",
    "transfer-encoding",
    "connection",
    "upgrade",
    "chatgpt-account-id",
];
const RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES: &[&str] =
    &["x-codex-", "x-openai-", "sec-websocket-"];

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GatewayOAuthConfig {
    pub authorization_url: String,
    pub token_url: String,
    pub client_id: String,
    pub resource: Option<String>,
    #[serde(default)]
    pub scopes: Vec<String>,
    pub redirect_port: Option<u16>,
    pub delivery: GatewayOAuthDelivery,
}

impl fmt::Debug for GatewayOAuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayOAuthConfig")
            .field("delivery", &self.delivery)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum GatewayOAuthDelivery {
    Header {
        name: String,
        #[serde(default = "bearer_scheme")]
        scheme: String,
    },
    Cookie {
        name: String,
    },
}

fn bearer_scheme() -> String {
    "Bearer".to_owned()
}

impl GatewayOAuthConfig {
    pub(crate) fn validate(&self, provider: &ModelProviderInfo) -> Result<(), String> {
        if provider.aws.is_some() || provider.is_amazon_bedrock() {
            return Err("provider gateway_oauth cannot be combined with AWS authentication".into());
        }
        validate_url(&self.authorization_url, "gateway_oauth.authorization_url")?;
        validate_url(&self.token_url, "gateway_oauth.token_url")?;
        let base_url = provider
            .base_url
            .as_deref()
            .ok_or("gateway_oauth requires base_url")?;
        validate_url(base_url, "base_url")?;
        if self.client_id.trim().is_empty() || self.redirect_port == Some(0) {
            return Err(
                "gateway_oauth requires a nonempty client_id and a nonzero redirect_port".into(),
            );
        }
        let header = match &self.delivery {
            GatewayOAuthDelivery::Header { name, scheme } => {
                let header = HeaderName::from_bytes(name.as_bytes())
                    .map_err(|_| "invalid gateway_oauth header name")?;
                if !is_token(scheme)
                    || RESERVED_GATEWAY_OAUTH_HEADERS.contains(&header.as_str())
                    || RESERVED_GATEWAY_OAUTH_HEADER_PREFIXES
                        .iter()
                        .any(|prefix| header.as_str().starts_with(prefix))
                {
                    return Err(
                        "invalid or reserved gateway_oauth delivery header or scheme".into(),
                    );
                }
                header
            }
            GatewayOAuthDelivery::Cookie { name } => {
                if !is_token(name) {
                    return Err("invalid gateway_oauth cookie name".into());
                }
                http::header::COOKIE
            }
        };
        if provider
            .http_headers
            .iter()
            .flat_map(|headers| headers.keys())
            .chain(
                provider
                    .env_http_headers
                    .iter()
                    .flat_map(|headers| headers.keys()),
            )
            .any(|name| name.eq_ignore_ascii_case(header.as_str()))
        {
            return Err(
                "gateway_oauth delivery conflicts with a configured provider header".into(),
            );
        }
        Ok(())
    }
}

fn is_token(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&byte))
}

fn validate_url(value: &str, field: &str) -> Result<(), String> {
    let url = Url::parse(value).map_err(|_| format!("invalid {field} URL"))?;
    let loopback = match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(host)) => host.is_loopback(),
        Some(Host::Ipv6(host)) => host.is_loopback(),
        None => false,
    };
    if url.host().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(url.scheme() == "https" || (url.scheme() == "http" && loopback))
    {
        return Err(format!(
            "{field} must use HTTPS (or loopback HTTP), without userinfo or a fragment"
        ));
    }
    Ok(())
}

#[cfg(test)]
#[path = "gateway_oauth_tests.rs"]
mod tests;
