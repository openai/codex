use std::env;
use std::time::Duration;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::types::AppToolApproval;
use crate::default_client::build_reqwest_client;
use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputQuestion;
use codex_protocol::request_user_input::RequestUserInputQuestionOption;
use codex_protocol::request_user_input::RequestUserInputResponse;
use rmcp::model::ToolAnnotations;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum McpToolApprovalDecision {
    Accept,
    AcceptAndRemember,
    Decline,
    Cancel,
}

pub(crate) struct McpToolApprovalMetadata {
    pub(crate) annotations: Option<ToolAnnotations>,
    pub(crate) connector_id: Option<String>,
    pub(crate) connector_name: Option<String>,
    pub(crate) tool_description: Option<String>,
    pub(crate) tool_title: Option<String>,
}

#[derive(Clone, Copy)]
pub(crate) struct McpToolApprovalRequest<'a> {
    pub(crate) call_id: &'a str,
    pub(crate) server: &'a str,
    pub(crate) tool_name: &'a str,
    pub(crate) arguments: Option<&'a Value>,
    pub(crate) metadata: Option<&'a McpToolApprovalMetadata>,
    pub(crate) approval_mode: AppToolApproval,
}

const MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX: &str = "mcp_tool_call_approval";
const MCP_TOOL_APPROVAL_ACCEPT: &str = "Approve Once";
const MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER: &str = "Approve this Session";
const MCP_TOOL_APPROVAL_DECLINE: &str = "Deny";
const MCP_TOOL_APPROVAL_CANCEL: &str = "Cancel";
const MCP_TOOL_SAFETY_MONITOR_TIMEOUT: Duration = Duration::from_secs(15);
const MCP_TOOL_SAFETY_MONITOR_URL_OVERRIDE_ENV_VAR: &str =
    "CODEX_MCP_TOOL_SAFETY_MONITOR_URL_OVERRIDE";
const MCP_TOOL_SAFETY_MONITOR_BEARER_TOKEN_OVERRIDE_ENV_VAR: &str =
    "CODEX_MCP_TOOL_SAFETY_MONITOR_BEARER_TOKEN_OVERRIDE";

#[derive(Debug, Serialize)]
struct McpToolApprovalKey {
    server: String,
    connector_id: Option<String>,
    tool_name: String,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
enum McpToolDangerLevel {
    VeryHigh,
    High,
    Medium,
    Low,
}

impl McpToolDangerLevel {
    fn requires_approval(self) -> bool {
        self != Self::Low
    }
}

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
struct McpToolSafetyMonitorResult {
    title: String,
    description: String,
    action_name: String,
    danger_level: McpToolDangerLevel,
}

#[derive(Debug, Serialize)]
struct McpToolSafetyMonitorRequest<'a> {
    convo_snapshot: Value,
    tool_name: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_description: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    arguments: Option<&'a Value>,
}

pub(crate) async fn maybe_request_mcp_tool_approval(
    sess: &Session,
    turn_context: &TurnContext,
    request: McpToolApprovalRequest<'_>,
) -> Option<McpToolApprovalDecision> {
    if request.approval_mode == AppToolApproval::Approve {
        return None;
    }
    let annotations = request
        .metadata
        .and_then(|metadata| metadata.annotations.as_ref());
    let safety_monitor_result = if request.approval_mode == AppToolApproval::Auto {
        if is_full_access_mode(turn_context) {
            return None;
        }
        let safety_monitor_result = request_mcp_tool_safety_monitor(
            sess,
            turn_context,
            request.tool_name,
            request.arguments,
            request.metadata,
        )
        .await;
        if let Some(result) = safety_monitor_result.as_ref() {
            if !result.danger_level.requires_approval() {
                return None;
            }
        } else if !requires_mcp_tool_approval(annotations) {
            return None;
        }
        safety_monitor_result
    } else {
        None
    };

    let approval_key = if request.approval_mode == AppToolApproval::Auto {
        let connector_id = request
            .metadata
            .and_then(|metadata| metadata.connector_id.clone());
        if request.server == CODEX_APPS_MCP_SERVER_NAME && connector_id.is_none() {
            None
        } else {
            Some(McpToolApprovalKey {
                server: request.server.to_string(),
                connector_id,
                tool_name: request.tool_name.to_string(),
            })
        }
    } else {
        None
    };
    if let Some(key) = approval_key.as_ref()
        && mcp_tool_approval_is_remembered(sess, key).await
    {
        return Some(McpToolApprovalDecision::Accept);
    }

    let question_id = format!("{MCP_TOOL_APPROVAL_QUESTION_ID_PREFIX}_{}", request.call_id);
    let question = build_mcp_tool_approval_question(
        question_id.clone(),
        &request,
        safety_monitor_result.as_ref(),
        approval_key.is_some(),
    );
    let args = RequestUserInputArgs {
        questions: vec![question],
    };
    let response = sess
        .request_user_input(turn_context, request.call_id.to_string(), args)
        .await;
    let decision = normalize_approval_decision_for_mode(
        parse_mcp_tool_approval_response(response, &question_id),
        request.approval_mode,
    );
    if matches!(decision, McpToolApprovalDecision::AcceptAndRemember)
        && let Some(key) = approval_key
    {
        remember_mcp_tool_approval(sess, key).await;
    }
    Some(decision)
}

