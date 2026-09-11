//! Experimental user-verification APIs for trusted UI clients.
//! Native implementation and registration transport are independent of these contracts.

use crate::JsonSchema;
use crate::RequestId;
use crate::TS;
use serde::Deserialize;
use serde::Serialize;

/// A signature over the exact decoded challenge. The verifier validates and consumes it.
#[derive(Serialize, Deserialize, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationProof {
    pub credential_id: String,
    /// Unpadded base64url DER ECDSA signature using P-256 and SHA-256.
    pub signature: String,
}

impl std::fmt::Debug for UserVerificationProof {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UserVerificationProof")
            .finish_non_exhaustive()
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UserVerificationUnavailableReason {
    CredentialMissing,
    BiometricsUnavailable,
    ProviderUnavailable,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UserVerificationCancellationReason {
    UserCancelled,
    Interrupted,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UserVerificationFailureReason {
    AuthenticationFailed,
    Timeout,
    ProviderError,
    ServiceError,
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(rename_all = "camelCase", export_to = "v2/")]
pub enum UserVerificationInvalidRequestReason {
    InvalidParams,
}

/// Closed error categories; native diagnostic payloads must not cross this boundary.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq, JsonSchema, TS)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
#[ts(tag = "type", rename_all = "camelCase", export_to = "v2/")]
pub enum UserVerificationErrorDetails {
    InvalidRequest {
        reason: UserVerificationInvalidRequestReason,
    },
    Unavailable {
        reason: UserVerificationUnavailableReason,
    },
    Cancelled {
        reason: UserVerificationCancellationReason,
    },
    Failed {
        reason: UserVerificationFailureReason,
    },
}

/// The error object inside the normal JSON-RPC envelope.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationRpcError {
    #[ts(type = "number")]
    pub code: i64,
    pub message: String,
    pub data: UserVerificationErrorDetails,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationStatusParams {}

/// Local readiness only; this neither prompts nor queries server registration.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UserVerificationStatusResponse {
    pub credential_id: Option<String>,
    pub unavailable_reason: Option<UserVerificationUnavailableReason>,
    pub unavailable_message: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationEnrollParams {}

/// Public metadata for a created or reused local credential. The caller completes
/// backend registration; this response does not establish server enrollment.
/// Older app-servers omit the metadata fields; callers must check both before registration.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UserVerificationEnrollResponse {
    pub credential_id: String,
    // TODO: Make both metadata fields required after the experimental/alpha rollout
    // guarantees them on the oldest supported app-server version.
    pub algorithm: Option<String>,
    /// Unpadded base64url of the SubjectPublicKeyInfo DER encoding.
    pub public_key: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationDeleteParams {}

#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UserVerificationDeleteResponse {}

/// Local signing primitive, independent of any pending elicitation.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationVerifyParams {
    /// Unpadded base64url encoding of 1–4096 challenge bytes.
    pub challenge: String,
    /// Display context already approved by the UI; 1–256 UTF-8 bytes.
    pub title: String,
    /// Additional display context; at most 4096 UTF-8 bytes.
    pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UserVerificationVerifyResponse {
    pub proof: UserVerificationProof,
}

/// Cancels a native verification RPC issued on this connection, not an elicitation.
/// Use a fresh request ID for each operation and a distinct ID for this cancellation RPC.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export_to = "v2/")]
pub struct UserVerificationCancelParams {
    pub request_id: RequestId,
}

/// Acknowledges the cancellation signal; native work may still be finishing.
/// Unknown or finished requests are a no-op, and completed effects are not rolled back.
#[derive(Serialize, Deserialize, Debug, Default, Clone, PartialEq, Eq, JsonSchema, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export_to = "v2/")]
pub struct UserVerificationCancelResponse {}

#[cfg(test)]
#[path = "user_verification_tests.rs"]
mod tests;
