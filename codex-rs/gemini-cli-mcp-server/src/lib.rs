/// Gemini CLI MCP Server library
///
/// Provides OAuth 2.0 + PKCE authentication for Google Gemini API
pub mod oauth;

// Re-export main types
pub use oauth::OAuthConfig;
pub use oauth::OAuthManager;
pub use oauth::OAuthToken;
pub use oauth::PKCEChallenge;
