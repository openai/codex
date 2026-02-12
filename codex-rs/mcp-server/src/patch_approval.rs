use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use codex_core::CodexThread;
use codex_core::protocol::FileChange;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use codex_protocol::ThreadId;
use rmcp::model::ErrorData;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tracing::error;

use crate::outgoing_message::OutgoingMessageSender;

#[derive(Debug, Deserialize, Serialize)]
pub struct PatchApprovalElicitRequestParams {
    pub message: String,
    #[serde(rename = "requestedSchema")]
    pub requested_schema: Value,
    #[serde(rename = "threadId")]
    pub thread_id: ThreadId,
    pub codex_elicitation: String,
    pub codex_mcp_tool_call_id: String,
    pub codex_event_id: String,
    pub codex_call_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub codex_grant_root: Option<PathBuf>,
    pub codex_changes: HashMap<PathBuf, FileChange>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PatchApprovalResponse {
    pub decision: ReviewDecision,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn handle_patch_approval_request(
    call_id: String,
    reason: Option<String>,
    grant_root: Option<PathBuf>,
    changes: HashMap<PathBuf, FileChange>,
    outgoing: Arc<OutgoingMessageSender>,
    codex: Arc<CodexThread>,
    request_id: RequestId,
    tool_call_id: String,
    event_id: String,
    thread_id: ThreadId,
) {
    let approval_id = call_id.clone();
    let mut message_lines = Vec::new();
    if let Some(r) = &reason {
        message_lines.push(r.clone());
    }
    message_lines.push("Allow Codex to apply proposed code changes?".to_string());

    let params = PatchApprovalElicitRequestParams {
        message: message_lines.join("\n"),
        requested_schema: json!({"type":"object","properties":{}}),
        thread_id,
        codex_elicitation: "patch-approval".to_string(),
        codex_mcp_tool_call_id: tool_call_id.clone(),
        codex_event_id: event_id.clone(),
        codex_call_id: call_id,
        codex_reason: reason,
        codex_grant_root: grant_root,
        codex_changes: changes,
    };
    let params_json = match serde_json::to_value(&params) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("Failed to serialize PatchApprovalElicitRequestParams: {err}");
            error!("{message}");

            outgoing
                .send_error(request_id.clone(), ErrorData::invalid_params(message, None))
                .await;

            return;
        }
    };

    let on_response = outgoing
        .send_request("elicitation/create", Some(params_json))
        .await;

    // Listen for the response on a separate task so we don't block the main agent loop.
    {
        let codex = codex.clone();
        let approval_id = approval_id.clone();
        tokio::spawn(async move {
            on_patch_approval_response(approval_id, on_response, codex).await;
        });
    }
}

pub(crate) async fn on_patch_approval_response(
    approval_id: String,
    receiver: tokio::sync::oneshot::Receiver<serde_json::Value>,
    codex: Arc<CodexThread>,
) {
    let response = receiver.await;
    let value = match response {
        Ok(value) => value,
        Err(err) => {
            error!("request failed: {err:?}");
            if let Err(submit_err) = codex
                .submit(Op::PatchApproval {
                    id: approval_id.clone(),
                    decision: ReviewDecision::Denied,
                })
                .await
            {
                error!("failed to submit denied PatchApproval after request failure: {submit_err}");
            }
            return;
        }
    };

    let response = serde_json::from_value::<PatchApprovalResponse>(value).unwrap_or_else(|err| {
        error!("failed to deserialize PatchApprovalResponse: {err}");
        PatchApprovalResponse {
            decision: ReviewDecision::Denied,
        }
    });

    if let Err(err) = codex
        .submit(Op::PatchApproval {
            id: approval_id,
            decision: response.decision,
        })
        .await
    {
        error!("failed to submit PatchApproval: {err}");
    }
}
