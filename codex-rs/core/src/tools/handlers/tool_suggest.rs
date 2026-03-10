use std::collections::BTreeMap;
use std::collections::HashSet;

use async_trait::async_trait;
use codex_app_server_protocol::McpElicitationObjectType;
use codex_app_server_protocol::McpElicitationSchema;
use codex_app_server_protocol::McpServerElicitationRequest;
use codex_app_server_protocol::McpServerElicitationRequestParams;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use rmcp::model::RequestId;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;

use crate::connectors;
use crate::function_tool::FunctionCallError;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct ToolSuggestHandler;

pub(crate) const TOOL_SUGGEST_TOOL_NAME: &str = "tool_suggest";
const TOOL_SUGGEST_DECISION_INSTALL: &str = "install";
const TOOL_SUGGEST_DECISION_ENABLE: &str = "enable";
const TOOL_SUGGEST_DECISION_NOT_NOW: &str = "not_now";
const TOOL_SUGGEST_META_KIND_KEY: &str = "codex_approval_kind";
const TOOL_SUGGEST_META_KIND_VALUE: &str = "tool_suggestion";

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ToolSuggestionToolType {
    Connector,
    Plugin,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum ToolSuggestionType {
    Install,
    Enable,
}

impl ToolSuggestionType {
    fn decision(self) -> &'static str {
        match self {
            Self::Install => TOOL_SUGGEST_DECISION_INSTALL,
            Self::Enable => TOOL_SUGGEST_DECISION_ENABLE,
        }
    }

    fn verb(self) -> &'static str {
        match self {
            Self::Install => "Install",
            Self::Enable => "Enable",
        }
    }
}

#[derive(Deserialize)]
struct ToolSuggestArgs {
    connector_id: String,
    tool_type: ToolSuggestionToolType,
    suggestion_type: ToolSuggestionType,
    suggest_reason: Option<String>,
}

#[derive(Deserialize)]
struct ToolSuggestElicitationContent {
    decision: String,
}

fn tool_suggest_message(
    suggestion_type: ToolSuggestionType,
    connector_name: &str,
    suggest_reason: Option<&str>,
    connector_description: Option<&str>,
    url: &str,
) -> String {
    let mut parts = vec![format!(
        "{} {connector_name} to continue?",
        suggestion_type.verb()
    )];
    if let Some(suggest_reason) = normalized_optional_text(suggest_reason) {
        parts.push(format!("Reason: {suggest_reason}"));
    }
    if let Some(description) = normalized_optional_text(connector_description) {
        parts.push(description);
    }
    parts.push(format!("Open URL: {url}"));
    parts.join(" | ")
}

fn normalized_optional_text(text: Option<&str>) -> Option<String> {
    text.and_then(|text| {
        let normalized = text.split_whitespace().collect::<Vec<_>>().join(" ");
        if normalized.is_empty() {
            None
        } else {
            Some(normalized)
        }
    })
}

fn tool_suggest_requested_schema() -> McpElicitationSchema {
    McpElicitationSchema {
        schema_uri: None,
        type_: McpElicitationObjectType::Object,
        properties: BTreeMap::new(),
        required: None,
    }
}

fn tool_suggest_elicitation_meta(
    tool_type: ToolSuggestionToolType,
    suggestion_type: ToolSuggestionType,
    connector_id: &str,
    connector_name: &str,
    suggest_reason: Option<&str>,
    connector_description: Option<&str>,
    install_url: &str,
) -> Value {
    let mut meta = json!({
        TOOL_SUGGEST_META_KIND_KEY: TOOL_SUGGEST_META_KIND_VALUE,
        "tool_type": tool_type,
        "suggestion_type": suggestion_type,
        "connector_id": connector_id,
        "connector_name": connector_name,
        "connector_description": connector_description,
        "install_url": install_url,
    });
    if let Some(suggest_reason) = suggest_reason
        && let Some(meta) = meta.as_object_mut()
    {
        meta.insert("suggest_reason".to_string(), json!(suggest_reason));
    }
    meta
}

