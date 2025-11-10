//! Registry of model providers supported by Codex.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Codex works out-of-the-box.
//!   2. User-defined entries inside `~/.codex/config.toml` under the `model_providers`
//!      key. These override or extend the defaults at runtime.

use std::collections::HashMap;
use std::env::VarError;
use std::time::Duration;

use codex_app_server_protocol::AuthMode;
use serde::Deserialize;
use serde::Serialize;

use crate::auth::AuthContext;
use crate::error::Error;
use crate::error::Result;

const DEFAULT_STREAM_IDLE_TIMEOUT_MS: i64 = 300_000;
const DEFAULT_STREAM_MAX_RETRIES: i64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: i64 = 4;
/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: i64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: i64 = 100;
const DEFAULT_OLLAMA_PORT: i32 = 11434;

/// Wire protocol that the provider speaks. Most third-party services only
/// implement the classic OpenAI Chat Completions JSON schema, whereas OpenAI
/// itself (and a handful of others) additionally expose the more modern
/// Responses API. The two protocols use different request/response shapes
/// and cannot be auto-detected at runtime, therefore each provider entry
/// must declare which one it expects.
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
    /// Optional instructions to help the user get a valid value for the
    /// variable and set it.
    pub env_key_instructions: Option<String>,
    /// Value to use with `Authorization: Bearer <token>` header. Use of this
    /// config is discouraged in favor of `env_key` for security reasons, but
    /// this may be necessary when using this programmatically.
    pub experimental_bearer_token: Option<String>,
    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,
    /// Optional query parameters to append to the base URL.
    pub query_params: Option<HashMap<String, String>>,
    /// Additional HTTP headers to include in requests to this provider where
    /// the (key, value) pairs are the header name and value.
    pub http_headers: Option<HashMap<String, String>>,
    /// Optional HTTP headers to include in requests to this provider where the
    /// (key, value) pairs are the header name and environment variable whose
    /// value should be used. If the environment variable is not set, or the
    /// value is empty, the header will not be included in the request.
    pub env_http_headers: Option<HashMap<String, String>>,
    /// Maximum number of times to retry a failed HTTP request to this provider.
    pub request_max_retries: Option<i64>,
    /// Number of times to retry reconnecting a dropped streaming response before failing.
    pub stream_max_retries: Option<i64>,
    /// Idle timeout (in milliseconds) to wait for activity on a streaming response before treating
    /// the connection as lost.
    pub stream_idle_timeout_ms: Option<i64>,
    /// Does this provider require an OpenAI API Key or ChatGPT login token? If true,
    /// the user is presented with a login screen on first run, and login preference and token/key
    /// are stored in auth.json. If false (which is the default), the login screen is skipped,
    /// and the API key (if needed) comes from the `env_key` environment variable.
    #[serde(default)]
    pub requires_openai_auth: bool,
}

impl ModelProviderInfo {
    /// Construct a `POST` request builder for the given URL using the provided
    /// [`reqwest::Client`] applying:
    ///   - provider-specific headers (static and environment based)
    ///   - Bearer auth header when an API key is available
    ///   - Auth token for OAuth
    ///
    /// If the provider declares an `env_key` but the variable is missing or empty, this returns an
    /// error identical to the one produced by [`ModelProviderInfo::api_key`].
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

        let url = self.get_full_url(effective_auth.as_ref());
        let mut builder = client.post(url);

        if let Some(context) = effective_auth.as_ref()
            && let Some(token) = context.bearer_token.as_ref()
        {
            builder = builder.bearer_auth(token);
        }

        Ok(self.apply_http_headers(builder))
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

    /// Apply provider-specific HTTP headers (both static and environment-based) onto an existing
    /// [`reqwest::RequestBuilder`] and return the updated builder.
    fn apply_http_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
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

/// Returns the baked-in list of providers. These can be overridden by a `[model_providers]`
/// entry inside `~/.codex/config.toml`.
pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    let mut providers = HashMap::new();

    providers.insert(
        OPENAI_MODEL_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "OpenAI".to_string(),
            base_url: None,
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: Some("Log in to OpenAI and create a new API key at https://platform.openai.com/api-keys. Then paste it here.".to_string()),
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

    providers.insert(
        ANTHROPIC_MODEL_PROVIDER_ID.to_string(),
        ModelProviderInfo {
            name: "Anthropic".to_string(),
            base_url: Some("https://api.anthropic.com/v1/messages".to_string()),
            env_key: Some("ANTHROPIC_API_KEY".to_string()),
            env_key_instructions: Some("Create a new API key at https://console.anthropic.com/settings/keys and paste it here.".to_string()),
            experimental_bearer_token: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: Some(
                maplit::hashmap! {
                    "anthropic-version".to_string() => "2023-06-01".to_string(),
                }
            ),
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
        },
    );

    providers.insert(
        BUILT_IN_OSS_MODEL_PROVIDER_ID.to_string(),
        create_oss_provider_with_base_url("http://localhost:11434"),
    );

    providers
}

pub fn create_oss_provider_with_base_url(url: &str) -> ModelProviderInfo {
    let http_headers = maplit::hashmap! {
        "x-oss-provider".to_string() => "ollama".to_string(),
    };
    ModelProviderInfo {
        name: "Self-hosted OpenAI-compatible (OSS)".to_string(),
        base_url: Some(url.to_string()),
        env_key: Some("CODEX_OSS_PROVIDER_API_KEY".to_string()),
        env_key_instructions: Some(
            "Set CODEX_OSS_PROVIDER_API_KEY to authenticate with this provider.".to_string(),
        ),
        experimental_bearer_token: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: Some(http_headers),
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
    }
}

/// Convenience helper to construct a default `openai/compatible` provider pointing at localhost.
pub fn create_oss_provider() -> ModelProviderInfo {
    create_oss_provider_with_base_url(&format!("http://localhost:{DEFAULT_OLLAMA_PORT}"))
}