pub(crate) async fn lookup_mcp_tool_metadata(
    sess: &Session,
    server: &str,
    tool_name: &str,
) -> Option<McpToolApprovalMetadata> {
    let tools = sess
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;

    tools.into_values().find_map(|tool_info| {
        if tool_info.server_name == server && tool_info.tool_name == tool_name {
            Some(McpToolApprovalMetadata {
                annotations: tool_info.tool.annotations,
                connector_id: tool_info.connector_id,
                connector_name: tool_info.connector_name,
                tool_description: tool_info.tool.description.map(std::borrow::Cow::into_owned),
                tool_title: tool_info.tool.title,
            })
        } else {
            None
        }
    })
}

async fn request_mcp_tool_safety_monitor(
    sess: &Session,
    turn_context: &TurnContext,
    tool_name: &str,
    arguments: Option<&Value>,
    metadata: Option<&McpToolApprovalMetadata>,
) -> Option<McpToolSafetyMonitorResult> {
    let session_auth = sess.services.auth_manager.auth().await;
    let bearer_token_override =
        non_empty_env_var(MCP_TOOL_SAFETY_MONITOR_BEARER_TOKEN_OVERRIDE_ENV_VAR);
    let account_id = if bearer_token_override.is_some() {
        None
    } else {
        session_auth.as_ref().and_then(|auth| auth.get_account_id())
    };
    let bearer_token = if let Some(bearer_token_override) = bearer_token_override {
        bearer_token_override
    } else {
        let auth = session_auth.as_ref()?;
        if !auth.is_chatgpt_auth() {
            return None;
        }
        match auth.get_token() {
            Ok(token) => token,
            Err(err) => {
                tracing::warn!("failed to read auth token for MCP tool safety monitor: {err}");
                return None;
            }
        }
    };

    let tool_description = metadata
        .and_then(|metadata| metadata.tool_description.as_deref())
        .or_else(|| metadata.and_then(|metadata| metadata.tool_title.as_deref()));
    let payload = McpToolSafetyMonitorRequest {
        convo_snapshot: build_mcp_tool_safety_monitor_convo_snapshot(sess).await,
        tool_name,
        tool_description,
        arguments,
    };
    let url = mcp_tool_safety_monitor_url(&turn_context.config.chatgpt_base_url);
    let mut request = build_reqwest_client()
        .post(&url)
        .timeout(MCP_TOOL_SAFETY_MONITOR_TIMEOUT)
        .header("Content-Type", "application/json")
        .bearer_auth(bearer_token)
        .json(&payload);
    if let Some(account_id) = account_id {
        request = request.header("chatgpt-account-id", account_id);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            tracing::warn!("failed to call MCP tool safety monitor for {tool_name}: {err}");
            return None;
        }
    };
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    if !status.is_success() {
        tracing::warn!(
            "MCP tool safety monitor failed for {tool_name} with status {status}: {body}"
        );
        return None;
    }

    match serde_json::from_str::<McpToolSafetyMonitorResult>(&body) {
        Ok(result) => Some(result),
        Err(err) => {
            tracing::warn!(
                "failed to parse MCP tool safety monitor response for {tool_name}: {err}; body={body}"
            );
            None
        }
    }
}

fn mcp_tool_safety_monitor_url(base_url: &str) -> String {
    if let Some(url_override) = non_empty_env_var(MCP_TOOL_SAFETY_MONITOR_URL_OVERRIDE_ENV_VAR) {
        url_override
    } else {
        mcp_tool_safety_monitor_url_from_base_url(base_url)
    }
}

fn mcp_tool_safety_monitor_url_from_base_url(base_url: &str) -> String {
    let base_url = normalize_chatgpt_backend_base_url(base_url);
    format!("{base_url}/codex/safety_monitor")
}

