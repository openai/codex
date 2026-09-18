//! Model catalog cache identity follows provider routing, effective auth, and gateway OAuth config.
//! Only a digest is persisted; access tokens for ChatGPT are excluded so token
//! rotation does not discard a catalog for the same account, user, and plan.

use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::Result as CoreResult;
use sha2::Digest;
use sha2::Sha256;

use crate::auth::resolve_provider_auth;

pub(crate) fn identity(
    provider_info: &ModelProviderInfo,
    auth: Option<&CodexAuth>,
) -> CoreResult<String> {
    let mut api_provider = provider_info.to_api_provider(auth.map(CodexAuth::auth_mode))?;
    let mut digest = Sha256::new();
    // Length-prefix every field to avoid ambiguity between adjacent values.
    let mut field = |value: &[u8]| {
        digest.update((value.len() as u64).to_le_bytes());
        digest.update(value);
    };
    field(b"models-cache-v1");
    field(api_provider.name.as_bytes());
    field(api_provider.base_url.as_bytes());
    // Preserve existing cache identities when no explicit catalog is configured.
    // The API provider resolves base_url defaults; model_catalog_url stays on ModelProviderInfo.
    if let Some(catalog_url) = &provider_info.model_catalog_url {
        field(b"model_catalog_url");
        field(catalog_url.as_bytes());
    }
    let mut query: Vec<_> = api_provider.query_params.iter().flatten().collect();
    query.sort();
    field(&(query.len() as u64).to_le_bytes());
    for (name, value) in query {
        field(name.as_bytes());
        field(value.as_bytes());
    }
    field(&[u8::from(provider_info.requires_openai_auth)]);
    field(&[u8::from(provider_info.has_command_auth())]);
    field(format!("{:?}", auth.map(CodexAuth::api_auth_mode)).as_bytes());
    if let Some(auth) = auth {
        field(format!("{:?}", auth.get_account_id()).as_bytes());
        field(format!("{:?}", auth.get_chatgpt_user_id()).as_bytes());
        field(format!("{:?}", auth.get_account_email()).as_bytes());
        field(format!("{:?}", auth.account_plan_type()).as_bytes());
        field(
            format!(
                "{:?}",
                auth.get_token_data()
                    .ok()
                    .and_then(|tokens| tokens.id_token.get_chatgpt_plan_type_raw())
            )
            .as_bytes(),
        );
        field(&[u8::from(auth.is_fedramp_account())]);
    }
    let has_stable_account = auth.is_some_and(|auth| {
        matches!(
            auth,
            CodexAuth::Chatgpt(_) | CodexAuth::ChatgptAuthTokens(_) | CodexAuth::AgentIdentity(_)
        ) && auth.get_account_id().is_some()
            && (auth.get_chatgpt_user_id().is_some() || auth.get_account_email().is_some())
    });
    let explicit_bearer =
        provider_info.api_key()?.is_some() || provider_info.experimental_bearer_token.is_some();
    // Command auth also returns opaque API keys. A changed key may belong to a
    // different account, so only credentials with stable owner metadata can be reused.
    if !has_stable_account || explicit_bearer {
        let headers = resolve_provider_auth(auth, provider_info)?.to_auth_headers();
        api_provider.headers.extend(headers);
    }
    let mut headers: Vec<_> = api_provider.headers.iter().collect();
    headers.sort_by(|(a, av), (b, bv)| {
        a.as_str()
            .cmp(b.as_str())
            .then(av.as_bytes().cmp(bv.as_bytes()))
    });
    field(&(headers.len() as u64).to_le_bytes());
    for (name, value) in headers {
        field(name.as_str().as_bytes());
        field(value.as_bytes());
    }
    // Preserve existing identities for providers without gateway OAuth.
    if let Some(gateway_oauth) = &provider_info.gateway_oauth {
        field(&serde_json::to_vec(gateway_oauth)?);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
#[path = "models_identity_tests.rs"]
mod tests;
