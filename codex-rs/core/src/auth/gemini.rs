//! Gemini authentication provider supporting API Key and OAuth 2.0 modes

use anyhow::{Result, anyhow};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Gemini authentication credential source
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CredentialSource {
    /// API Key from Google AI Studio
    ApiKey,
    /// OAuth 2.0 via Google Account (Vertex AI)
    OAuth,
}

/// Gemini provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GeminiProvider {
    /// Google AI Studio (Generative Language API)
    AiStudio,
    /// Vertex AI
    Vertex,
}

/// Gemini authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct GeminiAuthConfig {
    /// Authentication mode
    pub mode: CredentialSource,
    /// Provider type
    pub provider: GeminiProvider,
    /// Prefer geminicli for OAuth flow
    pub prefer_cli: bool,
    /// Google Cloud Project ID (required for Vertex AI)
    pub project: Option<String>,
    /// Vertex AI region (required for Vertex AI)
    pub region: Option<String>,
}

impl Default for GeminiAuthConfig {
    fn default() -> Self {
        Self {
            mode: CredentialSource::ApiKey,
            provider: GeminiProvider::AiStudio,
            prefer_cli: true,
            project: None,
            region: None,
        }
    }
}

/// Gemini OAuth token data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeminiOAuthTokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    pub expires_at: Option<i64>,
    pub token_type: String,
}

/// Gemini authentication provider
pub struct GeminiAuthProvider {
    config: GeminiAuthConfig,
}

impl GeminiAuthProvider {
    /// Create a new Gemini auth provider
    pub fn new(config: GeminiAuthConfig) -> Self {
        Self { config }
    }

    /// Get API key from environment or config
    pub fn get_api_key(&self) -> Result<String> {
        // Check environment variables in priority order
        if let Ok(key) = std::env::var("GEMINI_API_KEY") {
            return Ok(key);
        }
        
        if let Ok(key) = std::env::var("GOOGLE_AI_STUDIO_API_KEY") {
            return Ok(key);
        }

        Err(anyhow!(
            "Gemini API key not found. Set GEMINI_API_KEY or GOOGLE_AI_STUDIO_API_KEY environment variable"
        ))
    }

    /// Check if geminicli is installed
    pub fn is_geminicli_installed(&self) -> bool {
        which::which("geminicli").is_ok()
    }

    /// Get OAuth configuration from environment
    pub fn get_oauth_config(&self) -> Result<OAuthConfig> {
        let client_id = std::env::var("GOOGLE_OAUTH_CLIENT_ID")
            .map_err(|_| anyhow!("GOOGLE_OAUTH_CLIENT_ID not set"))?;

        let project = self.config.project.clone()
            .or_else(|| std::env::var("GCP_PROJECT_ID").ok())
            .ok_or_else(|| anyhow!("GCP project ID required for OAuth"))?;

        let region = self.config.region.clone()
            .or_else(|| std::env::var("VERTEX_REGION").ok())
            .unwrap_or_else(|| "us-central1".to_string());

        Ok(OAuthConfig {
            client_id,
            project,
            region,
        })
    }

    /// Get authentication method to use
    pub fn get_auth_method(&self) -> AuthMethod {
        match self.config.mode {
            CredentialSource::ApiKey => AuthMethod::ApiKey,
            CredentialSource::OAuth => {
                if self.config.prefer_cli && self.is_geminicli_installed() {
                    AuthMethod::OAuthGeminiCli
                } else {
                    AuthMethod::OAuthInternal
                }
            }
        }
    }
}

/// OAuth configuration
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub project: String,
    pub region: String,
}

/// Authentication method to use
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthMethod {
    /// Use API key
    ApiKey,
    /// Use geminicli for OAuth
    OAuthGeminiCli,
    /// Use internal PKCE flow
    OAuthInternal,
}

/// Load Gemini auth config from environment and config file
pub fn load_gemini_config(_codex_home: &Path) -> GeminiAuthConfig {
    // For now, return default config
    // In full implementation, this would read from .codex/config.toml
    let mut config = GeminiAuthConfig::default();

    // Override from environment
    if std::env::var("GEMINI_API_KEY").is_ok() 
        || std::env::var("GOOGLE_AI_STUDIO_API_KEY").is_ok() {
        config.mode = CredentialSource::ApiKey;
    }

    if std::env::var("GOOGLE_OAUTH_CLIENT_ID").is_ok() {
        config.mode = CredentialSource::OAuth;
    }

    config
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = GeminiAuthConfig::default();
        assert_eq!(config.mode, CredentialSource::ApiKey);
        assert_eq!(config.provider, GeminiProvider::AiStudio);
        assert!(config.prefer_cli);
    }

    #[test]
    fn test_auth_method_selection() {
        let config = GeminiAuthConfig::default();
        let provider = GeminiAuthProvider::new(config);
        
        // Should default to API key mode
        assert_eq!(provider.get_auth_method(), AuthMethod::ApiKey);
    }
}