fn non_empty_env_var(env_var: &str) -> Option<String> {
    match env::var(env_var) {
        Ok(value) => {
            let value = value.trim();
            if value.is_empty() {
                None
            } else {
                Some(value.to_string())
            }
        }
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            tracing::warn!("{env_var} contains invalid Unicode; ignoring");
            None
        }
    }
}

fn normalize_chatgpt_backend_base_url(base_url: &str) -> String {
    let mut base_url = base_url.trim_end_matches('/').to_string();
    if (base_url.starts_with("https://chatgpt.com")
        || base_url.starts_with("https://chat.openai.com"))
        && !base_url.contains("/backend-api")
    {
        base_url = format!("{base_url}/backend-api");
    }
    base_url
}

async fn build_mcp_tool_safety_monitor_convo_snapshot(sess: &Session) -> Value {
    let history = sess.clone_history().await;
    mcp_tool_safety_monitor_convo_snapshot_from_items(
        sess.conversation_id.to_string(),
        history.raw_items(),
    )
}

fn mcp_tool_safety_monitor_convo_snapshot_from_items(
    conversation_id: String,
    items: &[ResponseItem],
) -> Value {
    let messages = items
        .iter()
        .filter_map(mcp_tool_safety_monitor_message_from_item)
        .collect::<Vec<_>>();

    serde_json::json!({
        "id": conversation_id,
        "messages": messages,
    })
}

fn mcp_tool_safety_monitor_message_from_item(item: &ResponseItem) -> Option<Value> {
    let ResponseItem::Message { role, content, .. } = item else {
        return None;
    };
    if !matches!(role.as_str(), "assistant" | "developer" | "system" | "user") {
        return None;
    }
    let text = mcp_tool_safety_monitor_text(content)?;

    Some(serde_json::json!({
        "author": {
            "role": role,
        },
        "content": {
            "content_type": "text",
            "parts": [text],
        },
    }))
}

