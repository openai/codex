//! Converts MCP approval elicitations to Guardian requests and review decisions to responses.
//! The host retains session lookup, provenance checks, and approval enforcement.

use crate::GuardianApprovalRequest;
use codex_mcp::ElicitationReviewRequest;
use codex_prompts::ResolvedModelMessages;
use codex_protocol::config_types::ApprovalsReviewer;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_KEY as MCP_ELICITATION_APPROVAL_KIND_KEY;
use codex_protocol::mcp_approval_meta::APPROVAL_KIND_MCP_TOOL_CALL as MCP_ELICITATION_APPROVAL_KIND_MCP_TOOL_CALL;
use codex_protocol::mcp_approval_meta::APPROVALS_REVIEWER_KEY as MCP_ELICITATION_APPROVALS_REVIEWER_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_DESCRIPTION_KEY as MCP_ELICITATION_CONNECTOR_DESCRIPTION_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_ID_KEY as MCP_ELICITATION_CONNECTOR_ID_KEY;
use codex_protocol::mcp_approval_meta::CONNECTOR_NAME_KEY as MCP_ELICITATION_CONNECTOR_NAME_KEY;
use codex_protocol::mcp_approval_meta::REQUEST_TYPE_APPROVAL_REQUEST as MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST;
use codex_protocol::mcp_approval_meta::REQUEST_TYPE_KEY as MCP_ELICITATION_REQUEST_TYPE_KEY;
use codex_protocol::mcp_approval_meta::TOOL_DESCRIPTION_KEY as MCP_ELICITATION_TOOL_DESCRIPTION_KEY;
use codex_protocol::mcp_approval_meta::TOOL_NAME_KEY as MCP_ELICITATION_TOOL_NAME_KEY;
use codex_protocol::mcp_approval_meta::TOOL_PARAMS_KEY as MCP_ELICITATION_TOOL_PARAMS_KEY;
use codex_protocol::mcp_approval_meta::TOOL_TITLE_KEY as MCP_ELICITATION_TOOL_TITLE_KEY;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::protocol::ReviewDecision;
use codex_rmcp_client::Elicitation;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::ElicitationAction;
use rmcp::model::RequestMetaObject;
use serde_json::Map;
use serde_json::Value;

const MCP_ELICITATION_DECLINE_MESSAGE_KEY: &str = "message";

#[derive(Debug, PartialEq)]
pub enum GuardianElicitationReview {
    NotRequested,
    Decline(&'static str),
    ApprovalRequest {
        request: Box<GuardianApprovalRequest>,
        tool_call_id: Option<String>,
    },
}

pub fn guardian_elicitation_review_request(
    request: &ElicitationReviewRequest,
    originating_call_id: Option<&str>,
) -> GuardianElicitationReview {
    let (meta, requested_schema) = match &request.elicitation {
        Elicitation::Mcp(rmcp::model::ElicitRequestParams::FormElicitationParams {
            meta,
            requested_schema,
            ..
        }) => (meta, Some(requested_schema)),
        Elicitation::Mcp(rmcp::model::ElicitRequestParams::UrlElicitationParams {
            meta, ..
        }) => {
            return if meta_requests_approval_request(meta) {
                GuardianElicitationReview::Decline(
                    "guardian MCP elicitation review only supports form elicitations",
                )
            } else {
                GuardianElicitationReview::NotRequested
            };
        }
        Elicitation::Mcp(_) => {
            return GuardianElicitationReview::Decline(
                "guardian MCP elicitation review does not support this elicitation mode",
            );
        }
        Elicitation::OpenAiForm { .. }
        | Elicitation::OpenAiElicitationForm { .. }
        | Elicitation::UserVerification { .. } => {
            return GuardianElicitationReview::NotRequested;
        }
    };

    let Some(meta) = meta.as_ref().map(|meta| &meta.0.0) else {
        return GuardianElicitationReview::NotRequested;
    };
    if metadata_str(meta, MCP_ELICITATION_REQUEST_TYPE_KEY)
        != Some(MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST)
    {
        return GuardianElicitationReview::NotRequested;
    }
    if metadata_str(meta, MCP_ELICITATION_APPROVAL_KIND_KEY)
        != Some(MCP_ELICITATION_APPROVAL_KIND_MCP_TOOL_CALL)
    {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation metadata must declare mcp_tool_call approval kind",
        );
    }
    if requested_schema.is_some_and(|schema| !schema.properties.is_empty()) {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation review only supports empty form schemas",
        );
    }

    let Some(tool_name) = metadata_owned_string(meta, MCP_ELICITATION_TOOL_NAME_KEY) else {
        return GuardianElicitationReview::Decline(
            "guardian MCP elicitation metadata must include a non-empty tool_name",
        );
    };
    let arguments = match meta.get(MCP_ELICITATION_TOOL_PARAMS_KEY) {
        Some(value @ Value::Object(_)) => Some(value.clone()),
        Some(_) => {
            return GuardianElicitationReview::Decline(
                "guardian MCP elicitation tool_params must be an object",
            );
        }
        None => Some(Value::Object(Map::new())),
    };

    GuardianElicitationReview::ApprovalRequest {
        // Synthetic display IDs must not participate in tool observation ordering.
        tool_call_id: originating_call_id.map(str::to_owned),
        request: Box::new(GuardianApprovalRequest::McpToolCall {
            id: originating_call_id.map(str::to_owned).unwrap_or_else(|| {
                format!(
                    "mcp_elicitation:{}:{}",
                    request.server_name, request.request_id
                )
            }),
            server: request.server_name.clone(),
            tool_name,
            arguments,
            connector_id: metadata_owned_string(meta, MCP_ELICITATION_CONNECTOR_ID_KEY),
            connector_name: metadata_owned_string(meta, MCP_ELICITATION_CONNECTOR_NAME_KEY),
            connector_description: metadata_owned_string(
                meta,
                MCP_ELICITATION_CONNECTOR_DESCRIPTION_KEY,
            ),
            connected_account_email: None,
            tool_title: metadata_owned_string(meta, MCP_ELICITATION_TOOL_TITLE_KEY),
            tool_description: metadata_owned_string(meta, MCP_ELICITATION_TOOL_DESCRIPTION_KEY),
            annotations: None,
        }),
    }
}

