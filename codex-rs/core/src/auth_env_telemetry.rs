use codex_otel::AuthEnvTelemetryMetadata;

use crate::auth::CODEX_API_KEY_ENV_VAR;
use crate::auth::OPENAI_API_KEY_ENV_VAR;
use crate::auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use crate::model_provider_info::ModelProviderInfo;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AuthEnvTelemetry {
    pub(crate) openai_api_key_env_present: bool,
    pub(crate) codex_api_key_env_present: bool,
    pub(crate) codex_api_key_env_enabled: bool,
    pub(crate) provider_env_key_name: Option<String>,
    pub(crate) provider_env_key_present: Option<bool>,
    pub(crate) refresh_token_url_override_present: bool,
}

impl AuthEnvTelemetry {
    pub(crate) fn to_otel_metadata(&self) -> AuthEnvTelemetryMetadata {
        AuthEnvTelemetryMetadata {
            openai_api_key_env_present: self.openai_api_key_env_present,
            codex_api_key_env_present: self.codex_api_key_env_present,
            codex_api_key_env_enabled: self.codex_api_key_env_enabled,
            provider_env_key_name: self.provider_env_key_name.clone(),
            provider_env_key_present: self.provider_env_key_present,
            refresh_token_url_override_present: self.refresh_token_url_override_present,
        }
    }
}

pub(crate) fn collect_auth_env_telemetry(
    provider: &ModelProviderInfo,
    codex_api_key_env_enabled: bool,
) -> AuthEnvTelemetry {
    AuthEnvTelemetry {
        openai_api_key_env_present: env_var_present(OPENAI_API_KEY_ENV_VAR),
        codex_api_key_env_present: env_var_present(CODEX_API_KEY_ENV_VAR),
        codex_api_key_env_enabled,
        // Custom provider `env_key` is arbitrary config text, so emit only a safe bucket.
        provider_env_key_name: provider.env_key.as_ref().map(|_| "configured".to_string()),
        provider_env_key_present: provider.env_key.as_deref().map(env_var_present),
        refresh_token_url_override_present: env_var_present(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR),
    }
}

fn env_var_present(name: &str) -> bool {
    match std::env::var(name) {
        Ok(value) => !value.trim().is_empty(),
        Err(std::env::VarError::NotUnicode(_)) => true,
        Err(std::env::VarError::NotPresent) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn collect_auth_env_telemetry_buckets_provider_env_key_name() {
        let provider = ModelProviderInfo {
            name: "Custom".to_string(),
            base_url: None,
            env_key: Some("sk-should-not-leak".to_string()),
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: crate::model_provider_info::WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            websocket_connect_timeout_ms: None,
            requires_openai_auth: false,
            supports_websockets: false,
        };

        let telemetry = collect_auth_env_telemetry(&provider, false);

        assert_eq!(
            telemetry.provider_env_key_name,
            Some("configured".to_string())
        );
    }
}
