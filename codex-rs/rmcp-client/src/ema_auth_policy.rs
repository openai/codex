//! Credential-destination policy for enterprise MCP OAuth.

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use serde_json::Value;
use url::Host;
use url::Url;

/// A sanitized enterprise-auth failure that callers may handle without parsing text.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmaAuthFailure {
    #[error("invalid_grant")]
    InvalidGrant { grant_source: EmaInvalidGrantSource },
    #[error("insufficient_user_authentication")]
    InsufficientUserAuthentication,
    #[error("enterprise identity requires authentication")]
    ReauthenticationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmaInvalidGrantSource {
    EnterpriseIdentity,
    ResourceAuthorization,
}

pub(crate) fn ema_reauthentication_required(message: &'static str) -> anyhow::Error {
    anyhow::Error::new(EmaAuthFailure::ReauthenticationRequired).context(message)
}

pub(crate) fn validate_ema_public_client_auth(
    advertised_methods: Option<&Value>,
    issuer_description: &str,
) -> Result<()> {
    let advertised_methods = advertised_methods.ok_or_else(|| {
        anyhow!(
            "{issuer_description} does not explicitly advertise public-client token endpoint authentication"
        )
    })?;
    let methods = advertised_methods.as_array().ok_or_else(|| {
        anyhow!("{issuer_description} advertised malformed token endpoint authentication methods")
    })?;
    if !methods.iter().any(|method| method.as_str() == Some("none")) {
        bail!("{issuer_description} does not support public-client token endpoint authentication");
    }
    Ok(())
}

pub(crate) fn validate_ema_oauth_endpoint(endpoint: &str, description: &str) -> Result<()> {
    let url = Url::parse(endpoint).with_context(|| format!("{description} is not a valid URL"))?;
    validate_credential_destination(&url, description)
}

fn validate_credential_destination(url: &Url, description: &str) -> Result<()> {
    let loopback = match url.host() {
        Some(Host::Domain(host)) => host.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        bail!("{description} must use HTTPS or an HTTP loopback address");
    }
    if !url.username().is_empty() || url.password().is_some() || url.fragment().is_some() {
        bail!("{description} contains disallowed credentials or a URL fragment");
    }
    Ok(())
}

pub(crate) fn advertised_capability(
    value: Option<&Value>,
    expected: &str,
    description: &str,
) -> Result<Option<bool>> {
    value
        .map(|value| {
            let values = value
                .as_array()
                .ok_or_else(|| anyhow!("{description} is malformed"))?;
            Ok(values.iter().any(|value| value.as_str() == Some(expected)))
        })
        .transpose()
}

/// Resource indicators must describe the configured MCP origin, query and path.
pub fn validate_ema_auth_resource(server_url: &str, resource: Option<&str>) -> Result<()> {
    let server = Url::parse(server_url).context("enterprise MCP server URL is invalid")?;
    validate_credential_destination(&server, "enterprise MCP server URL")?;
    let Some(resource) = resource.filter(|resource| !resource.trim().is_empty()) else {
        return Ok(());
    };
    let resource = Url::parse(resource).context("enterprise MCP resource indicator is invalid")?;
    validate_credential_destination(&resource, "enterprise MCP resource indicator")?;
    if resource.origin() != server.origin() || resource.query() != server.query() {
        bail!(
            "enterprise MCP resource indicator must match the configured MCP server origin and query"
        );
    }
    let resource_path = resource.path().trim_end_matches('/');
    let server_path = server.path().trim_end_matches('/');
    if server_path != resource_path
        && !server_path
            .strip_prefix(resource_path)
            .is_some_and(|suffix| suffix.starts_with('/'))
    {
        bail!("enterprise MCP resource indicator path must contain the configured MCP server path");
    }
    Ok(())
}

#[cfg(test)]
#[path = "ema_auth_policy_tests.rs"]
mod tests;
