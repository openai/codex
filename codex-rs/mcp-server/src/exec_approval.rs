use std::path::PathBuf;
use std::sync::Arc;

use codex_core::Codex;
use codex_core::protocol::Op;
use codex_core::protocol::ReviewDecision;
use mcp_types::ElicitRequest;
use mcp_types::ElicitRequestParamsRequestedSchema;
use mcp_types::ModelContextProtocolRequest;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
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
pub(crate) async fn handle_exec_approval_request(
    command: Vec<String>,
    cwd: PathBuf,
    outgoing: Arc<crate::outgoing_message::OutgoingMessageSender>,
    codex: Arc<Codex>,
    sub_id: String,
    event_id: String,
) {
    let escaped_command =
        shlex::try_join(command.iter().map(|s| s.as_str())).unwrap_or_else(|_| command.join(" "));
    let message = format!("Allow Codex to run `{escaped_command}` in {cwd:?}?");

    let params = ExecApprovalElicitRequestParams {
        message,
        requested_schema: ElicitRequestParamsRequestedSchema {
            r#type: "object".to_string(),
            properties: json!({}),
            required: None,
        },
        codex_elicitation: "exec-approval".to_string(),
        codex_mcp_tool_call_id: sub_id.clone(),
        codex_event_id: event_id.clone(),
        codex_command: command,
        codex_cwd: cwd,
    };
    let params_json = match serde_json::to_value(&params) {
        Ok(value) => value,
        Err(err) => {
            let message = format!("Failed to serialize ExecApprovalElicitRequestParams: {err}");
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
            on_exec_approval_response(event_id, on_response, codex).await;
        });
    }
}

async fn on_exec_approval_response(
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