fn parse_tool_suggest_elicitation_response(
    response: ElicitationResponse,
    suggestion_type: ToolSuggestionType,
) -> (&'static str, String) {
    let elicitation_action = match response.action {
        ElicitationAction::Accept => "accept",
        ElicitationAction::Decline => "decline",
        ElicitationAction::Cancel => "cancel",
    };

    let user_decision = match response.action {
        ElicitationAction::Accept => response
            .content
            .and_then(|content| {
                serde_json::from_value::<ToolSuggestElicitationContent>(content).ok()
            })
            .map(|content| content.decision)
            .filter(|decision| {
                matches!(
                    decision.as_str(),
                    TOOL_SUGGEST_DECISION_INSTALL
                        | TOOL_SUGGEST_DECISION_ENABLE
                        | TOOL_SUGGEST_DECISION_NOT_NOW
                )
            })
            .unwrap_or_else(|| suggestion_type.decision().to_string()),
        ElicitationAction::Decline | ElicitationAction::Cancel => {
            TOOL_SUGGEST_DECISION_NOT_NOW.to_string()
        }
    };

    (elicitation_action, user_decision)
}

#[async_trait]
impl ToolHandler for ToolSuggestHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            payload,
            turn,
            call_id,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{TOOL_SUGGEST_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };

        let args: ToolSuggestArgs = parse_arguments(&arguments)?;
        let connector_id = args.connector_id.trim();
        if connector_id.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "connector_id must not be empty".to_string(),
            ));
        }
        if args.tool_type != ToolSuggestionToolType::Connector {
            return Err(FunctionCallError::RespondToModel(format!(
                "tool_type `{}` is not supported by {TOOL_SUGGEST_TOOL_NAME} yet",
                match args.tool_type {
                    ToolSuggestionToolType::Connector => "connector",
                    ToolSuggestionToolType::Plugin => "plugin",
                }
            )));
        }

        let connectors = match connectors::list_cached_connectors(&turn.config).await {
            Some(connectors) => connectors,
            None => connectors::list_connectors(&turn.config)
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to load discoverable apps: {err}"
                    ))
                })?,
        };

        let connector = connectors
            .into_iter()
            .find(|connector| connector.id == connector_id)
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!("unknown connector_id `{connector_id}`"))
            })?;

        let enabled_connector_overrides = session.get_connector_selection().await;
        let connector_is_enabled =
            connector.is_enabled || enabled_connector_overrides.contains(connector_id);
        match args.suggestion_type {
            ToolSuggestionType::Install => {
                if connector.is_accessible {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "connector_id `{connector_id}` is already installed; use search_tool_bm25 with mode `enabled`, or mode `discoverable` with suggestion_type `enable` if it is disabled"
                    )));
                }
            }
            ToolSuggestionType::Enable => {
                if !connector.is_accessible {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "connector_id `{connector_id}` is not installed; use search_tool_bm25 with mode `discoverable` and suggestion_type `install` instead"
                    )));
                }
                if connector_is_enabled {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "connector_id `{connector_id}` is already enabled; use its tools or search_tool_bm25 with mode `enabled` instead"
                    )));
                }
            }
        }

        let install_url = connector
            .install_url
            .clone()
            .unwrap_or_else(|| connectors::connector_install_url(&connector.name, &connector.id));
        let suggest_reason = normalized_optional_text(args.suggest_reason.as_deref());
        let request_id = RequestId::String(format!("{TOOL_SUGGEST_TOOL_NAME}_{call_id}").into());
        let elicitation_response = session
            .request_mcp_server_elicitation(
                turn.as_ref(),
                request_id,
                McpServerElicitationRequestParams {
                    thread_id: session.conversation_id.to_string(),
                    turn_id: Some(turn.sub_id.clone()),
                    server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
                    request: McpServerElicitationRequest::Form {
                        meta: Some(tool_suggest_elicitation_meta(
                            args.tool_type,
                            args.suggestion_type,
                            &connector.id,
                            &connector.name,
                            suggest_reason.as_deref(),
                            connector.description.as_deref(),
                            &install_url,
                        )),
                        message: tool_suggest_message(
                            args.suggestion_type,
                            &connector.name,
                            suggest_reason.as_deref(),
                            connector.description.as_deref(),
                            &install_url,
                        ),
                        requested_schema: tool_suggest_requested_schema(),
                    },
                },
            )
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "tool_suggest was cancelled before receiving a response".to_string(),
                )
            })?;
        let (elicitation_action, user_decision) =
            parse_tool_suggest_elicitation_response(elicitation_response, args.suggestion_type);

        if user_decision == TOOL_SUGGEST_DECISION_INSTALL
            || user_decision == TOOL_SUGGEST_DECISION_ENABLE
        {
            session
                .merge_connector_selection(HashSet::from([connector.id.clone()]))
                .await;
        }

        let user_declined_suggestion =
            user_decision == TOOL_SUGGEST_DECISION_NOT_NOW && elicitation_action != "cancel";

        let assistant_instruction = if user_decision == TOOL_SUGGEST_DECISION_INSTALL {
            "The user confirmed they completed the install flow. Treat this connector as selectable for this turn, but verify its tools appear in a `search_tool_bm25` search with `mode: \"enabled\"` before trying to use them. If they still do not appear, it may also need enabling."
        } else if user_decision == TOOL_SUGGEST_DECISION_ENABLE {
            "The user confirmed they enabled this connector. Treat it as selectable for this turn, but verify its tools appear in a `search_tool_bm25` search with `mode: \"enabled\"` before trying to use them."
        } else if user_declined_suggestion {
            "The user declined this suggestion. Do not ask them again to install or enable this connector unless they later ask for it. Do not try to use this tool in this turn."
        } else {
            "The user did not complete this suggestion flow. Do not try to use this tool in this turn."
        };
        let mut content = json!({
            "connector_id": connector.id,
            "connector_name": connector.name,
            "connector_description": connector.description,
            "install_url": install_url,
            "tool_type": args.tool_type,
            "suggestion_type": args.suggestion_type,
            "elicitation_action": elicitation_action,
            "user_decision": user_decision,
            "assistant_instruction": assistant_instruction,
        });
        if let Some(suggest_reason) = suggest_reason
            && let Some(content) = content.as_object_mut()
        {
            content.insert("suggest_reason".to_string(), json!(suggest_reason));
        }
        let content = content.to_string();

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

