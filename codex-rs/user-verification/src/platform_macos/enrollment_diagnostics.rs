//! Collects content-free Apple error and biometric readiness details after local enrollment fails.

use super::provider::noninteractive_context;
use core_foundation::base::CFType;
use core_foundation::base::TCFType as _;
use core_foundation::dictionary::CFDictionary;
use core_foundation::error::CFError;
use core_foundation::error::CFErrorCopyUserInfo;
use core_foundation::error::kCFErrorUnderlyingErrorKey;
use core_foundation::string::CFString;
use objc2_local_authentication::LAPolicy;
use security_framework::base::Error as SecurityError;

pub(super) enum EnrollmentError<'a> {
    Status(SecurityError),
    Creation(&'a CFError),
}

pub(super) fn log_enrollment_failure(stage: &'static str, error: Option<EnrollmentError<'_>>) {
    let (native_error, underlying) = match error {
        Some(EnrollmentError::Status(error)) => (
            Some(("NSOSStatusErrorDomain", i64::from(error.code()))),
            None,
        ),
        Some(EnrollmentError::Creation(error)) => {
            (Some(diagnostic_code(error)), underlying_error(error))
        }
        None => (None, None),
    };
    // Query a fresh, noninteractive context only after failure. Readiness is
    // evidence at that instant and must not replace the original Keychain result.
    let context = noninteractive_context();
    let readiness_error = unsafe {
        context.canEvaluatePolicy_error(LAPolicy::DeviceOwnerAuthenticationWithBiometrics)
    }
    .err();
    let readiness_domain = readiness_error
        .as_ref()
        .map(|error| error.domain().to_string());
    tracing::warn!(
        target: "codex_user_verification::enrollment",
        stage,
        native_domain = native_error.map(|(domain, _)| domain),
        native_code = native_error.map(|(_, code)| code),
        underlying_domain = underlying.map(|(domain, _)| domain),
        underlying_code = underlying.map(|(_, code)| code),
        biometrics_ready = readiness_error.is_none(),
        readiness_domain = readiness_domain.as_deref().map(diagnostic_domain),
        readiness_code = readiness_error.as_ref().map(|error| error.code() as i64),
        "macos_user_verification_enrollment_failed"
    );
}

fn diagnostic_code(error: &CFError) -> (&'static str, i64) {
    (
        diagnostic_domain(&error.domain().to_string()),
        error.code() as i64,
    )
}

fn underlying_error(error: &CFError) -> Option<(&'static str, i64)> {
    // Security may wrap a more specific error. Read only the underlying error's
    // domain and code; the rest of userInfo can contain private descriptions.
    let info = unsafe { CFErrorCopyUserInfo(error.as_concrete_TypeRef()) };
    if info.is_null() {
        return None;
    }
    let info = unsafe { CFDictionary::<CFString, CFType>::wrap_under_create_rule(info) };
    let key = unsafe { CFString::wrap_under_get_rule(kCFErrorUnderlyingErrorKey) };
    let cause = info.find(&key)?.downcast::<CFError>()?;
    Some(diagnostic_code(&cause))
}

fn diagnostic_domain(domain: &str) -> &'static str {
    match domain {
        "NSOSStatusErrorDomain" => "NSOSStatusErrorDomain",
        "com.apple.LocalAuthentication" => "com.apple.LocalAuthentication",
        "CryptoTokenKit" => "CryptoTokenKit",
        _ => "other",
    }
}
