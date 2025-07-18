//! Registry of model providers supported by Codex.
//!
//! Providers can be defined in two places:
//!   1. Built-in defaults compiled into the binary so Codex works out-of-the-box.
//!   2. User-defined entries inside `~/.codex/config.toml` under the `model_providers`
//!      key. These override or extend the defaults at runtime.

use serde::Deserialize;
use serde::Serialize;
use std::collections::HashMap;
use std::env::VarError;

use crate::error::EnvVarError;
use crate::openai_api_key::get_openai_api_key;

/// Value for the `OpenAI-Originator` header that is sent with requests to
/// OpenAI.
const OPENAI_ORIGINATOR_HEADER: &str = "codex_cli_rs";

/// Wire protocol that the provider speaks. Most third-party services only
/// implement the classic OpenAI Chat Completions JSON schema, whereas OpenAI
/// itself (and a handful of others) additionally expose the more modern
/// *Responses* API. The two protocols use different request/response shapes
/// and *cannot* be auto-detected at runtime, therefore each provider entry
/// must declare which one it expects.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WireApi {
    /// The experimental “Responses” API exposed by OpenAI at `/v1/responses`.
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
    pub base_url: String,
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
}

impl ModelProviderInfo {
    /// Construct a `POST` RequestBuilder for the given URL using the provided
    /// reqwest Client applying:
    ///   • provider-specific headers (static + env based)
    ///   • Bearer auth header when an API key is available.
    ///
    /// When `require_api_key` is true and the provider declares an `env_key`
    /// but the variable is missing/empty, returns an [`Err`] identical to the
    /// one produced by [`ModelProviderInfo::api_key`].
    pub fn create_request_builder<'a>(
        &'a self,
        client: &'a reqwest::Client,
    ) -> crate::error::Result<reqwest::RequestBuilder> {
        let api_key = self.api_key()?;

        let url = self.get_full_url();

        let mut builder = client.post(url);
        if let Some(key) = api_key {
            builder = builder.bearer_auth(key);
        }

        Ok(self.apply_http_headers(builder))
    }

    pub(crate) fn get_full_url(&self) -> String {
        let query_string = self
            .query_params
            .as_ref()
            .map_or_else(String::new, |params| {
                let full_params = params
                    .iter()
                    .map(|(k, v)| format!("{k}={v}"))
                    .collect::<Vec<_>>()
                    .join("&");
                format!("?{full_params}")
            });
        let base_url = &self.base_url;
        match self.wire_api {
            WireApi::Responses => format!("{base_url}/responses{query_string}"),
            WireApi::Chat => format!("{base_url}/chat/completions{query_string}"),
        }
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
    /// (and non-empty) in the environment. Returns None if the key is not found,
    /// allowing providers to work without API keys (e.g., local providers like Ollama).
    fn api_key(&self) -> crate::error::Result<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let env_value = if env_key == crate::openai_api_key::OPENAI_API_KEY_ENV_VAR {
                    get_openai_api_key().map_or_else(|| Err(VarError::NotPresent), Ok)
                } else {
                    std::env::var(env_key)
                };
                match env_value {
                    Ok(v) if !v.trim().is_empty() => Ok(Some(v)),
                    _ => {
                        // For OpenAI provider, missing API key is still an error
                        // For other providers, missing API key is allowed (e.g., local providers)
                        if env_key == crate::openai_api_key::OPENAI_API_KEY_ENV_VAR {
                            Err(crate::error::CodexErr::EnvVar(EnvVarError {
                                var: env_key.clone(),
                                instructions: self.env_key_instructions.clone(),
                            }))
                        } else {
                            Ok(None)
                        }
                    }
                }
            }
            None => Ok(None),
        }
    }
}

