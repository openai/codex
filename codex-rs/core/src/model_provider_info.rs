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

    /// Optional custom headers to include in API requests.
    pub custom_headers: Option<HashMap<String, String>>,
}

impl ModelProviderInfo {
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
}

impl ModelProviderInfo {
    /// If `env_key` is Some, returns the API key for this provider if present
    /// (and non-empty) in the environment. If `env_key` is required but
    /// cannot be found, returns an error.
    pub fn api_key(&self) -> crate::error::Result<Option<String>> {
        match &self.env_key {
            Some(env_key) => {
                let env_value = if env_key == crate::openai_api_key::OPENAI_API_KEY_ENV_VAR {
                    get_openai_api_key().map_or_else(|| Err(VarError::NotPresent), Ok)
                } else {
                    std::env::var(env_key)
                };
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

    /// Returns custom headers from both provider config and provider-specific env var.
    pub fn get_custom_headers(&self, provider_name: &str) -> HashMap<String, String> {
        let mut headers = HashMap::new();
        
        // Add headers from provider config
        if let Some(provider_headers) = &self.custom_headers {
            headers.extend(provider_headers.clone());
        }
        
        // Add headers from provider-specific environment variable (e.g., OPENAI_CUSTOM_HEADERS)
        let env_var_name = format!("{}_CUSTOM_HEADERS", provider_name.to_uppercase());
        if let Ok(env_headers) = std::env::var(&env_var_name) {
            for line in env_headers.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    if !key.is_empty() {
                        headers.insert(key.to_string(), value.to_string());
                    }
                }
            }
        }
        
        headers
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
                base_url: "https://api.openai.com/v1".into(),
                env_key: Some("OPENAI_API_KEY".into()),
                env_key_instructions: Some("Create an API key (https://platform.openai.com) and export it as an environment variable.".into()),
                wire_api: WireApi::Responses,
                query_params: None,
                custom_headers: None,
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
            custom_headers: None,
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
            custom_headers: None,
        };

        let provider: ModelProviderInfo = toml::from_str(azure_provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }

    #[test]
    fn test_custom_headers_from_config() {
        // Ensure no env var interference
        unsafe {
            std::env::remove_var("TEST_CUSTOM_HEADERS");
        }
        
        let provider = ModelProviderInfo {
            name: "Test Provider".into(),
            base_url: "https://api.example.com".into(),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            custom_headers: Some(maplit::hashmap! {
                "X-Custom-Header".to_string() => "static-value".to_string(),
                "X-Provider-Version".to_string() => "1.0.0".to_string(),
            }),
        };

        let headers = provider.get_custom_headers("test");
        
        assert_eq!(headers.get("X-Custom-Header"), Some(&"static-value".to_string()));
        assert_eq!(headers.get("X-Provider-Version"), Some(&"1.0.0".to_string()));
    }

    #[test]
    fn test_provider_specific_custom_headers_env_var() {
        // Set up environment variables for testing
        unsafe {
            std::env::set_var("OPENAI_CUSTOM_HEADERS", "X-Custom-Auth: Bearer token123\nX-App-Version: 2.0.0\nX-Extra: some value");
        }
        
        let provider = ModelProviderInfo {
            name: "Test Provider".into(),
            base_url: "https://api.example.com".into(),
            env_key: None,
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            custom_headers: Some(maplit::hashmap! {
                "X-Provider-Header".to_string() => "provider-value".to_string(),
            }),
        };

        let headers = provider.get_custom_headers("openai");
        
        // Headers from provider config should be present
        assert_eq!(headers.get("X-Provider-Header"), Some(&"provider-value".to_string()));
        
        // Headers from OPENAI_CUSTOM_HEADERS should be present
        assert_eq!(headers.get("X-Custom-Auth"), Some(&"Bearer token123".to_string()));
        assert_eq!(headers.get("X-App-Version"), Some(&"2.0.0".to_string()));
        assert_eq!(headers.get("X-Extra"), Some(&"some value".to_string()));
        
        // Clean up
        unsafe {
            std::env::remove_var("OPENAI_CUSTOM_HEADERS");
        }
    }

    #[test]
    fn test_deserialize_custom_headers_toml() {
        let provider_toml = r#"
name = "Custom Provider"
base_url = "https://api.example.com/v1"
env_key = "CUSTOM_API_KEY"
custom_headers = { "X-Custom-Auth" = "Bearer token", "X-App-Version" = "1.0.0" }
        "#;
        
        let expected_provider = ModelProviderInfo {
            name: "Custom Provider".into(),
            base_url: "https://api.example.com/v1".into(),
            env_key: Some("CUSTOM_API_KEY".into()),
            env_key_instructions: None,
            wire_api: WireApi::Chat,
            query_params: None,
            custom_headers: Some(maplit::hashmap! {
                "X-Custom-Auth".to_string() => "Bearer token".to_string(),
                "X-App-Version".to_string() => "1.0.0".to_string(),
            }),
        };

        let provider: ModelProviderInfo = toml::from_str(provider_toml).unwrap();
        assert_eq!(expected_provider, provider);
    }
}
