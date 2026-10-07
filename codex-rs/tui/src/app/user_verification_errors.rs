//! Keeps safe display messages and bounded categories for local verification failures.

use codex_app_server_client::TypedRequestError;
use codex_app_server_protocol::UserVerificationCancellationReason;
use codex_app_server_protocol::UserVerificationErrorDetails;
use codex_app_server_protocol::UserVerificationFailureReason;
use codex_app_server_protocol::UserVerificationInvalidRequestReason;
use codex_app_server_protocol::UserVerificationUnavailableReason;

use crate::app_command::UserVerificationFailure;

pub(super) fn verification_failure(error: &TypedRequestError) -> UserVerificationFailure {
    let TypedRequestError::Server { source, .. } = error else {
        return UserVerificationFailure {
            message: "Could not complete user verification with the local Codex binary.",
            details: UserVerificationErrorDetails::Failed {
                reason: UserVerificationFailureReason::ServiceError,
            },
        };
    };
    let details = source
        .data
        .clone()
        .and_then(|data| serde_json::from_value::<UserVerificationErrorDetails>(data).ok());
    let message = match details {
        Some(UserVerificationErrorDetails::InvalidRequest {
            reason: UserVerificationInvalidRequestReason::InvalidParams,
        }) => "The local Codex binary could not verify this request.",
        Some(UserVerificationErrorDetails::Unavailable {
            reason: UserVerificationUnavailableReason::CredentialMissing,
        }) => "No user-verification credential is available in the local Codex binary.",
        Some(UserVerificationErrorDetails::Unavailable {
            reason: UserVerificationUnavailableReason::BiometricsUnavailable,
        }) => "Biometric verification is currently unavailable on this device.",
        Some(UserVerificationErrorDetails::Unavailable {
            reason: UserVerificationUnavailableReason::ProviderUnavailable,
        }) => "User verification is unavailable for this request.",
        Some(UserVerificationErrorDetails::Cancelled {
            reason: UserVerificationCancellationReason::UserCancelled,
        }) => "User verification was cancelled.",
        Some(UserVerificationErrorDetails::Cancelled {
            reason: UserVerificationCancellationReason::Interrupted,
        }) => "User verification was interrupted.",
        Some(UserVerificationErrorDetails::Failed {
            reason: UserVerificationFailureReason::AuthenticationFailed,
        }) => "Biometric verification did not succeed. The request was cancelled.",
        Some(UserVerificationErrorDetails::Failed {
            reason: UserVerificationFailureReason::Timeout,
        }) => "User verification timed out. The request was cancelled.",
        Some(UserVerificationErrorDetails::Failed {
            reason: UserVerificationFailureReason::ProviderError,
        })
        | Some(UserVerificationErrorDetails::Failed {
            reason: UserVerificationFailureReason::ServiceError,
        })
        | None => "The local Codex binary could not complete user verification.",
    };
    UserVerificationFailure {
        message,
        details: details.unwrap_or(UserVerificationErrorDetails::Failed {
            reason: UserVerificationFailureReason::ServiceError,
        }),
    }
}

#[cfg(test)]
#[path = "user_verification_errors_tests.rs"]
mod tests;
