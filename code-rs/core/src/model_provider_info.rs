//! Registry of model providers supported by Codex.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Codex works out-of-the-box.
//!   2. User-defined entries inside `~/.code/config.toml` under the `model_providers`
//!      table (Code also reads legacy `~/.codex/config.toml`).
//!      key. These override or extend the defaults at runtime.

use crate::CodexAuth;
use code_app_server_protocol::AuthMode;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::{BTreeMap, HashMap};
use std::env::VarError;
use std::time::Duration;
use crate::error::EnvVarError;
const DEFAULT_STREAM_IDLE_TIMEOUT_MS: u64 = 300_000;
const DEFAULT_STREAM_MAX_RETRIES: u64 = 5;
const DEFAULT_REQUEST_MAX_RETRIES: u64 = 4;
/// Hard cap for user-configured `stream_max_retries`.
const MAX_STREAM_MAX_RETRIES: u64 = 100;
/// Hard cap for user-configured `request_max_retries`.
const MAX_REQUEST_MAX_RETRIES: u64 = 100;

/// Wire protocol that the provider speaks. Most third-party services only
/// implement the classic OpenAI Chat Completions JSON schema, whereas OpenAI
/// itself (and a handful of others) additionally expose the more modern
/// *Responses* API. The two protocols use different request/response shapes
/// and *cannot* be auto-detected at runtime, therefore each provider entry
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

    /// Which wire protocol this provider expects.
    #[serde(default)]
    pub wire_api: WireApi,

    /// Optional query parameters to append to the base URL.
    pub query_params: Option<HashMap<String, String>>,

    /// Additional HTTP headers to include in requests to this provider where
    /// the (key, value) pairs are the header name and value.
    pub http_headers: Option<HashMap<String, String>>,

    /// Optional HTTP headers to include in requests to this provider where the
    /// (key, value) pairs are the header name and _environment variable_ whose
    /// value should be used. If the environment variable is not set, or the
    /// value is empty, the header will not be included in the request.
    pub env_http_headers: Option<HashMap<String, String>>,

    /// Maximum number of times to retry a failed HTTP request to this provider.
    pub request_max_retries: Option<u64>,

    /// Number of times to retry reconnecting a dropped streaming response before failing.
    pub stream_max_retries: Option<u64>,

    /// Idle timeout (in milliseconds) to wait for activity on a streaming response before treating
    /// the connection as lost.
    pub stream_idle_timeout_ms: Option<u64>,

    /// Whether this provider requires some form of standard authentication (API key, ChatGPT token).
    #[serde(default)]
    pub requires_openai_auth: bool,

    /// Optional OpenRouter-specific configuration for routing preferences and metadata.
    #[serde(default)]
    pub openrouter: Option<OpenRouterConfig>,
}

/// OpenRouter-specific configuration, allowing users to control routing and pricing metadata.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OpenRouterConfig {
    /// Provider-level routing preferences forwarded to OpenRouter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<OpenRouterProviderConfig>,

    /// Optional `route` payload forwarded as-is to OpenRouter for advanced routing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub route: Option<Value>,

    /// Additional top-level fields that may be forwarded to OpenRouter as the API evolves.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

/// Provider routing preferences supported by OpenRouter.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OpenRouterProviderConfig {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allow_fallbacks: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_parameters: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_collection: Option<OpenRouterDataCollectionPolicy>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zdr: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub only: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ignore: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quantizations: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sort: Option<OpenRouterProviderSort>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_price: Option<OpenRouterMaxPrice>,

    /// Catch-all for additional provider keys so new OpenRouter features do not break deserialization.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OpenRouterDataCollectionPolicy {
    Allow,
    Deny,
}