fn meta_requests_approval_request(meta: &Option<RequestMetaObject>) -> bool {
    meta.as_ref()
        .and_then(|meta| metadata_str(&meta.0, MCP_ELICITATION_REQUEST_TYPE_KEY))
        == Some(MCP_ELICITATION_REQUEST_TYPE_APPROVAL_REQUEST)
}

fn metadata_str<'a>(meta: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    meta.get(key).and_then(Value::as_str)
}

fn metadata_owned_string(meta: &Map<String, Value>, key: &str) -> Option<String> {
    metadata_str(meta, key)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}

pub fn mcp_elicitation_response_from_guardian_decision(
    decision: ReviewDecision,
    model_info: &ModelInfo,
) -> ElicitationResponse {
    match decision {
        ReviewDecision::Approved
        | ReviewDecision::ApprovedForSession
        | ReviewDecision::ApprovedMcpPolicyAmendment
        | ReviewDecision::ApprovedExecpolicyAmendment { .. }
        | ReviewDecision::NetworkPolicyAmendment { .. } => ElicitationResponse {
            action: ElicitationAction::Accept,
            content: Some(serde_json::json!({})),
            meta: Some(mcp_elicitation_auto_meta()),
        },
        ReviewDecision::Denied { rejection } => mcp_elicitation_decline_with_message(rejection),
        ReviewDecision::TimedOut => mcp_elicitation_decline_with_message(
            ResolvedModelMessages::from_model(model_info)
                .auto_review()
                .timeout_instructions
                .to_string(),
        ),
        ReviewDecision::Abort => ElicitationResponse {
            action: ElicitationAction::Cancel,
            content: None,
            meta: Some(mcp_elicitation_auto_meta()),
        },
    }
}

fn mcp_elicitation_decline_with_message(message: String) -> ElicitationResponse {
    ElicitationResponse {
        action: ElicitationAction::Decline,
        content: None,
        meta: Some(serde_json::json!({
            MCP_ELICITATION_DECLINE_MESSAGE_KEY: message,
            MCP_ELICITATION_APPROVALS_REVIEWER_KEY: ApprovalsReviewer::AutoReview,
        })),
    }
}

pub fn mcp_elicitation_decline_without_message() -> ElicitationResponse {
    ElicitationResponse {
        action: ElicitationAction::Decline,
        content: None,
        meta: Some(mcp_elicitation_auto_meta()),
    }
}

fn mcp_elicitation_auto_meta() -> serde_json::Value {
    serde_json::json!({
        MCP_ELICITATION_APPROVALS_REVIEWER_KEY: ApprovalsReviewer::AutoReview,
    })
}

#[cfg(test)]
#[path = "elicitation_tests.rs"]
mod tests;
