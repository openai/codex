//! Ensures credential payloads cannot escape through diagnostic error formatting.

use super::*;
use keyring::credential::CredentialApi;
use pretty_assertions::assert_eq;

#[test]
fn payload_bearing_errors_redact_credentials_and_end_the_source_chain() {
    let credential = keyring::mock::MockCredential::default();
    credential.set_password("private-credential-value").unwrap();
    for (error, message) in [
        (
            KeyringError::Ambiguous(vec![Box::new(credential) as Box<keyring::Credential>]),
            "Entry is matched by 1 credentials",
        ),
        (
            KeyringError::BadEncoding(b"private-credential-value".to_vec()),
            "Data is not UTF-8 encoded",
        ),
    ] {
        let error = CredentialStoreError::new(error);
        assert_eq!(
            (
                error.to_string(),
                error.message(),
                format!("{error:?}"),
                error.source().is_none()
            ),
            (
                message.to_string(),
                message.to_string(),
                message.to_string(),
                true
            ),
        );
    }
}

#[test]
fn debug_preserves_native_error_details() {
    let error = CredentialStoreError::new(KeyringError::PlatformFailure(Box::new(
        std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "permission denied by keyring",
        ),
    )));
    assert_eq!(
        format!("{error:?}"),
        "Platform secure storage failure: permission denied by keyring",
    );
}
