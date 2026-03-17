use codex_otel::AuthEnvTelemetryMetadata;
use std::collections::BTreeSet;

use crate::auth::CODEX_API_KEY_ENV_VAR;
use crate::auth::OPENAI_API_KEY_ENV_VAR;
use crate::auth::REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR;
use crate::model_provider_info::ModelProviderInfo;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct AuthEnvTelemetry {
    sources: BTreeSet<AuthEnvSource>,
    codex_api_key_env_enabled: bool,
    provider_env_key_configured: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum AuthEnvSource {
    OpenAiApiKey,
    CodexApiKey,
    ProviderEnvKey,
    RefreshTokenUrlOverride,
}

impl AuthEnvTelemetry {
    pub(crate) fn new(
        sources: impl IntoIterator<Item = AuthEnvSource>,
        codex_api_key_env_enabled: bool,
        provider_env_key_configured: bool,
    ) -> Self {
        Self {
            sources: sources.into_iter().collect(),
            codex_api_key_env_enabled,
            provider_env_key_configured,
        }
    }

    pub(crate) fn openai_api_key_env_present(&self) -> bool {
        self.sources.contains(&AuthEnvSource::OpenAiApiKey)
    }

    pub(crate) fn codex_api_key_env_present(&self) -> bool {
        self.sources.contains(&AuthEnvSource::CodexApiKey)
    }

    pub(crate) fn codex_api_key_env_enabled(&self) -> bool {
        self.codex_api_key_env_enabled
    }

    pub(crate) fn provider_env_key_name(&self) -> Option<&'static str> {
        self.provider_env_key_configured.then_some("configured")
    }

    pub(crate) fn provider_env_key_present(&self) -> Option<bool> {
        self.provider_env_key_configured
            .then(|| self.has(AuthEnvSource::ProviderEnvKey))
    }

    pub(crate) fn refresh_token_url_override_present(&self) -> bool {
        self.sources
            .contains(&AuthEnvSource::RefreshTokenUrlOverride)
    }

    fn has(&self, source: AuthEnvSource) -> bool {
        self.sources.contains(&source)
    }

    pub(crate) fn to_otel_metadata(&self) -> AuthEnvTelemetryMetadata {
        AuthEnvTelemetryMetadata {
            openai_api_key_env_present: self.openai_api_key_env_present(),
            codex_api_key_env_present: self.codex_api_key_env_present(),
            codex_api_key_env_enabled: self.codex_api_key_env_enabled(),
            provider_env_key_name: self.provider_env_key_name().map(str::to_string),
            provider_env_key_present: self.provider_env_key_present(),
            refresh_token_url_override_present: self.refresh_token_url_override_present(),
        }
    }
}

pub(crate) fn collect_auth_env_telemetry(
    provider: &ModelProviderInfo,
    codex_api_key_env_enabled: bool,
) -> AuthEnvTelemetry {
    let mut sources = Vec::new();
    if env_var_present(OPENAI_API_KEY_ENV_VAR) {
        sources.push(AuthEnvSource::OpenAiApiKey);
    }
    if env_var_present(CODEX_API_KEY_ENV_VAR) {
        sources.push(AuthEnvSource::CodexApiKey);
    }
    if provider.env_key.as_deref().is_some_and(env_var_present) {
        sources.push(AuthEnvSource::ProviderEnvKey);
    }
    if env_var_present(REFRESH_TOKEN_URL_OVERRIDE_ENV_VAR) {
        sources.push(AuthEnvSource::RefreshTokenUrlOverride);
    }

    AuthEnvTelemetry::new(
        sources,
        codex_api_key_env_enabled,
        provider.env_key.is_some(),
    )
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
            requires_openai_auth: false,
            supports_websockets: false,
        };

        let telemetry = collect_auth_env_telemetry(&provider, false);

        assert_eq!(telemetry.provider_env_key_name(), Some("configured"));
    }
}
