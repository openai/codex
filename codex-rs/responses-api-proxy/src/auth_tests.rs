//! Ensures authentication diagnostics do not expose credentials.

use super::Auth;

#[test]
fn debug_redacts_api_keys_in_both_formats() {
    let auth = Auth::ApiKey("Bearer test-secret-for-proxy-auth");

    for output in [format!("{auth:?}"), format!("{auth:#?}")] {
        assert!(!output.contains("test-secret-for-proxy-auth"));
        assert!(output.contains("ApiKey"));
        assert!(output.contains("[REDACTED]"));
    }
}
