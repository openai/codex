//! Device credentials and signing, independent of RPC routing, UI, and backend registration.

mod credential;
mod error;
mod guard;
mod key_namespace;
#[cfg(any(target_os = "macos", test))]
mod lifecycle_lock;
#[cfg(any(target_os = "macos", test))]
mod native_operation;
#[cfg(any(target_os = "macos", test))]
mod platform_macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

pub use credential::UserVerificationBiometricType;
pub use credential::UserVerificationKeyCreation;
pub use credential::UserVerificationKeyDeletion;
pub use credential::UserVerificationKeyInfo;
pub use credential::UserVerificationProof;
pub use credential::UserVerificationRequest;
pub use credential::UserVerificationStatus;
pub use error::UserVerificationCancellationReason;
pub use error::UserVerificationError;
pub use error::UserVerificationFailureReason;
pub use error::UserVerificationUnavailableReason;
pub use guard::UserVerificationRequestGuard;
pub use key_namespace::UserVerificationKeyNamespace;
use std::sync::Arc;

/// Performs local credential operations for one captured account-user identity.
/// Implementations never perform network registration. Blocking implementations must run off
/// the async executor and check the guard after waiting, before effects, and before returning.
pub trait UserVerificationProvider: Send + Sync {
    /// Reads local readiness without creating credentials or prompting for authentication.
    fn status(
        &self,
        guard: &UserVerificationRequestGuard,
    ) -> Result<UserVerificationStatus, UserVerificationError>;

    /// Creates a protected key only if none exists. Success does not mean server enrollment.
    fn ensure_key(
        &self,
        guard: &UserVerificationRequestGuard,
    ) -> Result<UserVerificationKeyCreation, UserVerificationError>;

    /// Removes the local key idempotently. Backend revocation belongs to the caller.
    fn delete(
        &self,
        guard: &UserVerificationRequestGuard,
    ) -> Result<UserVerificationKeyDeletion, UserVerificationError>;

    /// Authenticates and signs 1–4096 challenge bytes without interpreting an elicitation.
    /// The caller owns approval UI, request correlation, and the captured identity check.
    fn verify(
        &self,
        request: &UserVerificationRequest,
        guard: &UserVerificationRequestGuard,
    ) -> Result<UserVerificationProof, UserVerificationError>;
}

/// Reports whether this build contains a native provider, independently of local readiness.
pub fn platform_supported() -> bool {
    cfg!(target_os = "macos")
}

/// Probes biometric hardware without reading credentials, prompting, or checking enrollment.
/// This performs local OS work; async callers should run it off their executor during setup.
pub fn device_supported() -> bool {
    #[cfg(target_os = "macos")]
    {
        platform_macos::device_supported()
    }
    #[cfg(not(target_os = "macos"))]
    {
        false
    }
}

pub fn platform_provider(
    namespace: UserVerificationKeyNamespace,
) -> Arc<dyn UserVerificationProvider> {
    #[cfg(target_os = "macos")]
    {
        Arc::new(platform_macos::NativeProvider { namespace })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = namespace.label;
        Arc::new(unsupported::UnsupportedProvider)
    }
}
