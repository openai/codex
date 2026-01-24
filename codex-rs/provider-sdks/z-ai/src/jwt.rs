//! JWT token generation with caching for Z.AI API authentication.
//!
//! Replicates the Python SDK's `_jwt_token.py` behavior exactly.

use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

use jsonwebtoken::Algorithm;
use jsonwebtoken::EncodingKey;
use jsonwebtoken::Header;
use serde::Serialize;
use tokio::sync::RwLock;

use crate::error::Result;
use crate::error::ZaiError;

/// Cache TTL: 3 minutes (same as Python SDK).
const CACHE_TTL: Duration = Duration::from_secs(3 * 60);

/// Token validity period: cache TTL + 30 seconds = 210000ms.
const API_TOKEN_TTL_MS: i64 = (3 * 60 + 30) * 1000;

/// JWT claims for Z.AI API.
#[derive(Debug, Serialize)]
struct JwtClaims {
    api_key: String,
    exp: i64,
    timestamp: i64,
}

/// Cached token with creation time.
#[derive(Debug)]
struct CachedToken {
    token: String,
    created_at: Instant,
}

/// JWT token cache with automatic refresh.
#[derive(Debug)]
pub struct JwtTokenCache {
    cache: RwLock<Option<CachedToken>>,
    api_key_id: String,
    secret: String,
}

impl JwtTokenCache {
    /// Create a new JWT token cache from API key.
    ///
    /// API key format: "{api_key_id}.{secret}"
    pub fn new(api_key: &str) -> Result<Self> {
        let parts: Vec<&str> = api_key.split('.').collect();
        if parts.len() != 2 {
            return Err(ZaiError::Configuration(
                "Invalid API key format, expected 'api_key_id.secret'".into(),
            ));
        }
        Ok(Self {
            cache: RwLock::new(None),
            api_key_id: parts[0].to_string(),
            secret: parts[1].to_string(),
        })
    }

    /// Generate or retrieve cached JWT token.
    pub async fn get_token(&self) -> Result<String> {
        // Check cache first
        {
            let cache = self.cache.read().await;
            if let Some(ref cached) = *cache {
                if cached.created_at.elapsed() < CACHE_TTL {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Generate new token
        let token = self.generate_token()?;

        // Update cache
        {
            let mut cache = self.cache.write().await;
            *cache = Some(CachedToken {
                token: token.clone(),
                created_at: Instant::now(),
            });
        }

        Ok(token)
    }

    fn generate_token(&self) -> Result<String> {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ZaiError::JwtError(e.to_string()))?
            .as_millis() as i64;

        let claims = JwtClaims {
            api_key: self.api_key_id.clone(),
            exp: now_ms + API_TOKEN_TTL_MS,
            timestamp: now_ms,
        };

        // Create header with custom sign_type field (matches Python SDK)
        let mut header = Header::new(Algorithm::HS256);
        header.typ = Some("JWT".into());

        let encoding_key = EncodingKey::from_secret(self.secret.as_bytes());

        // Note: jsonwebtoken crate doesn't support custom header fields directly.
        // We need to use a workaround by manually constructing the header.
        encode_with_sign_type(&header, &claims, &encoding_key)
    }
}

/// Encode JWT with custom sign_type header field.
fn encode_with_sign_type<T: Serialize>(
    header: &Header,
    claims: &T,
    key: &EncodingKey,
) -> Result<String> {
    // Create custom header JSON with sign_type
    let header_json = serde_json::json!({
        "alg": "HS256",
        "typ": "JWT",
        "sign_type": "SIGN"
    });

    let header_b64 = base64_url_encode(&serde_json::to_vec(&header_json)?);
    let claims_b64 = base64_url_encode(&serde_json::to_vec(claims)?);

    let message = format!("{header_b64}.{claims_b64}");

    // Sign with HMAC-SHA256
    use jsonwebtoken::crypto::sign;
    let signature =
        sign(message.as_bytes(), key, header.alg).map_err(|e| ZaiError::JwtError(e.to_string()))?;

    Ok(format!("{message}.{signature}"))
}

/// Base64 URL-safe encoding without padding.
fn base64_url_encode(data: &[u8]) -> String {
    use base64::Engine;
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    URL_SAFE_NO_PAD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_cache_creation_valid_key() {
        let cache = JwtTokenCache::new("api_key_id.secret");
        assert!(cache.is_ok());
    }

    #[test]
    fn test_jwt_cache_creation_invalid_key() {
        let cache = JwtTokenCache::new("invalid_key");
        assert!(cache.is_err());
    }

    #[tokio::test]
    async fn test_jwt_token_generation() {
        let cache = JwtTokenCache::new("test_id.test_secret").expect("valid key");
        let token = cache.get_token().await;
        assert!(token.is_ok());

        let token = token.expect("token");
        // JWT has 3 parts separated by dots
        assert_eq!(token.split('.').count(), 3);
    }

    #[tokio::test]
    async fn test_jwt_token_caching() {
        let cache = JwtTokenCache::new("test_id.test_secret").expect("valid key");

        let token1 = cache.get_token().await.expect("token1");
        let token2 = cache.get_token().await.expect("token2");

        // Tokens should be the same (cached)
        assert_eq!(token1, token2);
    }
}