/// Built-in default provider list.
pub fn built_in_model_providers() -> HashMap<String, ModelProviderInfo> {
    use ModelProviderInfo as P;

    // We do not want to be in the business of adjucating which third-party
    // providers are bundled with Codex CLI, so we only include the OpenAI
    // provider by default. Users are encouraged to add to `model_providers`
    // in config.toml to add their own providers.
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
                    .filter(|v| !v.trim().is_empty())
                    .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
                env_key: Some("OPENAI_API_KEY".into()),
                env_key_instructions: Some("Create an API key (https://platform.openai.com) and export it as an environment variable.".into()),
                wire_api: WireApi::Responses,
                query_params: None,
                http_headers: Some(
                    [
                        ("originator".to_string(), OPENAI_ORIGINATOR_HEADER.to_string()),
                        ("version".to_string(), env!("CARGO_PKG_VERSION").to_string()),
                    ]
                        .into_iter()
                        .collect(),
                ),
                env_http_headers: Some(
                    [
                        ("OpenAI-Organization".to_string(), "OPENAI_ORGANIZATION".to_string()),
                        ("OpenAI-Project".to_string(), "OPENAI_PROJECT".to_string()),
                    ]
                        .into_iter()
                        .collect(),
                ),
            },
        ),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]
    use super::*;
    use pretty_assertions::assert_eq;
    use std::env;

    #[test]
    fn test_deserialize_ollama_model_provider_toml() {
        let azure_provider_toml = r#"
name = "Ollama"
base_url = "http://localhost:11434/v1"
        "#;
        let expected_provider = ModelProviderInfo {
            name: "Ollama".into(),
            base_url: "http://localhost:11434/v1".into(),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
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
            base_url: "https://xxxxx.openai.azure.com/openai".into(),
            env_key: Some("AZURE_OPENAI_API_KEY".into()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: Some(maplit::hashmap! {
                "api-version".to_string() => "2025-04-01-preview".to_string(),
            }),
            http_headers: None,
            env_http_headers: None,
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
            base_url: "https://example.com".into(),
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
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_non_openai_provider_works_without_api_key() {
        // Test that non-OpenAI providers work even when their env_key is not set
        let ollama_provider = ModelProviderInfo {
            name: "Ollama".to_string(),
            base_url: "http://localhost:11434/v1".to_string(),
            env_key: Some("NONEXISTENT_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
        };

        // Ensure the env var is not set
        unsafe {
            env::remove_var("NONEXISTENT_API_KEY");
        }

        let client = reqwest::Client::new();
        let result = ollama_provider.create_request_builder(&client);
        
        // Should succeed even without the API key
        assert!(result.is_ok(), "Non-OpenAI provider should work without API key");
    }

    #[test]
    fn test_openai_provider_requires_api_key() {
        // Test that OpenAI provider still requires API key
        let openai_provider = ModelProviderInfo {
            name: "OpenAI".to_string(),
            base_url: "https://api.openai.com/v1".to_string(),
            env_key: Some("OPENAI_API_KEY".to_string()),
            env_key_instructions: Some("Create an API key".to_string()),
            wire_api: WireApi::Responses,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
        };

        // Ensure OPENAI_API_KEY is not set for this test
        let original_key = env::var("OPENAI_API_KEY").ok();
        unsafe {
            env::remove_var("OPENAI_API_KEY");
        }

        let client = reqwest::Client::new();
        let result = openai_provider.create_request_builder(&client);
        
        // Should fail without the API key
        assert!(result.is_err(), "OpenAI provider should require API key");

        // Restore original key if it existed
        if let Some(key) = original_key {
            unsafe {
                env::set_var("OPENAI_API_KEY", key);
            }
        }
    }

    #[test]
    fn test_provider_with_valid_api_key_works() {
        // Test that providers work when API key is provided
        let provider = ModelProviderInfo {
            name: "Test".to_string(),
            base_url: "https://api.test.com/v1".to_string(),
            env_key: Some("TEST_API_KEY".to_string()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
        };

        // Set the API key
        unsafe {
            env::set_var("TEST_API_KEY", "test-key-value");
        }

        let client = reqwest::Client::new();
        let result = provider.create_request_builder(&client);
        
        // Should succeed with the API key
        assert!(result.is_ok(), "Provider should work with valid API key");

        // Clean up
        unsafe {
            env::remove_var("TEST_API_KEY");
        }
    }

    #[test]
    fn test_provider_without_env_key_works() {
        // Test that providers without env_key work
        let provider = ModelProviderInfo {
            name: "Local".to_string(),
            base_url: "http://localhost:8080/v1".to_string(),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            http_headers: None,
            env_http_headers: None,
        };

        let client = reqwest::Client::new();
        let result = provider.create_request_builder(&client);
        
        // Should succeed without any API key requirement
        assert!(result.is_ok(), "Provider without env_key should work");
    }
}
