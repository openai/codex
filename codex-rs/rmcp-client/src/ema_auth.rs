//! Select the resource authorization server before polling identity or exchanging credentials.

use std::collections::HashMap;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::anyhow;
use anyhow::bail;
use codex_exec_server::HttpClient;
use futures::future::BoxFuture;
use http::StatusCode;
use serde::Deserialize;
use serde_json::Value;
use url::Url;

use crate::EmaCredentialLease;
use crate::ema_auth_policy::advertised_capability;
use crate::ema_auth_policy::validate_ema_auth_resource;
use crate::ema_auth_policy::validate_ema_oauth_endpoint;
use crate::ema_auth_policy::validate_ema_public_client_auth;
use crate::ema_exchange::EmaAccessToken;
use crate::ema_exchange::EmaIdJagExchangeRequest;
use crate::ema_exchange::JWT_BEARER_GRANT_TYPE;
use crate::ema_exchange::exchange_id_jag;
use crate::ema_identity::EmaIdpIdentity;
use crate::ema_resource::EmaDiscoveryRequest;
use crate::ema_resource::EmaResource;
use crate::ema_resource::discover_ema_resource;
use crate::ema_resource::validate_ema_metadata_issuer;

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
pub struct EmaAuthTokenExchangeRequest<'a> {
    pub server_url: &'a str,
    pub resource: Option<&'a str>,
    pub scopes: &'a [String],
    pub mcp_client_id: Option<&'a str>,
    pub expected_authorization_server_issuer: Option<&'a str>,
    pub idp_issuer: &'a str,
    pub idp_client_id: &'a str,
    pub idp_identity: BoxFuture<'a, Result<EmaIdpIdentity>>,
    pub idp_http_client: Arc<dyn HttpClient>,
    pub resource_http_client: Arc<dyn HttpClient>,
}

impl<'a> From<&EmaAuthTokenExchangeRequest<'a>> for EmaDiscoveryRequest<'a> {
    fn from(request: &EmaAuthTokenExchangeRequest<'a>) -> Self {
        Self {
            server_url: request.server_url,
            resource: request.resource,
            expected_issuer: request.expected_authorization_server_issuer,
            http_client: Arc::clone(&request.resource_http_client),
        }
    }
}

/// Discover the resource's authorization server, then perform the ID-JAG flow.
pub async fn exchange_ema_auth_token(
    request: EmaAuthTokenExchangeRequest<'_>,
) -> Result<(EmaAccessToken, EmaCredentialLease)> {
    validate_ema_auth_resource(request.server_url, request.resource)?;
    let client_id = request
        .mcp_client_id
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            anyhow!(
                "ema_auth requires the OAuth client ID registered with the MCP authorization server"
            )
        })?;
    let (metadata, resource) =
        discover_ema_authorization_metadata(EmaDiscoveryRequest::from(&request)).await?;
    let issuer = metadata
        .issuer
        .as_deref()
        .ok_or_else(|| anyhow!("MCP authorization metadata is missing an issuer"))?;
    let EmaIdpIdentity {
        token_endpoint,
        refresh_token,
        credentials,
    } = request.idp_identity.await?;
    let token = exchange_id_jag(EmaIdJagExchangeRequest {
        resource: &resource,
        scopes: request.scopes,
        mcp_client_id: client_id,
        authorization_server_issuer: issuer,
        authorization_server_token_endpoint: &metadata.token_endpoint,
        idp_token_endpoint: &token_endpoint,
        idp_issuer: request.idp_issuer,
        idp_client_id: request.idp_client_id,
        refresh_token,
        idp_http_client: request.idp_http_client,
        resource_http_client: request.resource_http_client,
    })
    .await?;
    Ok((token, credentials))
}

async fn discover_ema_authorization_metadata(
    request: EmaDiscoveryRequest<'_>,
) -> Result<(EmaAuthorizationMetadata, String)> {
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
                bail!(
                    "enterprise authorization metadata issuer does not match its advertised issuer"
                );
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
            match validate_ema_public_client_auth(
                metadata
                    .additional_fields
                    .get("token_endpoint_auth_methods_supported"),
                "MCP authorization server",
            ) {
                Ok(()) => return Ok((metadata, resource)),
                Err(error) if expected_issuer.is_some() => return Err(error),
                Err(_) => break,
            }
        }
    }
    bail!(
        "enterprise MCP discovery found no authorization server advertising public-client JWT bearer support for ID-JAG"
    )
}

#[cfg(test)]
#[path = "ema_auth_tests.rs"]
mod tests;
