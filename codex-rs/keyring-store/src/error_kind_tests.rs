//! Tests native cause classification without touching the real keyring.

use super::*;
use pretty_assertions::assert_eq;

#[test]
fn preserves_typed_causes_without_guessing_from_messages() {
    let cases = [
        (
            keyring::Error::NoStorageAccess(Box::new(std::io::Error::other(
                "locked timeout access denied",
            ))),
            ErrorKind::Other,
        ),
        (
            keyring::Error::PlatformFailure(Box::new(std::io::Error::new(
                ErrorKind::TimedOut,
                "private account",
            ))),
            ErrorKind::TimedOut,
        ),
        (
            keyring::Error::PlatformFailure(Box::new(std::sync::Arc::new(std::io::Error::new(
                ErrorKind::PermissionDenied,
                "private D-Bus socket",
            )))),
            ErrorKind::PermissionDenied,
        ),
        (
            keyring::Error::Invalid("private account".into(), "private reason".into()),
            ErrorKind::Other,
        ),
    ];
    for (error, expected) in cases {
        let error = std::io::Error::from(CredentialStoreError::new(error));
        assert_eq!(error.kind(), expected);
        assert!(
            error
                .get_ref()
                .unwrap()
                .downcast_ref::<CredentialStoreError>()
                .is_some()
        );
    }
}

#[test]
fn classifies_native_backend_errors() {
    #[cfg(target_os = "macos")]
    let native: Box<dyn Error + Send + Sync> = Box::new(
        security_framework::base::Error::from_code(/*code*/ -25291),
    );
    #[cfg(target_os = "linux")]
    let native: Box<dyn Error + Send + Sync> = Box::new(secret_service::Error::Locked);
    #[cfg(target_os = "windows")]
    let native: Box<dyn Error + Send + Sync> = Box::new(keyring::windows::Error(1312));
    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    assert_eq!(
        classify(&CredentialStoreError::new(keyring::Error::NoStorageAccess(
            native
        ))),
        if cfg!(target_os = "linux") {
            ErrorKind::WouldBlock
        } else {
            ErrorKind::NotConnected
        },
    );
}

#[cfg(target_os = "linux")]
#[test]
fn unavailable_secret_service_is_not_connected() {
    let error = CredentialStoreError::new(keyring::Error::PlatformFailure(Box::new(
        secret_service::Error::Unavailable,
    )));
    assert_eq!(std::io::Error::from(error).kind(), ErrorKind::NotConnected);
}

#[cfg(target_os = "linux")]
#[test]
fn secret_service_dbus_io_error_preserves_its_kind() {
    let error = secret_service::Error::Zbus(
        std::io::Error::new(ErrorKind::PermissionDenied, "private D-Bus socket").into(),
    );
    let error = CredentialStoreError::new(keyring::Error::PlatformFailure(Box::new(error)));
    assert_eq!(
        std::io::Error::from(error).kind(),
        ErrorKind::PermissionDenied
    );
}

#[cfg(target_os = "macos")]
#[test]
fn read_only_keychain_is_permission_denied() {
    let error = CredentialStoreError::new(keyring::Error::NoStorageAccess(Box::new(
        security_framework::base::Error::from_code(/*code*/ -25292),
    )));
    assert_eq!(
        std::io::Error::from(error).kind(),
        ErrorKind::PermissionDenied
    );
}