fn mcp_tool_safety_monitor_text(content: &[ContentItem]) -> Option<String> {
    let text = content
        .iter()
        .filter_map(|item| match item {
            ContentItem::InputText { text } | ContentItem::OutputText { text }
                if !text.trim().is_empty() =>
            {
                Some(text.as_str())
            }
            ContentItem::InputText { .. }
            | ContentItem::InputImage { .. }
            | ContentItem::OutputText { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n");

    if text.is_empty() { None } else { Some(text) }
}

fn is_full_access_mode(turn_context: &TurnContext) -> bool {
    matches!(turn_context.approval_policy.value(), AskForApproval::Never)
        && matches!(
            turn_context.sandbox_policy.get(),
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

fn build_mcp_tool_approval_question(
    question_id: String,
    request: &McpToolApprovalRequest<'_>,
    safety_monitor_result: Option<&McpToolSafetyMonitorResult>,
    allow_remember_option: bool,
) -> RequestUserInputQuestion {
    let annotations = request
        .metadata
        .and_then(|metadata| metadata.annotations.as_ref());
    let app_label = request
        .metadata
        .and_then(|metadata| metadata.connector_name.as_deref())
        .map(|name| format!("The {name} app"))
        .unwrap_or_else(|| {
            if request.server == CODEX_APPS_MCP_SERVER_NAME {
                "This app".to_string()
            } else {
                let server = request.server;
                format!("The {server} MCP server")
            }
        });
    let tool_label = request
        .metadata
        .and_then(|metadata| metadata.tool_title.as_deref())
        .filter(|title| !title.trim().is_empty())
        .or_else(|| {
            safety_monitor_result.and_then(|result| {
                let action_name = result.action_name.trim();
                if action_name.is_empty() {
                    None
                } else {
                    Some(result.action_name.as_str())
                }
            })
        })
        .unwrap_or(request.tool_name);
    let question = if let Some(result) = safety_monitor_result {
        let detail = mcp_tool_safety_monitor_question_detail(result);
        format!("{app_label} wants to run the tool \"{tool_label}\". {detail} Allow this action?")
    } else {
        let destructive =
            annotations.and_then(|annotations| annotations.destructive_hint) == Some(true);
        let open_world =
            annotations.and_then(|annotations| annotations.open_world_hint) == Some(true);
        let reason = match (destructive, open_world) {
            (true, true) => "may modify data and access external systems",
            (true, false) => "may modify or delete data",
            (false, true) => "may access external systems",
            (false, false) => "may have side effects",
        };

        format!(
            "{app_label} wants to run the tool \"{tool_label}\", which {reason}. Allow this action?"
        )
    };

    let mut options = vec![RequestUserInputQuestionOption {
        label: MCP_TOOL_APPROVAL_ACCEPT.to_string(),
        description: "Run the tool and continue.".to_string(),
    }];
    if allow_remember_option {
        options.push(RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER.to_string(),
            description: "Run the tool and remember this choice for this session.".to_string(),
        });
    }
    options.extend([
        RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_DECLINE.to_string(),
            description: "Decline this tool call and continue.".to_string(),
        },
        RequestUserInputQuestionOption {
            label: MCP_TOOL_APPROVAL_CANCEL.to_string(),
            description: "Cancel this tool call".to_string(),
        },
    ]);

    RequestUserInputQuestion {
        id: question_id,
        header: "Approve app tool call?".to_string(),
        question,
        is_other: false,
        is_secret: false,
        options: Some(options),
    }
}

fn mcp_tool_safety_monitor_question_detail(result: &McpToolSafetyMonitorResult) -> String {
    let title = result.title.trim();
    let description = result.description.trim();
    match (title.is_empty(), description.is_empty()) {
        (true, true) => "This tool call needs approval.".to_string(),
        (false, true) => ensure_sentence_punctuation(title),
        (true, false) => ensure_sentence_punctuation(description),
        (false, false) => {
            let title = if title.ends_with(':') {
                title.to_string()
            } else {
                format!("{title}:")
            };
            let description = ensure_sentence_punctuation(description);
            format!("{title} {description}")
        }
    }
}

fn ensure_sentence_punctuation(text: &str) -> String {
    let text = text.trim();
    if text.ends_with('.') || text.ends_with('!') || text.ends_with('?') {
        text.to_string()
    } else {
        format!("{text}.")
    }
}

fn parse_mcp_tool_approval_response(
    response: Option<RequestUserInputResponse>,
    question_id: &str,
) -> McpToolApprovalDecision {
    let Some(response) = response else {
        return McpToolApprovalDecision::Cancel;
    };
    let answers = response
        .answers
        .get(question_id)
        .map(|answer| answer.answers.as_slice());
    let Some(answers) = answers else {
        return McpToolApprovalDecision::Cancel;
    };
    if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
    {
        McpToolApprovalDecision::AcceptAndRemember
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_ACCEPT)
    {
        McpToolApprovalDecision::Accept
    } else if answers
        .iter()
        .any(|answer| answer == MCP_TOOL_APPROVAL_CANCEL)
    {
        McpToolApprovalDecision::Cancel
    } else {
        McpToolApprovalDecision::Decline
    }
}

fn normalize_approval_decision_for_mode(
    decision: McpToolApprovalDecision,
    approval_mode: AppToolApproval,
) -> McpToolApprovalDecision {
    if approval_mode == AppToolApproval::Prompt
        && decision == McpToolApprovalDecision::AcceptAndRemember
    {
        McpToolApprovalDecision::Accept
    } else {
        decision
    }
}

async fn mcp_tool_approval_is_remembered(sess: &Session, key: &McpToolApprovalKey) -> bool {
    let store = sess.services.tool_approvals.lock().await;
    matches!(store.get(key), Some(ReviewDecision::ApprovedForSession))
}

async fn remember_mcp_tool_approval(sess: &Session, key: McpToolApprovalKey) {
    let mut store = sess.services.tool_approvals.lock().await;
    store.put(key, ReviewDecision::ApprovedForSession);
}

fn requires_mcp_tool_approval(annotations: Option<&ToolAnnotations>) -> bool {
    let Some(annotations) = annotations else {
        return false;
    };
    if annotations.destructive_hint == Some(true) {
        return true;
    }

    annotations.read_only_hint == Some(false) && annotations.open_world_hint == Some(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn annotations(
        read_only: Option<bool>,
        destructive: Option<bool>,
        open_world: Option<bool>,
    ) -> ToolAnnotations {
        ToolAnnotations {
            destructive_hint: destructive,
            idempotent_hint: None,
            open_world_hint: open_world,
            read_only_hint: read_only,
            title: None,
        }
    }

    fn metadata(
        annotations: Option<ToolAnnotations>,
        connector_name: Option<&str>,
        tool_title: Option<&str>,
    ) -> McpToolApprovalMetadata {
        McpToolApprovalMetadata {
            annotations,
            connector_id: None,
            connector_name: connector_name.map(ToString::to_string),
            tool_description: None,
            tool_title: tool_title.map(ToString::to_string),
        }
    }

    fn approval_request<'a>(
        server: &'a str,
        tool_name: &'a str,
        metadata: Option<&'a McpToolApprovalMetadata>,
    ) -> McpToolApprovalRequest<'a> {
        McpToolApprovalRequest {
            call_id: "call-1",
            server,
            tool_name,
            arguments: None,
            metadata,
            approval_mode: AppToolApproval::Auto,
        }
    }

    #[test]
    fn approval_required_when_read_only_false_and_destructive() {
        let annotations = annotations(Some(false), Some(true), None);
        assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
    }

    #[test]
    fn approval_required_when_read_only_false_and_open_world() {
        let annotations = annotations(Some(false), None, Some(true));
        assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
    }

    #[test]
    fn approval_required_when_destructive_even_if_read_only_true() {
        let annotations = annotations(Some(true), Some(true), Some(true));
        assert_eq!(requires_mcp_tool_approval(Some(&annotations)), true);
    }

    #[test]
    fn prompt_mode_does_not_allow_session_remember() {
        assert_eq!(
            normalize_approval_decision_for_mode(
                McpToolApprovalDecision::AcceptAndRemember,
                AppToolApproval::Prompt,
            ),
            McpToolApprovalDecision::Accept
        );
    }

    #[test]
    fn custom_mcp_tool_question_mentions_server_name() {
        let metadata = metadata(
            Some(annotations(Some(false), Some(true), None)),
            None,
            Some("Run Action"),
        );
        let question = build_mcp_tool_approval_question(
            "q".to_string(),
            &approval_request("custom_server", "run_action", Some(&metadata)),
            None,
            true,
        );

        assert_eq!(question.header, "Approve app tool call?");
        assert_eq!(
            question.question,
            "The custom_server MCP server wants to run the tool \"Run Action\", which may modify or delete data. Allow this action?"
        );
        assert!(
            question
                .options
                .expect("options")
                .into_iter()
                .map(|option| option.label)
                .any(|label| label == MCP_TOOL_APPROVAL_ACCEPT_AND_REMEMBER)
        );
    }

    #[test]
    fn codex_apps_tool_question_keeps_legacy_app_label() {
        let metadata = metadata(
            Some(annotations(Some(false), Some(true), None)),
            None,
            Some("Run Action"),
        );
        let question = build_mcp_tool_approval_question(
            "q".to_string(),
            &approval_request(CODEX_APPS_MCP_SERVER_NAME, "run_action", Some(&metadata)),
            None,
            true,
        );

        assert!(
            question
                .question
                .starts_with("This app wants to run the tool \"Run Action\"")
        );
    }

    #[test]
    fn safety_monitor_result_adds_backend_reason_to_question() {
        let metadata = metadata(None, None, Some("Run Action"));
        let question = build_mcp_tool_approval_question(
            "q".to_string(),
            &approval_request("custom_server", "run_action", Some(&metadata)),
            Some(&McpToolSafetyMonitorResult {
                title: "Higher risk".to_string(),
                description: "This tool may send workspace data to an external service".to_string(),
                action_name: "Send data".to_string(),
                danger_level: McpToolDangerLevel::High,
            }),
            true,
        );

        assert_eq!(
            question.question,
            "The custom_server MCP server wants to run the tool \"Run Action\". Higher risk: This tool may send workspace data to an external service. Allow this action?"
        );
    }

    #[test]
    fn safety_monitor_url_normalizes_chatgpt_hosts() {
        assert_eq!(
            mcp_tool_safety_monitor_url_from_base_url("https://chatgpt.com/"),
            "https://chatgpt.com/backend-api/codex/safety_monitor"
        );
    }

    #[test]
    fn safety_monitor_convo_snapshot_serializes_text_messages() {
        let snapshot = mcp_tool_safety_monitor_convo_snapshot_from_items(
            "conversation-1".to_string(),
            &[
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![
                        ContentItem::InputText {
                            text: "hello".to_string(),
                        },
                        ContentItem::InputImage {
                            image_url: "https://example.com/image.png".to_string(),
                        },
                    ],
                    end_turn: None,
                    phase: None,
                },
                ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "hi there".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                },
            ],
        );

        assert_eq!(
            snapshot,
            serde_json::json!({
                "id": "conversation-1",
                "messages": [
                    {
                        "author": {
                            "role": "user",
                        },
                        "content": {
                            "content_type": "text",
                            "parts": ["hello"],
                        },
                    },
                    {
                        "author": {
                            "role": "assistant",
                        },
                        "content": {
                            "content_type": "text",
                            "parts": ["hi there"],
                        },
                    },
                ],
            })
        );
    }
}
