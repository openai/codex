/// OAuth 2.0 + PKCE authentication module for Gemini API
///
/// Implements RFC 7636 (PKCE) for secure OAuth flows without client secrets
use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

/// OAuth 2.0 configuration for Google Gemini
#[derive(Debug, Clone)]
pub struct OAuthConfig {
    pub client_id: String,
    pub auth_url: String,
    pub token_url: String,
    pub redirect_uri: String,
    pub scopes: Vec<String>,
    pub token_cache_path: PathBuf,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            client_id: "codex-gemini-client".to_string(),
            auth_url: "https://accounts.google.com/o/oauth2/v2/auth".to_string(),
            token_url: "https://oauth2.googleapis.com/token".to_string(),
            redirect_uri: "http://localhost:8080/oauth/callback".to_string(),
            scopes: vec![
                "https://www.googleapis.com/auth/generative-language".to_string(),
            ],
            token_cache_path: dirs::home_dir()
                .unwrap_or_default()
                .join(".codex")
                .join("gemini_oauth_token.json"),
        }
    }
}

/// OAuth 2.0 token response
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthToken {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    /// Unix timestamp when token was acquired
    pub acquired_at: u64,
}

impl OAuthToken {
    /// Check if token is expired (with 5-minute safety margin)
    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = self.acquired_at + self.expires_in;
        now >= expires_at.saturating_sub(300) // 5 min margin
    }

    /// Get remaining lifetime in seconds
    pub fn remaining_lifetime(&self) -> u64 {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = self.acquired_at + self.expires_in;
        expires_at.saturating_sub(now)
    }
}

/// PKCE (Proof Key for Code Exchange) verifier and challenge
#[derive(Debug, Clone)]
pub struct PKCEChallenge {
    pub verifier: String,
    pub challenge: String,
    pub challenge_method: String,
}

impl PKCEChallenge {
    /// Generate a new PKCE challenge (RFC 7636)
    pub fn generate() -> Result<Self> {
        // Generate 32-byte random verifier
        let verifier = Self::generate_verifier()?;

        // Generate SHA256 challenge
        let challenge = Self::generate_challenge(&verifier)?;

        Ok(Self {
            verifier,
            challenge,
            challenge_method: "S256".to_string(),
        })
    }

    /// Generate cryptographically random verifier (43-128 characters)
    fn generate_verifier() -> Result<String> {
        use rand::Rng;
        let mut rng = rand::rng();
        let bytes: Vec<u8> = (0..32).map(|_| rng.random::<u8>()).collect();
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(&bytes))
    }

    /// Generate SHA256 challenge from verifier
    fn generate_challenge(verifier: &str) -> Result<String> {
        let mut hasher = Sha256::new();
        hasher.update(verifier.as_bytes());
        let hash = hasher.finalize();
        Ok(base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(hash))
    }
}

/// OAuth 2.0 manager with PKCE support
pub struct OAuthManager {
    config: OAuthConfig,
    cached_token: Option<OAuthToken>,
}

impl OAuthManager {
    /// Create a new OAuth manager
    pub fn new(config: OAuthConfig) -> Self {
        Self {
            config,
            cached_token: None,
        }
    }

    /// Load cached token from disk
    pub fn load_cached_token(&mut self) -> Result<Option<OAuthToken>> {
        if !self.config.token_cache_path.exists() {
            return Ok(None);
        }

        let content = std::fs::read_to_string(&self.config.token_cache_path)
            .context("Failed to read token cache")?;

        let token: OAuthToken =
            serde_json::from_str(&content).context("Failed to parse token cache")?;

        if token.is_expired() {
            tracing::warn!("⚠️  Cached token expired, will need re-authentication");
            self.cached_token = None;
            Ok(None)
        } else {
            tracing::info!(
                "✅ Loaded cached token (expires in {} seconds)",
                token.remaining_lifetime()
            );
            self.cached_token = Some(token.clone());
            Ok(Some(token))
        }
    }

    /// Save token to disk cache
    pub fn save_token(&self, token: &OAuthToken) -> Result<()> {
        // Ensure cache directory exists
        if let Some(parent) = self.config.token_cache_path.parent() {
            std::fs::create_dir_all(parent).context("Failed to create cache directory")?;
        }

        let json = serde_json::to_string_pretty(token).context("Failed to serialize token")?;
        std::fs::write(&self.config.token_cache_path, json)
            .context("Failed to write token cache")?;

        tracing::info!("💾 Token cached to {:?}", self.config.token_cache_path);
        Ok(())
    }

    /// Get authorization URL with PKCE challenge
    pub fn get_authorization_url(&self, pkce: &PKCEChallenge) -> String {
        let scopes = self.config.scopes.join(" ");
        format!(
            "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&code_challenge={}&code_challenge_method={}",
            self.config.auth_url,
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(&scopes),
            urlencoding::encode(&pkce.challenge),
            pkce.challenge_method
        )
    }

