use anyhow::Context;
use anyhow::Result;
use rmcp::transport::auth::AuthError;
use rmcp::transport::auth::AuthorizationMetadata;

use super::StoredOAuthTokens;
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
