//! Azure authentication support for Azure OpenAI endpoints.
//!
//! This module provides support for authenticating with Azure OpenAI using
//! Azure's DefaultAzureCredential chain, which tries multiple authentication
//! methods in order: environment variables, managed identity, Azure CLI, etc.
//!
//! This feature requires the `azure-identity` feature flag to be enabled.

use crate::error::CodexErr;
use crate::error::Result;
use crate::model_provider_info::AzureAuthMode;
use std::sync::Arc;
use tokio::sync::RwLock;

/// The OAuth scope required for Azure OpenAI / Cognitive Services.
pub const AZURE_OPENAI_SCOPE: &str = "https://cognitiveservices.azure.com/.default";

/// Buffer time before token expiry to trigger a refresh (5 minutes).
const TOKEN_REFRESH_BUFFER_SECS: u64 = 300;

/// Cached token with expiration tracking.
struct CachedToken {
    token: String,
    expires_at: std::time::Instant,
}

/// Provider for Azure authentication tokens.
///
/// This wraps Azure's DefaultAzureCredential and provides token caching
/// with automatic refresh before expiry.
pub struct AzureCredentialProvider {
    #[cfg(feature = "azure-identity")]
    credential: Arc<azure_identity::DefaultAzureCredential>,
    cached_token: Arc<RwLock<Option<CachedToken>>>,
    #[allow(dead_code)]
    mode: AzureAuthMode,
}

impl std::fmt::Debug for AzureCredentialProvider {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AzureCredentialProvider")
            .field("mode", &self.mode)
            .finish_non_exhaustive()
    }
}

impl AzureCredentialProvider {
    /// Creates a new Azure credential provider.
    ///
    /// # Errors
    ///
    /// Returns an error if the `azure-identity` feature is not enabled or
    /// if the Azure credential chain cannot be initialized.
    #[cfg(feature = "azure-identity")]
    pub fn new(mode: AzureAuthMode) -> Result<Self> {
        use azure_identity::DefaultAzureCredential;
        use azure_identity::TokenCredentialOptions;

        let credential = DefaultAzureCredential::create(TokenCredentialOptions::default())
            .map_err(|e| CodexErr::AzureAuth(format!("failed to create credential: {e}")))?;

        Ok(Self {
            credential: Arc::new(credential),
            cached_token: Arc::new(RwLock::new(None)),
            mode,
        })
    }

    /// Creates a new Azure credential provider.
    ///
    /// # Errors
    ///
    /// Returns an error because the `azure-identity` feature is not enabled.
    #[cfg(not(feature = "azure-identity"))]
    pub fn new(_mode: AzureAuthMode) -> Result<Self> {
        Err(CodexErr::AzureAuth(
            "Azure authentication requires the 'azure-identity' feature to be enabled. \
             Rebuild with: cargo build --features azure-identity"
                .to_string(),
        ))
    }

    /// Ensures a valid token is available, refreshing if necessary.
    ///
    /// This should be called before each API request to ensure the token
    /// is fresh. The token is cached and only refreshed when close to expiry.
    ///
    /// # Errors
    ///
    /// Returns an error if the token cannot be obtained from Azure.
    #[cfg(feature = "azure-identity")]
    pub async fn ensure_token(&self) -> Result<String> {
        use azure_core::auth::TokenCredential;

        // Check if we have a valid cached token
        {
            let cache = self.cached_token.read().await;
            if let Some(cached) = cache.as_ref() {
                let now = std::time::Instant::now();
                let buffer = std::time::Duration::from_secs(TOKEN_REFRESH_BUFFER_SECS);
                if now + buffer < cached.expires_at {
                    return Ok(cached.token.clone());
                }
            }
        }

        // Need to refresh the token
        let token_response = self
            .credential
            .get_token(&[AZURE_OPENAI_SCOPE])
            .await
            .map_err(|e| CodexErr::AzureAuth(format!("failed to get token: {e}")))?;

        let token = token_response.token.secret().to_string();
        // Calculate time until expiry
        let now = time::OffsetDateTime::now_utc();
        let duration_until_expiry = token_response.expires_on - now;
        let expires_at = std::time::Instant::now()
            + std::time::Duration::from_secs(
                duration_until_expiry.whole_seconds().max(0) as u64,
            );

        // Cache the new token
        {
            let mut cache = self.cached_token.write().await;
            *cache = Some(CachedToken {
                token: token.clone(),
                expires_at,
            });
        }

        Ok(token)
    }

    /// Ensures a valid token is available, refreshing if necessary.
    ///
    /// # Errors
    ///
    /// Returns an error because the `azure-identity` feature is not enabled.
    #[cfg(not(feature = "azure-identity"))]
    pub async fn ensure_token(&self) -> Result<String> {
        Err(CodexErr::AzureAuth(
            "Azure authentication requires the 'azure-identity' feature".to_string(),
        ))
    }

    /// Returns the cached bearer token if available and not expired.
    ///
    /// This is a synchronous method for use in contexts where async is not
    /// available. Callers should call `ensure_token()` before the request
    /// to guarantee a valid token is cached.
    pub fn cached_bearer_token(&self) -> Option<String> {
        // Use try_read to avoid blocking; if we can't get the lock, return None
        // and let the caller handle it (they should have called ensure_token first)
        let cache = self.cached_token.try_read().ok()?;
        let cached = cache.as_ref()?;

        let now = std::time::Instant::now();
        if now < cached.expires_at {
            Some(cached.token.clone())
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_azure_auth_mode_default() {
        let mode = AzureAuthMode::default();
        assert_eq!(mode, AzureAuthMode::DefaultCredential);
    }

    #[test]
    fn test_azure_auth_mode_serde() {
        let json = r#""default_credential""#;
        let mode: AzureAuthMode = serde_json::from_str(json).expect("deserialize");
        assert_eq!(mode, AzureAuthMode::DefaultCredential);

        let serialized = serde_json::to_string(&mode).expect("serialize");
        assert_eq!(serialized, json);
    }

    #[cfg(not(feature = "azure-identity"))]
    #[test]
    fn test_azure_provider_fails_without_feature() {
        let result = AzureCredentialProvider::new(AzureAuthMode::DefaultCredential);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, CodexErr::AzureAuth(_)));
    }
}
