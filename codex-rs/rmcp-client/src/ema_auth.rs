//! Select the resource authorization server before polling identity or exchanging credentials.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_config::McpEmaTokenEndpointAuthMethod;
use codex_exec_server::HttpClient;
use futures::future::BoxFuture;
use http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::EmaCredentialLease;
use crate::EmaErrorDetail;
use crate::ema_auth_policy::advertised_capability;
use crate::ema_auth_policy::resolve_ema_client_auth;
use crate::ema_auth_policy::validate_ema_auth_resource;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::ema_exchange::EmaAccessToken;
use crate::ema_exchange::EmaIdJagExchangeRequest;
use crate::ema_exchange::JWT_BEARER_GRANT_TYPE;
use crate::ema_exchange::exchange_id_jag;
use crate::ema_identity::EmaIdpIdentity;
use crate::ema_resource::EmaDiscoveryRequest;
use crate::ema_resource::EmaResource;
use crate::ema_resource::discover_ema_resource;
use crate::ema_resource::validate_ema_metadata_issuer;
use rmcp::transport::auth::enterprise::EmaClientAuthentication;

const ID_JAG_GRANT_PROFILE: &str = "urn:ietf:params:oauth:grant-profile:id-jag";

/// Token-only Resource AS metadata must not require an unused authorization endpoint.
#[derive(Deserialize)]
struct EmaAuthorizationMetadata {
    issuer: Option<String>,
    token_endpoint: String,
    #[serde(flatten)]
    additional_fields: HashMap<String, Value>,
}

/// Inputs for non-interactive enterprise-managed MCP authorization.
pub struct EmaAuthTokenExchangeRequest<'a, L = EmaCredentialLease> {
    pub server_url: &'a str,
    pub resource: Option<&'a str>,
    pub scopes: &'a [String],
    pub mcp_client_id: Option<&'a str>,
    pub mcp_client_secret: Option<&'a str>,
    pub token_endpoint_auth_method: Option<McpEmaTokenEndpointAuthMethod>,
    pub expected_authorization_server_issuer: Option<&'a str>,
    pub idp_issuer: &'a str,
    pub idp_client_id: &'a str,
    pub idp_identity: BoxFuture<'a, Result<EmaIdpIdentity<L>>>,
    pub idp_http_client: Arc<dyn HttpClient>,
    pub resource_http_client: Arc<dyn HttpClient>,
}

impl<'a, L> From<&EmaAuthTokenExchangeRequest<'a, L>> for EmaDiscoveryRequest<'a> {
    fn from(request: &EmaAuthTokenExchangeRequest<'a, L>) -> Self {
        Self {
            server_url: request.server_url,
            resource: request.resource,
            expected_issuer: request.expected_authorization_server_issuer,
            http_client: Arc::clone(&request.resource_http_client),
        }
    }
}

/// Discover the resource's authorization server, then perform the ID-JAG flow.
pub async fn exchange_ema_auth_token<L: Send, R: Send>(
    request: EmaAuthTokenExchangeRequest<'_, L>,
    finish_idp: impl FnOnce(L) -> R + Send,
) -> Result<(EmaAccessToken, R)> {
    validate_ema_auth_resource(request.server_url, request.resource)?;
    let client_id = request
        .mcp_client_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "ema_auth requires the OAuth client ID registered with the MCP authorization server"
            )
        })?;
    if request.mcp_client_secret.is_some()
        && request
            .expected_authorization_server_issuer
            .is_none_or(|issuer| issuer.trim().is_empty())
    {
        bail!("confidential ema_auth requires a configured authorization server issuer");
    }
    let (metadata, resource, resource_client_authentication) = discover_ema_authorization_metadata(
        EmaDiscoveryRequest::from(&request),
        request.mcp_client_secret,
        request.token_endpoint_auth_method,
    )
    .await
    .context(EmaErrorDetail::new(
        "enterprise MCP authorization server discovery failed",
    ))?;
    let issuer = metadata
        .issuer
        .as_deref()
        .ok_or_else(|| anyhow!("MCP authorization metadata is missing an issuer"))?;
    let EmaIdpIdentity {
        token_endpoint,
        refresh_token,
        credentials,
        client_authentication: idp_client_authentication,
    } = request.idp_identity.await?;
    exchange_id_jag(
        EmaIdJagExchangeRequest {
            resource: &resource,
            scopes: request.scopes,
            mcp_client_id: client_id,
            authorization_server_issuer: issuer,
            authorization_server_token_endpoint: &metadata.token_endpoint,
            idp_token_endpoint: &token_endpoint,
            idp_issuer: request.idp_issuer,
            idp_client_id: request.idp_client_id,
            idp_client_authentication,
            resource_client_authentication,
            refresh_token,
            idp_http_client: request.idp_http_client,
            resource_http_client: request.resource_http_client,
        },
        credentials,
        finish_idp,
    )
    .await
}

