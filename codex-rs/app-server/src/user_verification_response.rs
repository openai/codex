//! Device proofs travel only in elicitation content, never in diagnostic metadata.

use codex_app_server_protocol::McpServerElicitationAction;
use codex_app_server_protocol::McpServerElicitationRequestResponse;
use codex_app_server_protocol::UserVerificationProof;
use codex_rmcp_client::UserVerificationReason;
use serde::Deserialize;
use tokio::sync::oneshot;

use crate::outgoing_message::ClientRequestResult;
use crate::server_request_error::is_turn_transition_server_request_error;

pub(crate) fn from_client_result(
    result: Result<ClientRequestResult, oneshot::error::RecvError>,
) -> McpServerElicitationRequestResponse {
    let mut reason = UserVerificationReason::InvalidResponse;
    let response = match result {
        Ok(Ok(value)) => serde_json::from_value::<McpServerElicitationRequestResponse>(value).ok(),
        Ok(Err(error)) => {
            reason = if is_turn_transition_server_request_error(&error) {
                UserVerificationReason::Interrupted
            } else {
                UserVerificationReason::ResponseError
            };
            None
        }
        Err(_) => {
            reason = UserVerificationReason::ResponseChannelClosed;
            None
        }
    };
    if let Some(mut response) = response {
        let client_reason = UserVerificationReason::from_meta(response.meta.as_ref());
        response.meta = None;
        match response.action {
            McpServerElicitationAction::Accept => {
                if response
                    .content
                    .as_ref()
                    .is_some_and(|content| UserVerificationProof::deserialize(content).is_ok())
                {
                    // The MCP boundary validates the encoded proof and size limits.
                    return response;
                }
                reason = UserVerificationReason::InvalidProof;
            }
            McpServerElicitationAction::Decline | McpServerElicitationAction::Cancel => {
                response.content = None;
                response.meta = client_reason.map(UserVerificationReason::into_meta);
                return response;
            }
        }
    }
    McpServerElicitationRequestResponse {
        action: McpServerElicitationAction::Cancel,
        content: None,
        meta: Some(reason.into_meta()),
    }
}

#[cfg(test)]
#[path = "user_verification_response_tests.rs"]
mod tests;