impl Default for OpenRouterDataCollectionPolicy {
    fn default() -> Self {
        OpenRouterDataCollectionPolicy::Allow
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum OpenRouterProviderSort {
    Price,
    Throughput,
    Latency,
}

impl Default for OpenRouterProviderSort {
    fn default() -> Self {
        OpenRouterProviderSort::Price
    }
}

/// `max_price` envelope for OpenRouter provider routing controls.
#[derive(Debug, Clone, Default, Deserialize, Serialize, PartialEq)]
#[serde(default)]
pub struct OpenRouterMaxPrice {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub prompt: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completion: Option<f64>,

    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

impl ModelProviderInfo {
    /// Construct a `POST` RequestBuilder for the given URL using the provided
    /// reqwest Client applying:
    ///   • provider-specific headers (static + env based)
    ///   • Bearer auth header when an API key is available.
    ///   • Auth token for OAuth.
    ///
    /// If the provider declares an `env_key` but the variable is missing/empty, returns an [`Err`] identical to the
    /// one produced by [`ModelProviderInfo::api_key`].
    pub async fn create_request_builder<'a>(
        &'a self,
        client: &'a reqwest::Client,
        auth: &Option<CodexAuth>,
    ) -> crate::error::Result<reqwest::RequestBuilder> {
        let effective_auth = match self.api_key() {
            Ok(Some(key)) => Some(CodexAuth::from_api_key(&key)),
            Ok(None) => auth.clone(),
            Err(err) => {
                if auth.is_some() {
                    auth.clone()
                } else {
                    return Err(err);
                }
            }
        };

        let url = self.get_full_url(&effective_auth);

        let mut builder = client.post(&url);

        // Always set an explicit Host header that matches the upstream target.
        // Some forward proxies incorrectly reuse the inbound Host header
        // (e.g. "127.0.0.1:5055") for TLS SNI when connecting to the
        // upstream server, which causes handshake failures. By setting
        // Host to the authority derived from the final URL here, we ensure
        // the proxy sees the correct host and can forward/SNI appropriately.
        if let Ok(parsed) = url::Url::parse(&url) {
            if let Some(host) = parsed.host_str() {
                let authority = match parsed.port() {
                    Some(port) => format!("{host}:{port}"),
                    None => host.to_string(),
                };
                if let Ok(hv) = reqwest::header::HeaderValue::from_str(&authority) {
                    builder = builder.header(reqwest::header::HOST, hv);
                }
            }
        }

        if let Some(auth) = effective_auth.as_ref() {
            builder = builder.bearer_auth(auth.get_token().await?);
        }

        Ok(self.apply_http_headers(builder))
    }

    /// Returns the OpenRouter-specific configuration, if this provider declares one.
    pub fn openrouter_config(&self) -> Option<&OpenRouterConfig> {
        self.openrouter.as_ref()
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

    pub(crate) fn get_full_url(&self, auth: &Option<CodexAuth>) -> String {
        let default_base_url = if matches!(
            auth,
            Some(CodexAuth {
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
            .unwrap_or(default_base_url.to_string());

        match self.wire_api {
            WireApi::Responses => format!("{base_url}/responses{query_string}"),
            WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
        }
    }

    pub(crate) fn is_azure_responses_endpoint(&self) -> bool {
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

    pub(crate) fn is_backend_responses_endpoint(&self) -> bool {
        if self.wire_api != WireApi::Responses {
            return false;
        }

        if self.name.eq_ignore_ascii_case("backend") {
            return true;
        }

        self.base_url
            .as_ref()
            .map_or(false, |base| base.contains("/backend-api"))
    }

    pub(crate) fn is_public_openai_responses_endpoint(&self) -> bool {
        if self.wire_api != WireApi::Responses {
            return false;
        }
        if self.is_backend_responses_endpoint() || self.is_azure_responses_endpoint() {
            return false;
        }

        self.base_url
            .as_ref()
            .and_then(|base| url::Url::parse(base).ok())
            .and_then(|parsed| parsed.host_str().map(|host| host.eq_ignore_ascii_case("api.openai.com")))
            .unwrap_or(true)
    }

    /// Apply provider-specific HTTP headers (both static and environment-based)
    /// onto an existing `reqwest::RequestBuilder` and return the updated
    /// builder.
    fn apply_http_headers(&self, mut builder: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if let Some(extra) = &self.http_headers {
            for (k, v) in extra {
                builder = builder.header(k, v);
            }
        }

        if let Some(env_headers) = &self.env_http_headers {
            for (header, env_var) in env_headers {
                if let Ok(val) = std::env::var(env_var) {
                    if !val.trim().is_empty() {
                        builder = builder.header(header, val);
                    }
                }
            }
        }
        builder
    }

    /// If `env_key` is Some, returns the API key for this provider if present
    /// (and non-empty) in the environment. If `env_key` is required but
    /// cannot be found, returns an error.
    pub fn api_key(&self) -> crate::error::Result<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let env_value = std::env::var(env_key);
                env_value
                    .and_then(|v| {
                        if v.trim().is_empty() {
                            Err(VarError::NotPresent)
                        } else {
                            Ok(Some(v))
                        }
                    })
                    .map_err(|_| {
                        crate::error::CodexErr::EnvVar(EnvVarError {
                            var: env_key.clone(),
                            instructions: self.env_key_instructions.clone(),
                        })
                    })
            }
            None => Ok(None),
        }
    }

    /// Effective maximum number of request retries for this provider.
    pub fn request_max_retries(&self) -> u64 {
        self.request_max_retries
            .unwrap_or(DEFAULT_REQUEST_MAX_RETRIES)
            .min(MAX_REQUEST_MAX_RETRIES)
    }

    /// Effective maximum number of stream reconnection attempts for this provider.
    pub fn stream_max_retries(&self) -> u64 {
        self.stream_max_retries
            .unwrap_or(DEFAULT_STREAM_MAX_RETRIES)
            .min(MAX_STREAM_MAX_RETRIES)
    }

    /// Effective idle timeout for streaming responses.
    pub fn stream_idle_timeout(&self) -> Duration {
        self.stream_idle_timeout_ms
            .map(Duration::from_millis)
            .unwrap_or(Duration::from_millis(DEFAULT_STREAM_IDLE_TIMEOUT_MS))
    }
}

const DEFAULT_OLLAMA_PORT: u32 = 11434;

pub const BUILT_IN_OSS_MODEL_PROVIDER_ID: &str = "oss";

/// Built-in default provider list.
fn wire_api_override_from_env(env_key: &str) -> Option<WireApi> {
    match std::env::var(env_key) {
        Ok(value) => match value.trim().to_ascii_lowercase().as_str() {
            "chat" => Some(WireApi::Chat),
            "responses" => Some(WireApi::Responses),
            other if !other.is_empty() => {
                tracing::warn!(
                    "Ignoring unknown {env_key} value '{other}'; falling back to default wire API"
                );
                None
            }
            _ => None,
        },
        Err(_) => None,
    }
}

pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    use ModelProviderInfo as P;

    // We do not want to be in the business of adjucating which third-party
    // providers are bundled with Codex CLI, so we only include the OpenAI and
    // open source ("oss") providers by default. Users are encouraged to add to
    // `model_providers` in config.toml to add their own providers.
    [
        (
            "openai",
            P {
                name: "OpenAI".into(),
                // Allow users to override the default OpenAI endpoint by
                // exporting `OPENAI_BASE_URL`. This is useful when pointing
                // Codex at a proxy, mock server, or Azure-style deployment
                // without requiring a full TOML override for the built-in
                // OpenAI provider.
                base_url: std::env::var("OPENAI_BASE_URL")
                    .ok()
                    .filter(|v| !v.trim().is_empty()),
                env_key: None,
                env_key_instructions: None,
                wire_api: wire_api_override_from_env("OPENAI_WIRE_API")
                    .unwrap_or(WireApi::Responses),
                query_params: None,
                http_headers: Some(
                    [
                        (
                            "version".to_string(),
                            code_version::version().to_string(),
                        ),
                    ]
                    .into_iter()
                    .collect(),
                ),
                env_http_headers: Some(
                    [
                        (
                            "OpenAI-Organization".to_string(),
                            "OPENAI_ORGANIZATION".to_string(),
                        ),
                        ("OpenAI-Project".to_string(), "OPENAI_PROJECT".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                ),
                // Use global defaults for retry/timeout unless overridden in config.toml.
                request_max_retries: None,
                stream_max_retries: None,
                stream_idle_timeout_ms: None,
                requires_openai_auth: true,
                openrouter: None,
            },
        ),
        (BUILT_IN_OSS_MODEL_PROVIDER_ID, create_oss_provider()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

pub fn create_oss_provider() -> ModelProviderInfo {
    // These CODEX_OSS_ environment variables are experimental: we may
    // switch to reading values from config.toml instead.
    let code_oss_base_url = match std::env::var("CODEX_OSS_BASE_URL")
        .ok()
        .filter(|v| !v.trim().is_empty())
    {
        Some(url) => url,
        None => format!(
            "http://localhost:{port}/v1",
            port = std::env::var("CODEX_OSS_PORT")
                .ok()
                .filter(|v| !v.trim().is_empty())
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(DEFAULT_OLLAMA_PORT)
        ),
    };

    create_oss_provider_with_base_url(&code_oss_base_url)
}

pub fn create_oss_provider_with_base_url(base_url: &str) -> ModelProviderInfo {
    ModelProviderInfo {
        name: "gpt-oss".into(),
        base_url: Some(base_url.into()),
        env_key: None,
        env_key_instructions: None,
        wire_api: WireApi::Chat,
        query_params: None,
        http_headers: None,
        env_http_headers: None,
        request_max_retries: None,
        stream_max_retries: None,
        stream_idle_timeout_ms: None,
        requires_openai_auth: false,
        openrouter: None,
    }
}

fn matches_azure_responses_base_url(base_url: &str) -> bool {
    let base = base_url.to_ascii_lowercase();
    const AZURE_MARKERS: [&str; 5] = [
        "openai.azure.",
        "cognitiveservices.azure.",
        "aoai.azure.",
        "azure-api.",
        "azurefd.",
    ];
    AZURE_MARKERS.iter().any(|marker| base.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_deserialize_ollama_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Ollama"
base_url = "http://localhost:11434/v1"
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Ollama".into(),
            base_url: Some("http://localhost:11434/v1".into()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_deserialize_azure_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Azure"
base_url = "https://xxxxx.openai.azure.com/openai"
env_key = "AZURE_OPENAI_API_KEY"
query_params = { api-version = "2025-04-01-preview" }
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Azure".into(),
            base_url: Some("https://xxxxx.openai.azure.com/openai".into()),
            env_key: Some("AZURE_OPENAI_API_KEY".into()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: Some(maplit::hashmap! {
                "api-version".to_string() => "2025-04-01-preview".to_string(),
            }),
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_deserialize_example_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Example"
base_url = "https://example.com"
env_key = "API_KEY"
http_headers = { "X-Example-Header" = "example-value" }
env_http_headers = { "X-Example-Env-Header" = "EXAMPLE_ENV_VAR" }
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Example".into(),
            base_url: Some("https://example.com".into()),
            env_key: Some("API_KEY".into()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: Some(maplit::hashmap! {
                "X-Example-Header".to_string() => "example-value".to_string(),
            }),
            env_http_headers: Some(maplit::hashmap! {
                "X-Example-Env-Header".to_string() => "EXAMPLE_ENV_VAR".to_string(),
            }),
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn detects_azure_responses_base_urls() {
        fn provider_for(base_url: &str) -> ModelProviderInfo {
            ModelProviderInfo {
                name: "test".into(),
                base_url: Some(base_url.into()),
                env_key: None,
                env_key_instructions: None,
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: None,
                env_http_headers: None,
                request_max_retries: None,
                stream_max_retries: None,
                stream_idle_timeout_ms: None,
                requires_openai_auth: false,
                openrouter: None,
            }
        }

        let positive_cases = [
            "https://foo.openai.azure.com/openai",
            "https://foo.openai.azure.us/openai/deployments/bar",
            "https://foo.cognitiveservices.azure.cn/openai",
            "https://foo.aoai.azure.com/openai",
            "https://foo.openai.azure-api.net/openai",
            "https://foo.z01.azurefd.net/",
        ];
        for base_url in positive_cases {
            let provider = provider_for(base_url);
            assert!(
                provider.is_azure_responses_endpoint(),
                "expected {base_url} to be detected as Azure"
            );
        }

        let named_provider = ModelProviderInfo {
            name: "Azure".into(),
            base_url: Some("https://example.com".into()),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
            request_max_retries: None,
            stream_max_retries: None,
            stream_idle_timeout_ms: None,
            requires_openai_auth: false,
            openrouter: None,
        };
        assert!(named_provider.is_azure_responses_endpoint());

        let negative_cases = [
            "https://api.openai.com/v1",
            "https://example.com/openai",
            "https://myproxy.azurewebsites.net/openai",
        ];
        for base_url in negative_cases {
            let provider = provider_for(base_url);
            assert!(
                !provider.is_azure_responses_endpoint(),
                "expected {base_url} not to be detected as Azure"
            );
        }
    }
}