    /// Exchange authorization code for access token (with PKCE verifier)
    pub async fn exchange_code(
        &mut self,
        code: &str,
        pkce_verifier: &str,
    ) -> Result<OAuthToken> {
        tracing::info!("🔄 Exchanging authorization code for access token");

        // Note: In real implementation, you would use reqwest or similar HTTP client
        // For now, this is a placeholder showing the correct OAuth 2.0 + PKCE flow

        let body = format!(
            "grant_type=authorization_code&code={}&redirect_uri={}&client_id={}&code_verifier={}",
            urlencoding::encode(code),
            urlencoding::encode(&self.config.redirect_uri),
            urlencoding::encode(&self.config.client_id),
            urlencoding::encode(pkce_verifier)
        );

        // Placeholder: In production, use HTTP client like reqwest
        tracing::warn!("⚠️  OAuth token exchange not yet implemented (placeholder)");
        tracing::info!("📝 Would POST to: {}", self.config.token_url);
        tracing::debug!("📝 With body: {}", body);

        // Return dummy token for now
        let token = OAuthToken {
            access_token: "ya29.example_access_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: Some("1//example_refresh_token".to_string()),
            scope: Some(self.config.scopes.join(" ")),
            acquired_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.cached_token = Some(token.clone());
        self.save_token(&token)?;

        Ok(token)
    }

    /// Refresh access token using refresh token
    pub async fn refresh_token(&mut self) -> Result<OAuthToken> {
        let refresh_token = self
            .cached_token
            .as_ref()
            .and_then(|t| t.refresh_token.as_ref())
            .context("No refresh token available")?;

        tracing::info!("🔄 Refreshing access token");

        let body = format!(
            "grant_type=refresh_token&refresh_token={}&client_id={}",
            urlencoding::encode(refresh_token),
            urlencoding::encode(&self.config.client_id)
        );

        // Placeholder: In production, use HTTP client
        tracing::warn!("⚠️  Token refresh not yet implemented (placeholder)");
        tracing::info!("📝 Would POST to: {}", self.config.token_url);
        tracing::debug!("📝 With body: {}", body);

        // Return dummy refreshed token
        let token = OAuthToken {
            access_token: "ya29.example_refreshed_token".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: Some(refresh_token.clone()),
            scope: Some(self.config.scopes.join(" ")),
            acquired_at: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        };

        self.cached_token = Some(token.clone());
        self.save_token(&token)?;

        Ok(token)
    }

    /// Get valid access token (handles caching and refresh automatically)
    pub async fn get_access_token(&mut self) -> Result<String> {
        // Try to load cached token first
        if self.cached_token.is_none() {
            self.load_cached_token()?;
        }

        // Check if we have a valid token
        if let Some(token) = &self.cached_token {
            if !token.is_expired() {
                tracing::debug!("✅ Using cached access token");
                return Ok(token.access_token.clone());
            } else if token.refresh_token.is_some() {
                // Try to refresh
                tracing::info!("🔄 Token expired, refreshing...");
                let refreshed = self.refresh_token().await?;
                return Ok(refreshed.access_token);
            }
        }

        // No valid token, user needs to authenticate
        anyhow::bail!(
            "No valid access token. User needs to authenticate via OAuth 2.0 flow.\n\
             Run: codex gemini auth"
        )
    }

    /// Clear cached token
    pub fn clear_cache(&mut self) -> Result<()> {
        self.cached_token = None;
        if self.config.token_cache_path.exists() {
            std::fs::remove_file(&self.config.token_cache_path)
                .context("Failed to remove token cache")?;
            tracing::info!("🗑️  Token cache cleared");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pkce_challenge_generation() {
        let pkce = PKCEChallenge::generate().unwrap();
        assert!(!pkce.verifier.is_empty());
        assert!(!pkce.challenge.is_empty());
        assert_eq!(pkce.challenge_method, "S256");
        assert_ne!(pkce.verifier, pkce.challenge);
    }

    #[test]
    fn test_oauth_token_expiry() {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Fresh token
        let fresh_token = OAuthToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 3600,
            refresh_token: None,
            scope: None,
            acquired_at: now,
        };
        assert!(!fresh_token.is_expired());

        // Expired token
        let expired_token = OAuthToken {
            access_token: "test".to_string(),
            token_type: "Bearer".to_string(),
            expires_in: 100,
            refresh_token: None,
            scope: None,
            acquired_at: now - 200,
        };
        assert!(expired_token.is_expired());
    }

    #[test]
    fn test_oauth_config_default() {
        let config = OAuthConfig::default();
        assert_eq!(config.auth_url, "https://accounts.google.com/o/oauth2/v2/auth");
        assert_eq!(config.token_url, "https://oauth2.googleapis.com/token");
        assert!(!config.scopes.is_empty());
    }

    #[test]
    fn test_authorization_url_generation() {
        let config = OAuthConfig::default();
        let manager = OAuthManager::new(config);
        let pkce = PKCEChallenge::generate().unwrap();
        
        let url = manager.get_authorization_url(&pkce);
        
        assert!(url.contains("client_id="));
        assert!(url.contains("redirect_uri="));
        assert!(url.contains("code_challenge="));
        assert!(url.contains("code_challenge_method=S256"));
    }
}