async fn discover_ema_authorization_metadata(
    request: EmaDiscoveryRequest<'_>,
    client_secret: Option<&str>,
    token_endpoint_auth_method: Option<McpEmaTokenEndpointAuthMethod>,
) -> Result<(EmaAuthorizationMetadata, String, EmaClientAuthentication)> {
    let expected_issuer = request.expected_issuer;
    let EmaResource {
        server_url: server,
        resource,
        authorization_servers: issuers,
        client,
    } = discover_ema_resource(request).await?;
    for issuer in issuers {
        let issuer_url = match Url::parse(&issuer) {
            Ok(url) => url,
            Err(error) if expected_issuer.is_some() => {
                return Err(error)
                    .context("configured enterprise authorization server issuer is invalid");
            }
            Err(_) => continue,
        };
        if let Err(error) = validate_ema_metadata_issuer(&server, &issuer_url) {
            if expected_issuer.is_some() {
                return Err(error);
            }
            continue;
        }
        let path = issuer_url.path().trim_matches('/');
        let suffix = if path.is_empty() {
            String::new()
        } else {
            format!("/{path}")
        };
        let mut urls = vec![
            issuer_url.join(&format!("/.well-known/oauth-authorization-server{suffix}"))?,
            issuer_url.join(&format!("/.well-known/openid-configuration{suffix}"))?,
        ];
        if !path.is_empty() {
            urls.push(issuer_url.join(&format!("/{path}/.well-known/openid-configuration"))?);
            urls.push(issuer_url.join("/.well-known/oauth-authorization-server")?);
        }
        for url in urls {
            let response = client.get(url).await?;
            if response.status() != StatusCode::OK {
                continue;
            }
            let Ok(metadata) = serde_json::from_slice::<EmaAuthorizationMetadata>(response.body())
            else {
                continue;
            };
            if metadata.issuer.as_deref() != Some(issuer.as_str()) {
                bail!(EmaErrorDetail::new(
                    "enterprise authorization metadata issuer does not match its advertised issuer"
                ));
            }
            validate_ema_oauth_endpoint(
                &metadata.token_endpoint,
                "MCP authorization server token endpoint",
            )?;
            if expected_issuer.is_some()
                && issuer_url.origin() != Url::parse(&metadata.token_endpoint)?.origin()
            {
                bail!("enterprise MCP authorization server token endpoint changed origin");
            }
            let profile = advertised_capability(
                metadata
                    .additional_fields
                    .get("authorization_grant_profiles_supported"),
                ID_JAG_GRANT_PROFILE,
                "MCP authorization grant profiles",
            )?;
            let grant = advertised_capability(
                metadata.additional_fields.get("grant_types_supported"),
                JWT_BEARER_GRANT_TYPE,
                "MCP authorization grant types",
            )?;
            if profile == Some(false) || grant != Some(true) {
                break;
            }
            match resolve_ema_client_auth(
                metadata
                    .additional_fields
                    .get("token_endpoint_auth_methods_supported"),
                client_secret,
                token_endpoint_auth_method,
                "MCP authorization server",
            ) {
                Ok(authentication) => return Ok((metadata, resource, authentication)),
                Err(error) if expected_issuer.is_some() => return Err(error),
                Err(_) => break,
            }
        }
    }
    bail!(
        "enterprise MCP discovery found no authorization server supporting the configured client authentication and ID-JAG JWT bearer grant"
    )
}

#[cfg(test)]
#[path = "ema_auth_tests.rs"]
mod tests;
