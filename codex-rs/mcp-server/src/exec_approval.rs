use std::path::PathBuf;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use mcp_types::ElicitRequestParamsRequestedSchema;
use serde::Deserialize;
use serde::Serialize;
use tracing::error;

/// Conforms to [`mcp_types::ElicitRequestParams`] so that it can be used as the
/// `params` field of an [`mcp_types::ElicitRequest`].
#[derive(Debug, Serialize)]
pub(crate) struct ExecApprovalElicitRequestParams {
    // These fields are required so that `params`
    // conforms to ElicitRequestParams.
    pub(crate) message: String,

    #[serde(rename = "requestedSchema")]
    pub(crate) requested_schema: ElicitRequestParamsRequestedSchema,

    // These are additional fields the client can use to
    // correlate the request with the codex tool call.
    pub(crate) codex_elicitation: String,
    pub(crate) codex_mcp_tool_call_id: String,
    pub(crate) codex_event_id: String,
    pub(crate) codex_command: Vec<String>,
    pub(crate) codex_cwd: PathBuf,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExecApprovalResponse {
    pub(crate) decision: ReviewDecision,
}

pub(crate) async fn on_exec_approval_response(
    event_id: String,
    receiver: tokio::sync::oneshot::Receiver<mcp_types::Result>,
    codex: Arc<Codex>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            return;
        }
    };

    // Try to deserialize `value` and then make the appropriate call to `codex`.
    let response = match serde_json::from_value::<ExecApprovalResponse>(value) {
        Ok(response) => response,
        Err(err) => {
            error!("failed to deserialize ExecApprovalResponse: {err}");
            // If we cannot deserialize the response, we deny the request to be
            // conservative.
            ExecApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        }
    };

    if let Err(err) = codex
        .submit(Op::ExecApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit ExecApproval: {err}");
    }
}
