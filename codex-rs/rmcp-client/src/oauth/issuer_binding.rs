use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationMetadata;
use url::Url;

use super::StoredOAuthTokens;

/// Reject authorization endpoints that cannot be bound to their actual issuer.
pub(crate) fn validate_authorization_server_endpoints(
    metadata: &AuthorizationMetadata,
) -> Result<()> {
    let authorization_endpoint = Url::parse(&metadata.authorization_endpoint)
        .context("OAuth authorization endpoint must be a valid URL")?;
    let issuer = metadata
        .issuer
        .as_deref()
        .filter(|issuer| !issuer.trim().is_empty())
        .map(Url::parse)
        .transpose()
        .context("OAuth authorization server issuer must be a valid URL")?;
    let issuer_bound_callbacks = metadata
        .additional_fields
        .get("authorization_response_iss_parameter_supported")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);

    if issuer_bound_callbacks {
        if issuer.is_none() {
            bail!("OAuth issuer-bound callbacks require an authorization server issuer");
        }
        return Ok(());
    }

    let token_endpoint =
        Url::parse(&metadata.token_endpoint).context("OAuth token endpoint must be a valid URL")?;

    if let Some(issuer) = issuer {
        if authorization_endpoint.origin() == issuer.origin()
            || authorization_endpoint.origin() == token_endpoint.origin()
            // Remove these narrow compatibility exceptions once both providers support RFC 9207.
            || matches!(
                (
                    issuer.as_str(),
                    authorization_endpoint.origin().ascii_serialization().as_str(),
                    token_endpoint.origin().ascii_serialization().as_str(),
                ),
                (
                    "https://api.figma.com/",
                    "https://www.figma.com",
                    "https://api.figma.com",
                ) | (
                    "https://agent.robinhood.com/mcp/trading",
                    "https://robinhood.com",
                    "https://api.robinhood.com",
                )
            )
        {
            return Ok(());
        }
        bail!(
            "OAuth authorization endpoint origin does not match the authorization server origin without issuer-bound callbacks"
        );
    }

    if token_endpoint.origin() != authorization_endpoint.origin() {
        bail!(
            "OAuth token endpoint origin does not match the authorization server origin without issuer-bound callbacks"
        );
    }

    Ok(())
}

/// Verifies that a stored refresh token remains bound to its original issuer.
///
/// Call this with the same metadata snapshot that RMCP will use for the credentials. Missing or
/// changed issuers require a new login rather than risking sending a refresh token to a different
/// authorization server.
pub(crate) fn validate_refresh_token_issuer(
    metadata: &AuthorizationMetadata,
    tokens: &StoredOAuthTokens,
) -> Result<()> {
    if !tokens.has_refresh_token() {
        return Ok(());
    }

    let Some(stored_issuer) = tokens.bound_issuer() else {
        return Err(AuthError::AuthorizationRequired).with_context(|| {
            format!(
                "OAuth refresh credentials for server {} are missing an authorization server issuer; authorization required",
                tokens.server_name
            )
        });
    };

    let Some(current_issuer) = metadata
        .issuer
        .as_deref()
        .filter(|issuer| !issuer.trim().is_empty())
    else {
        return Err(AuthError::AuthorizationRequired).with_context(|| {
            format!(
                "OAuth metadata for server {} did not include an authorization server issuer; authorization required",
                tokens.server_name
            )
        });
    };

    if current_issuer != stored_issuer {
        return Err(AuthError::AuthorizationRequired).with_context(|| {
            format!(
                "OAuth authorization server issuer changed for server {}; authorization required",
                tokens.server_name
            )
        });
    }

    Ok(())
}
