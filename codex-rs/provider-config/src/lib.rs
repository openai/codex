//! Provider configuration shared across Codex layers.
//!
//! This crate defines the provider-agnostic configuration and wire API
//! selection that higher layers (core/app/client) can use. It intentionally
//! avoids Codex-domain concepts like prompts, token counting, or event types.

use std::collections::HashMap;
use std::env::VarError;
use std::time::Duration;

use codex_app_server_protocol::AuthMode;
use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("missing environment variable {var}")]
    MissingEnvVar {
        var: String,
        instructions: Option<String>,
    },
}

pub type Result<T> = std::result::Result<T, Error>;

const DEFAULT_STREAM_IDLE_TIMEOUT_MS: i64 = 300_000;
const DEFAULT_STREAM_MAX_RETRIES: i64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: i64 = 4;
/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: i64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: i64 = 100;
const DEFAULT_OLLAMA_PORT: i32 = 11434;

/// Wire protocol that the provider speaks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    /// The Responses API exposed by OpenAI at `/v1/responses`.
    Responses,
    /// Regular Chat Completions compatible with `/v1/chat/completions`.
    #[default]
    Chat,
}

/// Serializable representation of a provider definition.
#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
pub struct ModelProviderInfo {
    /// Friendly display name.
    pub name: String,
    /// Base URL for the provider's OpenAI-compatible API.
    pub base_url: Option<String>,
    /// Environment variable that stores the user's API key for this provider.
    pub env_key: Option<String>,
    /// Optional instructions to help the user set the environment variable.
    pub env_key_instructions: Option<String>,
    /// Value to use with `Authorization: Bearer <token>` header. Prefer `env_key` when possible.
    pub experimental_bearer_token: Option<String>,
    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,
    /// Optional query parameters to append to the base URL.
    pub query_params: Option<HashMap<String, String>>,
    /// Additional static HTTP headers to include in requests.
    pub http_headers: Option<HashMap<String, String>>,
    /// Optional HTTP headers whose values come from environment variables.
    pub env_http_headers: Option<HashMap<String, String>>,
    /// Maximum number of times to retry a failed HTTP request.
    pub request_max_retries: Option<i64>,
    /// Number of times to retry reconnecting a dropped streaming response before failing.
    pub stream_max_retries: Option<i64>,
    /// Idle timeout (in milliseconds) to wait for activity on a streaming response.
    pub stream_idle_timeout_ms: Option<i64>,
    /// If true, user is prompted for OpenAI login; otherwise uses `env_key`.
    #[serde(default)]
    pub requires_openai_auth: bool,
}

impl ModelProviderInfo {
    /// Construct a `POST` request URL for the configured wire API.
    pub fn get_full_url(&self, auth: Option<&AuthContext>) -> String {
        let default_base_url = if matches!(
            auth,
            Some(AuthContext {
                mode: AuthMode::ChatGPT,
                ..
            })
        ) {
            "https://chatgpt.com/backend-api/codex"
        } else {
            "https://api.openai.com/v1"
        };
        let query_string = self.get_query_string();
        let base_url = self
            .base_url
            .clone()
            .unwrap_or_else(|| default_base_url.to_string());

        match self.wire_api {
            WireApi::Responses => format!("{base_url}/responses{query_string}"),
            WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
        }
    }

    fn get_query_string(&self) -> String {
        self.query_params
            .as_ref()
            .map_or_else(String::new, |params| {
                let full_params = params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("&");
                format!("?{full_params}")
            })
    }

    pub fn is_azure_responses_endpoint(&self) -> bool {
        if self.wire_api != WireApi::Responses {
            return false;
        }
        if self.name.eq_ignore_ascii_case("azure") {
            return true;
        }
        self.base_url
            .as_ref()
            .map(|base| matches_azure_responses_base_url(base))
            .unwrap_or(false)
    }

    /// Apply static and env-derived headers to the provided builder.
    pub fn apply_http_headers(
        &self,
        mut builder: reqwest::RequestBuilder,
    ) -> reqwest::RequestBuilder {
        if let Some(extra) = &self.http_headers {
            for (k, v) in extra {
                builder = builder.header(k, v);
            }
        }
        if let Some(env_headers) = &self.env_http_headers {
            for (header, env_var) in env_headers {
                if let Ok(val) = std::env::var(env_var)
                    && !val.trim().is_empty()
                {
                    builder = builder.header(header, val);
                }
            }
        }
        builder
    }

