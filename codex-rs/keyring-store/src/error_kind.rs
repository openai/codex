//! Maps reliable native failures to portable IO kinds without examining error messages.
//! Ambiguous keyring errors stay Other: NoStorageAccess alone does not prove a locked store.

use crate::CredentialStoreError;
use std::error::Error;
use std::io::ErrorKind;

pub(super) fn classify(error: &CredentialStoreError) -> ErrorKind {
    let mut cause: Option<&(dyn Error + 'static)> = Some(error);
    while let Some(error) = cause {
        if let Some(error) = error.downcast_ref::<std::io::Error>() {
            return error.kind();
        }
        if let Some(error) = error.downcast_ref::<std::sync::Arc<std::io::Error>>() {
            return error.kind();
        }
        #[cfg(target_os = "macos")]
        if let Some(error) = error.downcast_ref::<security_framework::base::Error>() {
            return match error.code() {
                -25291 => ErrorKind::NotConnected, // errSecNotAvailable
                -25292 | -25293 => ErrorKind::PermissionDenied, // errSecReadOnly / errSecAuthFailed
                -4 => ErrorKind::Unsupported,      // errSecUnimplemented
                _ => ErrorKind::Other,
            };
        }
        #[cfg(target_os = "linux")]
        if let Some(error) = error.downcast_ref::<secret_service::Error>() {
            match error {
                secret_service::Error::Locked => return ErrorKind::WouldBlock,
                secret_service::Error::Unavailable => return ErrorKind::NotConnected,
                _ => {}
            }
        }
        #[cfg(target_os = "windows")]
        if let Some(error) = error.downcast_ref::<keyring::windows::Error>() {
            return match error.0 {
                1312 => ErrorKind::NotConnected,  // ERROR_NO_SUCH_LOGON_SESSION
                5 => ErrorKind::PermissionDenied, // ERROR_ACCESS_DENIED
                50 => ErrorKind::Unsupported,     // ERROR_NOT_SUPPORTED
                1460 => ErrorKind::TimedOut,      // ERROR_TIMEOUT
                _ => ErrorKind::Other,
            };
        }
        cause = error.source();
    }
    ErrorKind::Other
}

#[cfg(test)]
#[path = "error_kind_tests.rs"]
mod tests;