#[cfg(test)]
mod tests {
    use codex_rmcp_client::ElicitationAction;
    use codex_rmcp_client::ElicitationResponse;
    use pretty_assertions::assert_eq;

    use super::TOOL_SUGGEST_DECISION_ENABLE;
    use super::TOOL_SUGGEST_DECISION_INSTALL;
    use super::TOOL_SUGGEST_DECISION_NOT_NOW;
    use super::ToolSuggestionType;
    use super::parse_tool_suggest_elicitation_response;
    use super::tool_suggest_message;

    #[test]
    fn tool_suggest_message_uses_single_line_text() {
        let message = tool_suggest_message(
            ToolSuggestionType::Install,
            "Docs & Notes",
            Some("Need access to\n workspace docs"),
            Some("Install <now> & sync"),
            "https://example.com/apps?name=Docs",
        );

        assert_eq!(
            message,
            "Install Docs & Notes to continue? | Reason: Need access to workspace docs | Install <now> & sync | Open URL: https://example.com/apps?name=Docs"
        );
    }

    #[test]
    fn accepted_tool_suggest_defaults_to_install_without_form_content() {
        let (elicitation_action, user_decision) = parse_tool_suggest_elicitation_response(
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: None,
                meta: None,
            },
            ToolSuggestionType::Install,
        );

        assert_eq!(elicitation_action, "accept");
        assert_eq!(user_decision, TOOL_SUGGEST_DECISION_INSTALL);
    }

    #[test]
    fn accepted_tool_suggest_defaults_to_enable_without_form_content() {
        let (elicitation_action, user_decision) = parse_tool_suggest_elicitation_response(
            ElicitationResponse {
                action: ElicitationAction::Accept,
                content: None,
                meta: None,
            },
            ToolSuggestionType::Enable,
        );

        assert_eq!(elicitation_action, "accept");
        assert_eq!(user_decision, TOOL_SUGGEST_DECISION_ENABLE);
    }

    #[test]
    fn declined_tool_suggest_maps_to_not_now() {
        let (elicitation_action, user_decision) = parse_tool_suggest_elicitation_response(
            ElicitationResponse {
                action: ElicitationAction::Decline,
                content: None,
                meta: None,
            },
            ToolSuggestionType::Install,
        );

        assert_eq!(elicitation_action, "decline");
        assert_eq!(user_decision, TOOL_SUGGEST_DECISION_NOT_NOW);
    }

    #[test]
    fn cancelled_tool_suggest_maps_to_not_now() {
        let (elicitation_action, user_decision) = parse_tool_suggest_elicitation_response(
            ElicitationResponse {
                action: ElicitationAction::Cancel,
                content: None,
                meta: None,
            },
            ToolSuggestionType::Install,
        );

        assert_eq!(elicitation_action, "cancel");
        assert_eq!(user_decision, TOOL_SUGGEST_DECISION_NOT_NOW);
    }
}
