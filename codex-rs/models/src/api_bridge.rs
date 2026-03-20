use codex_api::AuthProvider as ApiAuthProvider;
use codex_api::TransportError;
use codex_api::error::ApiError;
use codex_login::CodexAuth;

use crate::error::ModelsError;
use crate::error::Result;
use crate::model_provider_info::ModelProviderInfo;

pub(crate) fn map_api_error(err: ApiError) -> ModelsError {
    match err {
        ApiError::ContextWindowExceeded => ModelsError::Api("context window exceeded".to_string()),
        ApiError::QuotaExceeded => ModelsError::Api("quota exceeded".to_string()),
        ApiError::UsageNotIncluded => ModelsError::Api("usage not included".to_string()),
        ApiError::Retryable { message, .. } => ModelsError::Api(message),
        ApiError::Stream(message) => ModelsError::Api(message),
        ApiError::ServerOverloaded => ModelsError::Api("server overloaded".to_string()),
        ApiError::Api { status, .. } => ModelsError::Api(format!("api error {}", status.as_u16())),
        ApiError::InvalidRequest { message } => ModelsError::Api(message),
        ApiError::Transport(transport) => ModelsError::Api(match transport {
            TransportError::Http { status, .. } => format!("http {}", status.as_u16()),
            TransportError::RetryLimit => "retry limit reached".to_string(),
            TransportError::Timeout => "timeout".to_string(),
            TransportError::Network(message) | TransportError::Build(message) => message,
        }),
        ApiError::RateLimit(message) => ModelsError::Api(message),
    }
}

pub(crate) fn auth_provider_from_auth(
    auth: Option<CodexAuth>,
    provider: &ModelProviderInfo,
) -> Result<ModelsAuthProvider> {
    if let Some(api_key) = provider.api_key()? {
        return Ok(ModelsAuthProvider {
            token: Some(api_key),
            account_id: None,
        });
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(ModelsAuthProvider {
            token: Some(token),
            account_id: None,
        });
    }

    if let Some(auth) = auth {
        return Ok(ModelsAuthProvider {
            token: Some(auth.get_token()?),
            account_id: auth.get_account_id(),
        });
    }

    Ok(ModelsAuthProvider::default())
}

#[derive(Clone, Default)]
pub(crate) struct ModelsAuthProvider {
    token: Option<String>,
    account_id: Option<String>,
}

impl ModelsAuthProvider {
    pub(crate) fn auth_header_attached(&self) -> bool {
        self.token
            .as_ref()
            .is_some_and(|token| http::HeaderValue::from_str(&format!("Bearer {token}")).is_ok())
    }

    pub(crate) fn auth_header_name(&self) -> Option<&'static str> {
        self.auth_header_attached().then_some("authorization")
    }
}

impl ApiAuthProvider for ModelsAuthProvider {
    fn bearer_token(&self) -> Option<String> {
        self.token.clone()
    }

    fn account_id(&self) -> Option<String> {
        self.account_id.clone()
    }
}
