//! Credential-destination policy for enterprise MCP OAuth.

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_config::McpEmaTokenEndpointAuthMethod;
use serde_json::Value;
use url::Host;
use url::Url;

/// A sanitized enterprise-auth failure that callers may handle without parsing text.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum EmaAuthFailure {
    #[error("invalid_grant")]
    InvalidGrant { grant_source: EmaInvalidGrantSource },
    #[error("insufficient_user_authentication")]
    InsufficientUserAuthentication { grant_source: EmaInvalidGrantSource },
    #[error("enterprise identity rejected the OAuth client")]
    IdentityClientRejected,
    #[error("enterprise identity requires authentication")]
    ReauthenticationRequired,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmaInvalidGrantSource {
    EnterpriseIdentity,
    ResourceAuthorization,
}

/// The credential source selected for enterprise token acquisition or validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmaCredentialKind {
    Account,
    Keyring,
}

/// Carries only a credential kind and an already sanitized auth failure to transport.
/// Unexpected errors deliberately retain neither provider text nor their source chain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EmaTokenSourceError {
    AuthFailure {
        credential: EmaCredentialKind,
        failure: EmaAuthFailure,
    },
    Unexpected {
        credential: EmaCredentialKind,
    },
}

impl EmaTokenSourceError {
    pub fn new(credential: EmaCredentialKind, failure: EmaAuthFailure) -> Self {
        Self::AuthFailure {
            credential,
            failure,
        }
    }

    pub fn unexpected(credential: EmaCredentialKind) -> Self {
        Self::Unexpected { credential }
    }

    fn credential(self) -> EmaCredentialKind {
        match self {
            Self::AuthFailure { credential, .. } | Self::Unexpected { credential } => credential,
        }
    }

    fn failure(self) -> Option<EmaAuthFailure> {
        match self {
            Self::AuthFailure { failure, .. } => Some(failure),
            Self::Unexpected { .. } => None,
        }
    }

    pub fn is_authentication_required(&self) -> bool {
        match (self.credential(), self.failure()) {
            (
                _,
                Some(
                    EmaAuthFailure::InvalidGrant {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::InsufficientUserAuthentication {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::ReauthenticationRequired,
                ),
            )
            | (EmaCredentialKind::Account, Some(EmaAuthFailure::IdentityClientRejected)) => true,
            (
                _,
                Some(
                    EmaAuthFailure::InvalidGrant {
                        grant_source: EmaInvalidGrantSource::ResourceAuthorization,
                    }
                    | EmaAuthFailure::InsufficientUserAuthentication {
                        grant_source: EmaInvalidGrantSource::ResourceAuthorization,
                    },
                )
                | None,
            )
            | (EmaCredentialKind::Keyring, Some(EmaAuthFailure::IdentityClientRejected)) => false,
        }
    }
}

impl std::fmt::Display for EmaTokenSourceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match (self.credential(), self.failure()) {
            (
                _,
                Some(EmaAuthFailure::InsufficientUserAuthentication {
                    grant_source: EmaInvalidGrantSource::ResourceAuthorization,
                }),
            ) => {
                "The MCP authorization server requires additional user authentication; contact your administrator"
            }
            (EmaCredentialKind::Keyring, Some(EmaAuthFailure::IdentityClientRejected)) => {
                "The enterprise IdP rejected the configured MCP OAuth client; contact your administrator"
            }
            (
                EmaCredentialKind::Account,
                Some(
                    EmaAuthFailure::InvalidGrant {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::InsufficientUserAuthentication {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::IdentityClientRejected,
                ),
            ) => "Complete enterprise sign-in for this MCP server",
            (EmaCredentialKind::Account, Some(EmaAuthFailure::ReauthenticationRequired)) => {
                "Sign in to your Codex account to restore enterprise MCP access"
            }
            (
                EmaCredentialKind::Keyring,
                Some(
                    EmaAuthFailure::InvalidGrant {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::InsufficientUserAuthentication {
                        grant_source: EmaInvalidGrantSource::EnterpriseIdentity,
                    }
                    | EmaAuthFailure::ReauthenticationRequired,
                ),
            ) => "Complete enterprise sign-in to restore MCP access",
            (
                _,
                Some(EmaAuthFailure::InvalidGrant {
                    grant_source: EmaInvalidGrantSource::ResourceAuthorization,
                }),
            ) => "The MCP authorization server rejected the enterprise grant: invalid_grant",
            (_, None) => "enterprise MCP token exchange failed",
        };
        f.write_str(message)
    }
}

impl std::error::Error for EmaTokenSourceError {}

pub(crate) fn ema_reauthentication_required(message: &'static str) -> anyhow::Error {
    anyhow::Error::new(EmaAuthFailure::ReauthenticationRequired).context(message)
}

pub(crate) fn resolve_ema_client_auth(
    advertised_methods: Option<&Value>,
    client_secret: Option<&str>,
    token_endpoint_auth_method: Option<McpEmaTokenEndpointAuthMethod>,
    issuer_description: &str,
) -> Result<rmcp::transport::auth::enterprise::EmaClientAuthentication> {
    use oauth2::ClientSecret;
    use rmcp::transport::auth::enterprise::EmaClientAuthentication;

    if let Some(secret) = client_secret {
        if secret.trim().is_empty() {
            bail!("{issuer_description} client secret must not be empty");
        }
        let registered_method = token_endpoint_auth_method.unwrap_or_default();
        // RFC 8414 defaults absent server capabilities to Basic. Capabilities
        // validate the registration; they do not select its authentication method.
        let supported = match advertised_methods {
            None => registered_method == McpEmaTokenEndpointAuthMethod::ClientSecretBasic,
            Some(methods) => methods.as_array().is_some_and(|methods| {
                methods
                    .iter()
                    .any(|method| method.as_str() == Some(registered_method.as_str()))
            }),
        };
        if !supported {
            bail!(
                "{issuer_description} does not support the registered token endpoint authentication method"
            );
        }
        let secret = ClientSecret::new(secret.to_owned());
        return Ok(match registered_method {
            McpEmaTokenEndpointAuthMethod::ClientSecretBasic => {
                EmaClientAuthentication::ClientSecretBasic(secret)
            }
            McpEmaTokenEndpointAuthMethod::ClientSecretPost => {
                EmaClientAuthentication::ClientSecretPost(secret)
            }
        });
    }
    if token_endpoint_auth_method.is_some() {
        bail!("{issuer_description} token endpoint authentication method requires a client secret");
    }
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
    Ok(EmaClientAuthentication::None)
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
