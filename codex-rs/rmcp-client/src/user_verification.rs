//! Validation for the user-verification elicitation extension.

use base64::Engine as _;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use rmcp::model::CustomRequest;
use rmcp::model::ElicitationAction;
use serde::Deserialize;
use serde::Serialize;

use crate::rmcp_client::Elicitation;
use crate::rmcp_client::ElicitationResponse;

pub(crate) const MODE: &str = "openai/userVerification";

/// Bounded diagnostic reason only; never a proof or an authorization decision.
#[derive(Clone, Copy, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum UserVerificationReason {
    UserCancelled,
    UserDeclined,
    CredentialMissing,
    BiometricsUnavailable,
    ProviderUnavailable,
    AuthenticationFailed,
    Timeout,
    ProviderError,
    ServiceError,
    InvalidParams,
    InvalidProof,
    InvalidResponse,
    ResponseError,
    ResponseChannelClosed,
    Interrupted,
    ApprovalUnavailable,
}

impl UserVerificationReason {
    pub fn from_meta(meta: Option<&serde_json::Value>) -> Option<Self> {
        Self::deserialize(meta?.get("openai/userVerificationReason")?).ok()
    }

    pub fn into_meta(self) -> serde_json::Value {
        serde_json::json!({"openai/userVerificationReason": self})
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct RequestParams {
    mode: String,
    #[serde(rename = "_meta")]
    meta: Option<serde_json::Map<String, serde_json::Value>>,
    title: String,
    description: String,
    challenge: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Proof<'a> {
    credential_id: &'a str,
    signature: &'a str,
}

pub(crate) fn parse_request(request: CustomRequest) -> Result<Elicitation, rmcp::ErrorData> {
    let params = request
        .params_as::<RequestParams>()
        .ok()
        .flatten()
        .ok_or_else(invalid_request)?;
    if params.mode != MODE
        || params.title.is_empty()
        || params.title.len() > 256
        || params.description.len() > 4096
        || !valid_bytes(&params.challenge, /*max_decoded_bytes*/ 4096)
    {
        return Err(invalid_request());
    }
    Ok(Elicitation::UserVerification {
        meta: params.meta.map(serde_json::Value::Object),
        title: params.title,
        description: params.description,
        challenge: params.challenge,
    })
}

/// Accept only a bounded proof, and never return proof material for cancellation or rejection.
pub(crate) fn validate_response(mut response: ElicitationResponse) -> ElicitationResponse {
    let reason = UserVerificationReason::from_meta(response.meta.as_ref());
    response.meta = None;
    match response.action {
        ElicitationAction::Accept => {
            let proof = response
                .content
                .as_ref()
                .and_then(|content| Proof::deserialize(content).ok());
            if proof.is_some_and(|proof| {
                !proof.credential_id.is_empty()
                    && proof.credential_id.len() <= 1024
                    && valid_bytes(proof.signature, /*max_decoded_bytes*/ 128)
            }) {
                return response;
            }
            tracing::warn!("user-verification acceptance omitted a valid proof; cancelling");
            response.action = ElicitationAction::Cancel;
            response.meta = Some(UserVerificationReason::InvalidProof.into_meta());
        }
        ElicitationAction::Decline | ElicitationAction::Cancel => {
            response.meta = reason.map(UserVerificationReason::into_meta);
        }
        _ => {
            response.action = ElicitationAction::Cancel;
            response.meta = Some(UserVerificationReason::InvalidResponse.into_meta());
        }
    }
    response.content = None;
    response
}

fn valid_bytes(encoded: &str, max_decoded_bytes: usize) -> bool {
    !encoded.is_empty()
        && encoded.len() <= max_decoded_bytes.div_ceil(3) * 4
        && URL_SAFE_NO_PAD
            .decode(encoded)
            .is_ok_and(|bytes| !bytes.is_empty() && bytes.len() <= max_decoded_bytes)
}

fn invalid_request() -> rmcp::ErrorData {
    rmcp::ErrorData::invalid_params("invalid user-verification request", /*data*/ None)
}

#[cfg(test)]
#[path = "user_verification_tests.rs"]
mod tests;
