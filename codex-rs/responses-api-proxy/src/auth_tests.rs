//! Credential-safe diagnostics and transport fixtures for the production token-client policy.

use super::Auth;
use super::TokenClient;

pub(crate) fn direct_token_client() -> TokenClient {
    TokenClient(TokenClient::builder().build_direct().unwrap())
}

#[test]
fn debug_redacts_api_keys_in_both_formats() {
    let auth = Auth::ApiKey("Bearer test-secret-for-proxy-auth");

    for output in [format!("{auth:?}"), format!("{auth:#?}")] {
        assert!(!output.contains("test-secret-for-proxy-auth"));
        assert!(output.contains("ApiKey"));
        assert!(output.contains("[REDACTED]"));
    }
}
