use std::sync::Arc;

use crate::auth::AuthContext;
use crate::auth::AuthProvider;
use crate::error::Result;
use codex_provider_config::ModelProviderInfo;

/// Build a request builder with provider/auth/session headers applied.
pub async fn build_request(
    http_client: &reqwest::Client,
    provider: &ModelProviderInfo,
    auth: &Option<AuthContext>,
    extra_headers: &[(&str, String)],
) -> Result<reqwest::RequestBuilder> {
    let mut builder = provider
        .create_request_builder(
            http_client,
            &auth.as_ref().map(|a| codex_provider_config::AuthContext {
                mode: a.mode,
                bearer_token: a.bearer_token.clone(),
                account_id: a.account_id.clone(),
            }),
        )
        .await
        .map_err(|e| crate::error::Error::MissingEnvVar {
            var: match e {
                codex_provider_config::Error::MissingEnvVar { ref var, .. } => var.clone(),
            },
            instructions: match e {
                codex_provider_config::Error::MissingEnvVar {
                    ref instructions, ..
                } => instructions.clone(),
            },
        })?;
    for (name, value) in extra_headers {
        builder = builder.header(*name, value);
    }
    Ok(builder)
}

/// Resolve auth context from an optional provider.
pub async fn resolve_auth(auth_provider: &Option<Arc<dyn AuthProvider>>) -> Option<AuthContext> {
    if let Some(p) = auth_provider {
        p.auth_context().await
    } else {
        None
    }
}

/// Convert owned header pairs into borrowed key/value tuples for reqwest.
pub fn header_pairs(headers: &[(String, String)]) -> Vec<(&str, String)> {
    headers
        .iter()
        .map(|(k, v)| (k.as_str(), v.clone()))
        .collect()
}
