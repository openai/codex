//! Composes primary provider credentials with independently managed gateway OAuth credentials.

use std::sync::Arc;

use crate::GatewayAuthManager;
use crate::auth::ResolvedProviderAuth;
use codex_api::AuthError;
use codex_api::AuthHeaderTelemetry;
use codex_api::AuthHeadersFuture;
use codex_api::AuthProvider;
use codex_api::AuthProviderFuture;
use codex_api::SharedAuthProvider;
use codex_model_provider_info::GatewayOAuthDelivery;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use http::HeaderMap;
use http::HeaderName;
use http::HeaderValue;

pub(crate) async fn compose_auth(
    provider: &ModelProviderInfo,
    manager: Option<&std::result::Result<Arc<GatewayAuthManager>, String>>,
    mut resolved: ResolvedProviderAuth,
) -> Result<ResolvedProviderAuth> {
    let Some(config) = provider.gateway_oauth.as_ref() else {
        return Ok(resolved);
    };
    provider.validate().map_err(CodexErr::InvalidRequest)?;
    let manager = manager.ok_or_else(|| {
        CodexErr::InvalidRequest("gateway_oauth requires auth runtime configuration".into())
    })?;
    let manager = manager
        .as_ref()
        .map_err(|error| CodexErr::InvalidRequest(error.clone()))?;
    let token = manager.resolve_access_token().await?;
    let (name, value) = gateway_header(config.delivery(), &token)?;
    resolved.auth = Arc::new(CombinedAuth {
        primary: resolved.auth,
        name,
        value,
    });
    Ok(resolved)
}

fn gateway_header(
    delivery: &GatewayOAuthDelivery,
    token: &str,
) -> Result<(HeaderName, HeaderValue)> {
    if token.is_empty() {
        return Err(CodexErr::InvalidRequest(
            "gateway access token is empty".into(),
        ));
    }
    let (name, value) = match delivery {
        GatewayOAuthDelivery::Header { name, scheme } => (
            HeaderName::from_bytes(name.as_bytes())
                .map_err(|_| CodexErr::InvalidRequest("invalid gateway header".into()))?,
            format!("{scheme} {token}"),
        ),
        GatewayOAuthDelivery::Cookie { name } => (http::header::COOKIE, format!("{name}={token}")),
    };
    let mut value = HeaderValue::from_str(&value)
        .map_err(|_| CodexErr::InvalidRequest("invalid gateway OAuth token header".into()))?;
    value.set_sensitive(true);
    Ok((name, value))
}

struct CombinedAuth {
    primary: SharedAuthProvider,
    name: HeaderName,
    value: HeaderValue,
}

impl CombinedAuth {
    fn insert_gateway_header(&self, headers: &mut HeaderMap) -> std::result::Result<(), AuthError> {
        if headers.contains_key(&self.name) {
            return Err(AuthError::Build(format!(
                "The request already sets the {} header. Choose a different header or cookie in gateway_oauth.delivery for the gateway token.",
                self.name
            )));
        }
        headers.insert(self.name.clone(), self.value.clone());
        Ok(())
    }
}

impl AuthProvider for CombinedAuth {
    fn auth_header_telemetry(&self) -> std::result::Result<AuthHeaderTelemetry, AuthError> {
        self.primary.auth_header_telemetry()
    }

    fn add_auth_headers(&self, headers: &mut HeaderMap) -> std::result::Result<(), AuthError> {
        self.primary.add_auth_headers(headers)?;
        self.insert_gateway_header(headers)
    }

    fn apply_auth_headers(&self, headers: HeaderMap) -> AuthHeadersFuture<'_> {
        Box::pin(async move {
            let mut headers = self.primary.apply_auth_headers(headers).await?;
            self.insert_gateway_header(&mut headers)?;
            Ok(headers)
        })
    }

    fn apply_auth(&self, request: codex_http_client::Request) -> AuthProviderFuture<'_> {
        Box::pin(async move {
            let mut request = self.primary.apply_auth(request).await?;
            self.insert_gateway_header(&mut request.headers)?;
            Ok(request)
        })
    }
}