    pub fn api_key(&self) -> Result<Option<String>> {
        Ok(match self.env_key.as_ref() {
            Some(env_key) => match std::env::var(env_key) {
                Ok(value) if !value.trim().is_empty() => Some(value),
                Ok(_missing) => None,
                Err(VarError::NotPresent) => {
                    let instructions = self.env_key_instructions.clone();
                    return Err(Error::MissingEnvVar {
                        var: env_key.to_string(),
                        instructions,
                    });
                }
                Err(VarError::NotUnicode(_)) => {
                    return Err(Error::MissingEnvVar {
                        var: env_key.to_string(),
                        instructions: None,
                    });
                }
            },
            None => None,
        })
    }

    pub fn stream_max_retries(&self) -> i64 {
        let value = self
            .stream_max_retries
            .unwrap_or(DEFAULT_STREAM_MAX_RETRIES)
            .min(MAX_STREAM_MAX_RETRIES);
        value.max(0)
    }

    pub fn request_max_retries(&self) -> i64 {
        let value = self
            .request_max_retries
            .unwrap_or(DEFAULT_REQUEST_MAX_RETRIES)
            .min(MAX_REQUEST_MAX_RETRIES);
        value.max(0)
    }

    pub fn stream_idle_timeout(&self) -> Duration {
        let ms = self
            .stream_idle_timeout_ms
            .unwrap_or(DEFAULT_STREAM_IDLE_TIMEOUT_MS);
        let clamped = if ms < 0 { 0 } else { ms as u64 };
        Duration::from_millis(clamped)
    }
}

fn matches_azure_responses_base_url(base: &str) -> bool {
    base.starts_with("https://") && base.ends_with(".openai.azure.com/openai/responses")
}

pub const BUILT_IN_OSS_MODEL_PROVIDER_ID: &str = "openai/compatible";
pub const OPENAI_MODEL_PROVIDER_ID: &str = "openai";
pub const ANTHROPIC_MODEL_PROVIDER_ID: &str = "anthropic";

/// Convenience helper to construct a default `openai/compatible` provider pointing at localhost.
pub fn create_oss_provider_with_base_url(url: &str) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "openai/compatible".to_string(),
        base_url: Some(url.to_string()),
        env_key: None,
        env_key_instructions: None,
        experimental_bearer_token: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

pub fn create_oss_provider() -> ModelProviderInfo {
    create_oss_provider_with_base_url(&format!("http://localhost:{DEFAULT_OLLAMA_PORT}"))
}

pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    let mut map = HashMap::new();

    map.insert(
        OPENAI_MODEL_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "OpenAI".to_string(),
            base_url: None,
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: Some(
                "Log in to OpenAI and create a new API key at https://platform.openai.com/api-keys. Then paste it here.".to_string(),
            ),
            experimental_bearer_token: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: true,
        },
    );

    map.insert(
        ANTHROPIC_MODEL_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "Anthropic".to_string(),
            base_url: Some("https://api.anthropic.com/v1".to_string()),
            env_key: Some("ANTHROPIC_API_KEY".to_string()),
            env_key_instructions: None,
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        },
    );

    map.insert(
        BUILT_IN_OSS_MODEL_PROVIDER_ID.to_string(),
        create_oss_provider_with_base_url("http://localhost:11434"),
    );

    map
}

/// Minimal auth context used only for computing URLs and headers.
#[derive(Debug, Clone)]
pub struct AuthContext {
    pub mode: AuthMode,
    pub bearer_token: Option<String>,
    pub account_id: Option<String>,
}

impl ModelProviderInfo {
    /// Convenience to create a request builder with provider and auth headers.
    pub async fn create_request_builder(
        &self,
        client: &reqwest::Client,
        auth: &Option<AuthContext>,
    ) -> Result<reqwest::RequestBuilder> {
        let effective_auth = if let Some(secret_key) = &self.experimental_bearer_token {
            Some(AuthContext {
                mode: AuthMode::ApiKey,
                bearer_token: Some(secret_key.clone()),
                account_id: None,
            })
        } else {
            match self.api_key()? {
                Some(key) => Some(AuthContext {
                    mode: AuthMode::ApiKey,
                    bearer_token: Some(key),
                    account_id: None,
                }),
                None => auth.clone(),
            }
        };

        let mut builder = client.post(self.get_full_url(effective_auth.as_ref()));
        builder = self.apply_http_headers(builder);

        if let Some(context) = effective_auth.as_ref() {
            if let Some(token) = context.bearer_token.as_ref() {
                builder = builder.bearer_auth(token);
            }
            if let Some(account) = context.account_id.as_ref() {
                builder = builder.header("OpenAI-Beta", "codex-2");
                builder = builder.header("OpenAI-Organization", account);
            }
        }

        Ok(builder)
    }
}
