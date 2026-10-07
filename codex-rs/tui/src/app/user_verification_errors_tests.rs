use super::*;
use codex_app_server_protocol::JSONRPCErrorError;
use pretty_assertions::assert_eq;

#[test]
fn user_verification_errors_use_typed_reasons_without_raw_provider_diagnostics() {
    let error = TypedRequestError::Server {
        method: "userVerification/verify".to_string(),
        source: JSONRPCErrorError {
            code: -32603,
            message: "private native provider diagnostics".to_string(),
            data: Some(serde_json::json!({ "type": "unavailable", "reason": "credentialMissing" })),
        },
    };
    assert_eq!(
        verification_failure(&error).message,
        "No user-verification credential is available in the local Codex binary."
    );
    let malformed = TypedRequestError::Server {
        method: "userVerification/verify".to_string(),
        source: JSONRPCErrorError {
            code: -32603,
            message: "private native provider diagnostics".to_string(),
            data: Some(
                serde_json::json!({ "type": "unavailable", "reason": "secret native failure" }),
            ),
        },
    };
    assert_eq!(
        verification_failure(&malformed).message,
        "The local Codex binary could not complete user verification."
    );
}

#[test]
fn unavailable_reasons_use_device_and_session_neutral_copy() {
    for (reason, expected) in [
        (
            "biometricsUnavailable",
            "Biometric verification is currently unavailable on this device.",
        ),
        (
            "providerUnavailable",
            "User verification is unavailable for this request.",
        ),
    ] {
        let error = TypedRequestError::Server {
            method: "userVerification/verify".to_string(),
            source: JSONRPCErrorError {
                code: -32603,
                message: "private native provider diagnostics".to_string(),
                data: Some(serde_json::json!({ "type": "unavailable", "reason": reason })),
            },
        };
        assert_eq!(verification_failure(&error).message, expected);
    }
}
