use std::path::PathBuf;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::protocol::FileChange;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use mcp_types::ElicitRequest;
use mcp_types::ElicitRequestParamsRequestedSchema;
use mcp_types::ModelContextProtocolRequest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
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

pub(crate) async fn handle_patch_approval_request(
    reason: Option<String>,
    grant_root: Option<PathBuf>,
    changes: std::collections::HashMap<PathBuf, FileChange>,
    outgoing: Arc<crate::outgoing_message::OutgoingMessageSender>,
    codex: Arc<Codex>,
    sub_id: String,
    event_id: String,
) {
    let mut message_lines = Vec::new();
    if let Some(r) = &reason {
        message_lines.push(r.clone());
    }
    message_lines.push("Allow Codex to apply proposed code changes?".to_string());

    let params = PatchApprovalElicitRequestParams {
        message: message_lines.join("\n"),
        requested_schema: ElicitRequestParamsRequestedSchema {
            r#type: "object".to_string(),
            properties: json!({}),
            required: None,
        },
        codex_elicitation: "patch-approval".to_string(),
        codex_mcp_tool_call_id: sub_id.clone(),
        codex_event_id: event_id.clone(),
        codex_reason: reason,
        codex_grant_root: grant_root,
        codex_changes: changes,
    };
    let params_json = match serde_json::to_value(&params) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("Failed to serialize PatchApprovalElicitRequestParams: {err}");
            tracing::error!("{message}");
            return;
        }
    };

    let on_response = outgoing
        .send_request(ElicitRequest::METHOD, Some(params_json))
        .await;

    // Listen for the response on a separate task so we don't block the main agent loop.
    {
        let codex = codex.clone();
        let event_id = event_id.clone();
        tokio::spawn(async move {
            on_patch_approval_response(event_id, on_response, codex).await;
        });
    }
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
