use std::path::PathBuf;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::protocol::FileChange;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use mcp_types::ElicitRequestParamsRequestedSchema;
use serde::Deserialize;
use serde::Serialize;
use tracing::error;

#[derive(Debug, Serialize)]
pub(crate) struct PatchApprovalElicitRequestParams {
    pub(crate) message: String,
    #[serde(rename = "requestedSchema")]
    pub(crate) requested_schema: ElicitRequestParamsRequestedSchema,
    pub(crate) codex_elicitation: String,
    pub(crate) codex_mcp_tool_call_id: String,
    pub(crate) codex_event_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) codex_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) codex_grant_root: Option<PathBuf>,
    pub(crate) codex_changes: std::collections::HashMap<PathBuf, FileChange>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PatchApprovalResponse {
    pub(crate) decision: ReviewDecision,
}

pub(crate) async fn on_patch_approval_response(
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

    let response = match serde_json::from_value::<PatchApprovalResponse>(value) {
        Ok(response) => response,
        Err(err) => {
            error!("failed to deserialize PatchApprovalResponse: {err}");
            PatchApprovalResponse {
                decision: ReviewDecision::Denied,
            }
        }
    };

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: event_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}
