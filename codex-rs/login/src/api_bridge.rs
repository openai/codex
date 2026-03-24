use codex_api::CoreAuthProvider;
use codex_model_provider_info::ModelProviderInfo;

use crate::CodexAuth;

pub fn auth_provider_from_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> codex_protocol::error::Result<CoreAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(CoreAuthProvider::from_bearer_token(Some(api_key), None));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(CoreAuthProvider::from_bearer_token(Some(token), None));
    }

    if let Some(auth) = auth {
        let token = auth.get_token()?;
        Ok(CoreAuthProvider::from_bearer_token(
            Some(token),
            auth.get_account_id(),
        ))
    } else {
        Ok(CoreAuthProvider::from_bearer_token(None, None))
    }
}
