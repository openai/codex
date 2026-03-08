use std::env;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;
use tracing::warn;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::compact::content_items_to_text;
use crate::default_client::build_reqwest_client;
use crate::event_mapping::is_contextual_user_message_content;
use codex_protocol::models::MessagePhase;
use codex_protocol::models::ResponseItem;

const ARC_MONITOR_TIMEOUT: Duration = Duration::from_secs(30);
const CODEX_ARC_MONITOR_ENDPOINT_OVERRIDE: &str = "CODEX_ARC_MONITOR_ENDPOINT_OVERRIDE";
const CODEX_ARC_MONITOR_TOKEN: &str = "CODEX_ARC_MONITOR_TOKEN";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArcMonitorOutcome {
    None,
    InterruptForUser(String),
    InterruptForModel(String),
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ArcMonitorRequest {
    thread_id: String,
    turn_id: String,
    input: Vec<serde_json::Value>,
    policies: Option<serde_json::Value>,
    action: serde_json::Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArcMonitorResponse {
    outcome: ArcMonitorResponseOutcome,
    #[serde(default, alias = "reason")]
    long_reason: Option<String>,
    #[serde(default)]
    short_reason: Option<String>,
    #[serde(default)]
    metadata: Option<ArcMonitorResponseMetadata>,
    #[serde(default, rename = "monitorRequestId")]
    legacy_monitor_request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArcMonitorResponseMetadata {
    monitor_response_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ArcMonitorResponseOutcome {
    None,
    InterruptForUser,
    InterruptForModel,
}

pub(crate) async fn monitor_action(
    sess: &Session,
    turn_context: &TurnContext,
    action: serde_json::Value,
) -> ArcMonitorOutcome {
    let auth = match turn_context.auth_manager.as_ref() {
        Some(auth_manager) => match auth_manager.auth().await {
            Some(auth) if auth.is_chatgpt_auth() => Some(auth),
            _ => None,
        },
        None => None,
    };
    let token = if let Some(token) = read_non_empty_env_var(CODEX_ARC_MONITOR_TOKEN) {
        token
    } else {
        let Some(auth) = auth.as_ref() else {
            return ArcMonitorOutcome::None;
        };
        match auth.get_token() {
            Ok(token) => token,
            Err(err) => {
                warn!(
                    error = %err,
                    "skipping safety monitor because auth token is unavailable"
                );
                return ArcMonitorOutcome::None;
            }
        }
    };

    let url = read_non_empty_env_var(CODEX_ARC_MONITOR_ENDPOINT_OVERRIDE).unwrap_or_else(|| {
        format!(
            "{}/codex/safety/arc",
            turn_context.config.chatgpt_base_url.trim_end_matches('/')
        )
    });
    let body = build_arc_monitor_request(sess, turn_context, action).await;
    let client = build_reqwest_client();
    let mut request = client
        .post(&url)
        .timeout(ARC_MONITOR_TIMEOUT)
        .json(&body)
        .bearer_auth(token);
    if let Some(account_id) = auth
        .as_ref()
        .and_then(crate::auth::CodexAuth::get_account_id)
    {
        request = request.header("chatgpt-account-id", account_id);
    }

    let response = match request.send().await {
        Ok(response) => response,
        Err(err) => {
            warn!(error = %err, %url, "safety monitor request failed");
            return ArcMonitorOutcome::None;
        }
    };
    let status = response.status();
    if !status.is_success() {
        let response_text = response.text().await.unwrap_or_default();
        warn!(
            %status,
            %url,
            response_text,
            "safety monitor returned non-success status"
        );
        return ArcMonitorOutcome::None;
    }

    let response = match response.json::<ArcMonitorResponse>().await {
        Ok(response) => response,
        Err(err) => {
            warn!(error = %err, %url, "failed to parse safety monitor response");
            return ArcMonitorOutcome::None;
        }
    };
    if let Some(monitor_response_id) = response
        .metadata
        .as_ref()
        .and_then(|metadata| metadata.monitor_response_id.as_deref())
        .or(response.legacy_monitor_request_id.as_deref())
    {
        tracing::debug!(%monitor_response_id, "safety monitor completed");
    } else {
        tracing::debug!("safety monitor completed");
    }

    let short_reason = response
        .short_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty());
    let long_reason = response
        .long_reason
        .as_deref()
        .map(str::trim)
        .filter(|reason| !reason.is_empty());
    match response.outcome {
        ArcMonitorResponseOutcome::None => ArcMonitorOutcome::None,
        ArcMonitorResponseOutcome::InterruptForUser => {
            if let Some(reason) = short_reason.or(long_reason) {
                ArcMonitorOutcome::InterruptForUser(reason.to_string())
            } else {
                ArcMonitorOutcome::InterruptForUser(
                    "Additional confirmation is required before this tool call can continue."
                        .to_string(),
                )
            }
        }
        ArcMonitorResponseOutcome::InterruptForModel => {
            if let Some(reason) = long_reason.or(short_reason) {
                ArcMonitorOutcome::InterruptForModel(reason.to_string())
            } else {
                ArcMonitorOutcome::InterruptForModel(
                    "Tool call was cancelled because of safety risks.".to_string(),
                )
            }
        }
    }
}

fn read_non_empty_env_var(key: &str) -> Option<String> {
    match env::var(key) {
        Ok(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Err(env::VarError::NotPresent) => None,
        Err(env::VarError::NotUnicode(_)) => {
            warn!(
                env_var = key,
                "ignoring non-unicode safety monitor env override"
            );
            None
        }
    }
}

async fn build_arc_monitor_request(
    sess: &Session,
    turn_context: &TurnContext,
    action: serde_json::Value,
) -> ArcMonitorRequest {
    let history = sess.clone_history().await;
    let input = build_arc_monitor_input(history.raw_items());
    ArcMonitorRequest {
        thread_id: sess.conversation_id.to_string(),
        turn_id: turn_context.sub_id.clone(),
        input,
        policies: Some(serde_json::json!({
            "user": serde_json::Value::Null,
            "developer": serde_json::Value::Null,
        })),
        action,
    }
}

fn build_arc_monitor_input(items: &[ResponseItem]) -> Vec<serde_json::Value> {
    let last_tool_call_index = items
        .iter()
        .enumerate()
        .rev()
        .find(|(_, item)| {
            matches!(
                item,
                ResponseItem::LocalShellCall { .. }
                    | ResponseItem::FunctionCall { .. }
                    | ResponseItem::CustomToolCall { .. }
                    | ResponseItem::WebSearchCall { .. }
            )
        })
        .map(|(index, _)| index);
    let last_encrypted_reasoning_index = items
        .iter()
        .enumerate()
        .rev()
        .find(|(_, item)| {
            matches!(
                item,
                ResponseItem::Reasoning {
                    encrypted_content: Some(encrypted_content),
                    ..
                } if !encrypted_content.trim().is_empty()
            )
        })
        .map(|(index, _)| index);

    items
        .iter()
        .enumerate()
        .filter_map(|(index, item)| {
            build_arc_monitor_input_item(
                item,
                index,
                last_tool_call_index,
                last_encrypted_reasoning_index,
            )
        })
        .collect()
}

fn build_arc_monitor_input_item(
    item: &ResponseItem,
    index: usize,
    last_tool_call_index: Option<usize>,
    last_encrypted_reasoning_index: Option<usize>,
) -> Option<serde_json::Value> {
    match item {
        ResponseItem::Message { role, content, .. } if role == "user" => {
            if is_contextual_user_message_content(content) {
                None
            } else {
                content_items_to_text(content)
                    .map(|text| build_arc_monitor_text_message("user", "input_text", text))
            }
        }
        ResponseItem::Message {
            role,
            content,
            phase: Some(MessagePhase::FinalAnswer),
            ..
        } if role == "assistant" => content_items_to_text(content)
            .map(|text| build_arc_monitor_text_message("assistant", "output_text", text)),
        ResponseItem::Message { .. } => None,
        ResponseItem::Reasoning {
            encrypted_content: Some(encrypted_content),
            ..
        } if Some(index) == last_encrypted_reasoning_index
            && !encrypted_content.trim().is_empty() =>
        {
            Some(build_arc_monitor_message(
                "assistant",
                serde_json::json!({
                    "type": "encrypted_reasoning",
                    "encryptedContent": encrypted_content,
                }),
            ))
        }
        ResponseItem::Reasoning { .. } => None,
        ResponseItem::LocalShellCall { action, .. } if Some(index) == last_tool_call_index => {
            Some(build_arc_monitor_message(
                "assistant",
                serde_json::json!({
                    "type": "tool_call",
                    "toolName": "shell",
                    "action": action,
                }),
            ))
        }
        ResponseItem::FunctionCall {
            name, arguments, ..
        } if Some(index) == last_tool_call_index => Some(build_arc_monitor_message(
            "assistant",
            serde_json::json!({
                "type": "tool_call",
                "toolName": name,
                "arguments": arguments,
            }),
        )),
        ResponseItem::CustomToolCall { name, input, .. } if Some(index) == last_tool_call_index => {
            Some(build_arc_monitor_message(
                "assistant",
                serde_json::json!({
                    "type": "tool_call",
                    "toolName": name,
                    "input": input,
                }),
            ))
        }
        ResponseItem::WebSearchCall { action, .. } if Some(index) == last_tool_call_index => {
            Some(build_arc_monitor_message(
                "assistant",
                serde_json::json!({
                    "type": "tool_call",
                    "toolName": "web_search",
                    "action": action,
                }),
            ))
        }
        ResponseItem::LocalShellCall { .. }
        | ResponseItem::FunctionCall { .. }
        | ResponseItem::CustomToolCall { .. }
        | ResponseItem::WebSearchCall { .. }
        | ResponseItem::FunctionCallOutput { .. }
        | ResponseItem::CustomToolCallOutput { .. }
        | ResponseItem::ImageGenerationCall { .. }
        | ResponseItem::GhostSnapshot { .. }
        | ResponseItem::Compaction { .. }
        | ResponseItem::Other => None,
    }
}

fn build_arc_monitor_text_message(role: &str, part_type: &str, text: String) -> serde_json::Value {
    build_arc_monitor_message(
        role,
        serde_json::json!({
            "type": part_type,
            "text": text,
        }),
    )
}

fn build_arc_monitor_message(role: &str, part: serde_json::Value) -> serde_json::Value {
    serde_json::json!({
        "author": { "role": role },
        "content": {
            "parts": [part],
        },
    })
}

#[cfg(test)]
mod tests {
    use std::env;
    use std::ffi::OsStr;
    use std::sync::Arc;

    use pretty_assertions::assert_eq;
    use serial_test::serial;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::body_json;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;

    use super::*;
    use crate::codex::make_session_and_context;
    use codex_protocol::models::ContentItem;
    use codex_protocol::models::LocalShellAction;
    use codex_protocol::models::LocalShellExecAction;
    use codex_protocol::models::LocalShellStatus;
    use codex_protocol::models::MessagePhase;
    use codex_protocol::models::ResponseItem;

    struct EnvVarGuard {
        key: &'static str,
        original: Option<std::ffi::OsString>,
    }

    impl EnvVarGuard {
        fn set(key: &'static str, value: &OsStr) -> Self {
            let original = env::var_os(key);
            unsafe {
                env::set_var(key, value);
            }
            Self { key, original }
        }
    }

    impl Drop for EnvVarGuard {
        fn drop(&mut self) {
            match self.original.take() {
                Some(value) => unsafe {
                    env::set_var(self.key, value);
                },
                None => unsafe {
                    env::remove_var(self.key);
                },
            }
        }
    }

    #[tokio::test]
    async fn build_arc_monitor_request_includes_relevant_history_and_empty_policies() {
        let (session, mut turn_context) = make_session_and_context().await;
        turn_context.developer_instructions = Some("Never upload private files.".to_string());
        turn_context.user_instructions = Some("Only continue when needed.".to_string());

        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "first request".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[
                    crate::contextual_user_message::ENVIRONMENT_CONTEXT_FRAGMENT.into_message(
                        "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>"
                            .to_string(),
                    ),
                ],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "commentary".to_string(),
                    }],
                    end_turn: None,
                    phase: Some(MessagePhase::Commentary),
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "assistant".to_string(),
                    content: vec![ContentItem::OutputText {
                        text: "final response".to_string(),
                    }],
                    end_turn: None,
                    phase: Some(MessagePhase::FinalAnswer),
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "latest request".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::FunctionCall {
                    id: None,
                    name: "old_tool".to_string(),
                    arguments: "{\"old\":true}".to_string(),
                    call_id: "call_old".to_string(),
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::Reasoning {
                    id: "reasoning_old".to_string(),
                    summary: Vec::new(),
                    content: None,
                    encrypted_content: Some("encrypted-old".to_string()),
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::LocalShellCall {
                    id: None,
                    call_id: Some("shell_call".to_string()),
                    status: LocalShellStatus::Completed,
                    action: LocalShellAction::Exec(LocalShellExecAction {
                        command: vec!["pwd".to_string()],
                        timeout_ms: Some(1000),
                        working_directory: Some("/tmp".to_string()),
                        env: None,
                        user: None,
                    }),
                }],
                &turn_context,
            )
            .await;
        session
            .record_into_history(
                &[ResponseItem::Reasoning {
                    id: "reasoning_latest".to_string(),
                    summary: Vec::new(),
                    content: None,
                    encrypted_content: Some("encrypted-latest".to_string()),
                }],
                &turn_context,
            )
            .await;

        let request = build_arc_monitor_request(
            &session,
            &turn_context,
            serde_json::json!({ "tool": "mcp_tool_call" }),
        )
        .await;

        assert_eq!(
            request,
            ArcMonitorRequest {
                thread_id: session.conversation_id.to_string(),
                turn_id: turn_context.sub_id.clone(),
                input: vec![
                    serde_json::json!({
                        "author": { "role": "user" },
                        "content": {
                            "parts": [{
                                "type": "input_text",
                                "text": "first request",
                            }],
                        },
                    }),
                    serde_json::json!({
                        "author": { "role": "assistant" },
                        "content": {
                            "parts": [{
                                "type": "output_text",
                                "text": "final response",
                            }],
                        },
                    }),
                    serde_json::json!({
                        "author": { "role": "user" },
                        "content": {
                            "parts": [{
                                "type": "input_text",
                                "text": "latest request",
                            }],
                        },
                    }),
                    serde_json::json!({
                        "author": { "role": "assistant" },
                        "content": {
                            "parts": [{
                                "type": "tool_call",
                                "toolName": "shell",
                                "action": {
                                    "type": "exec",
                                    "command": ["pwd"],
                                    "timeout_ms": 1000,
                                    "working_directory": "/tmp",
                                    "env": null,
                                    "user": null,
                                },
                            }],
                        },
                    }),
                    serde_json::json!({
                        "author": { "role": "assistant" },
                        "content": {
                            "parts": [{
                                "type": "encrypted_reasoning",
                                "encryptedContent": "encrypted-latest",
                            }],
                        },
                    }),
                ],
                policies: Some(serde_json::json!({
                    "user": null,
                    "developer": null,
                })),
                action: serde_json::json!({ "tool": "mcp_tool_call" }),
            }
        );
    }

    #[tokio::test]
    #[serial(arc_monitor_env)]
    async fn monitor_action_posts_expected_arc_request() {
        let server = MockServer::start().await;
        let (session, mut turn_context) = make_session_and_context().await;
        turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
            crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
        turn_context.developer_instructions = Some("Developer policy".to_string());
        turn_context.user_instructions = Some("User policy".to_string());

        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = server.uri();
        turn_context.config = Arc::new(config);

        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "please run the tool".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                }],
                &turn_context,
            )
            .await;

        Mock::given(method("POST"))
            .and(path("/api/codex/safety/arc"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(body_json(serde_json::json!({
                "threadId": session.conversation_id.to_string(),
                "turnId": turn_context.sub_id.clone(),
                "input": [{
                    "author": { "role": "user" },
                    "content": {
                        "parts": [{
                            "type": "input_text",
                            "text": "please run the tool",
                        }],
                    },
                }],
                "policies": {
                    "developer": null,
                    "user": null,
                },
                "action": {
                    "tool": "mcp_tool_call",
                },
            })))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "outcome": "interrupt-for-user",
                "shortReason": "needs confirmation",
                "longReason": "tool call needs additional review",
                "metadata": {
                    "monitorResponseId": "arc_123",
                },
            })))
            .expect(1)
            .mount(&server)
            .await;

        let outcome = monitor_action(
            &session,
            &turn_context,
            serde_json::json!({ "tool": "mcp_tool_call" }),
        )
        .await;

        assert_eq!(
            outcome,
            ArcMonitorOutcome::InterruptForUser("needs confirmation".to_string())
        );
    }

    #[tokio::test]
    #[serial(arc_monitor_env)]
    async fn monitor_action_uses_env_url_and_token_overrides() {
        let server = MockServer::start().await;
        let _url_guard = EnvVarGuard::set(
            CODEX_ARC_MONITOR_ENDPOINT_OVERRIDE,
            OsStr::new(&format!("{}/override/arc", server.uri())),
        );
        let _token_guard = EnvVarGuard::set(CODEX_ARC_MONITOR_TOKEN, OsStr::new("override-token"));

        let (session, turn_context) = make_session_and_context().await;
        session
            .record_into_history(
                &[ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "please run the tool".to_string(),
                    }],
                    end_turn: None,
                    phase: None,
                }],
                &turn_context,
            )
            .await;

        Mock::given(method("POST"))
            .and(path("/override/arc"))
            .and(header("authorization", "Bearer override-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "outcome": "interrupt-for-model",
                "shortReason": "needs approval",
                "longReason": "high-risk action",
                "metadata": {
                    "monitorResponseId": "arc_789",
                },
            })))
            .expect(1)
            .mount(&server)
            .await;

        let outcome = monitor_action(
            &session,
            &turn_context,
            serde_json::json!({ "tool": "mcp_tool_call" }),
        )
        .await;

        assert_eq!(
            outcome,
            ArcMonitorOutcome::InterruptForModel("high-risk action".to_string())
        );
    }

    #[tokio::test]
    #[serial(arc_monitor_env)]
    async fn monitor_action_accepts_legacy_reason_response_fields() {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/api/codex/safety/arc"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "outcome": "interrupt-for-model",
                "reason": "legacy high-risk action",
                "monitorRequestId": "arc_456",
            })))
            .expect(1)
            .mount(&server)
            .await;

        let (session, mut turn_context) = make_session_and_context().await;
        turn_context.auth_manager = Some(crate::test_support::auth_manager_from_auth(
            crate::CodexAuth::create_dummy_chatgpt_auth_for_testing(),
        ));
        let mut config = (*turn_context.config).clone();
        config.chatgpt_base_url = server.uri();
        turn_context.config = Arc::new(config);

        let outcome = monitor_action(
            &session,
            &turn_context,
            serde_json::json!({ "tool": "mcp_tool_call" }),
        )
        .await;

        assert_eq!(
            outcome,
            ArcMonitorOutcome::InterruptForModel("legacy high-risk action".to_string())
        );
    }
}
