//! Mock OAuth endpoints for device sign-in tests.

use serde_json::json;
use wiremock::Mock;
use wiremock::MockGuard;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

/// Issues a challenge and answers one token poll. The guard lets a test wait for that poll.
pub async fn mount_device_code_grant(
    server: &MockServer,
    token_response: ResponseTemplate,
) -> MockGuard {
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "secret-device-code", "user_code": "ABCD-EFGH",
            "verification_uri": "https://login.example/device", "expires_in": 60, "interval": 1,
        })))
        .expect(1)
        .mount(server)
        .await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains(
            "grant_type=urn%3Aietf%3Aparams%3Aoauth%3Agrant-type%3Adevice_code",
        ))
        .and(body_string_contains("device_code=secret-device-code"))
        .respond_with(token_response)
        .expect(1)
        .mount_as_scoped(server)
        .await
}
