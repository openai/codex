use std::sync::Arc;

use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::maybe_intercept;
use crate::session::tests::make_session_and_context;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;

fn tracker() -> SharedTurnDiffTracker {
    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()))
}

async fn turn_with_interceptor(
    interceptor_url: String,
) -> (
    crate::session::session::Session,
    crate::session::turn_context::TurnContext,
) {
    let (session, mut turn) = make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.tool_interceptor = Some(interceptor_url);
    turn.config = Arc::new(config);
    (session, turn)
}

#[tokio::test]
async fn function_tool_call_uses_interceptor_output() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tool-call"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "protocol_version": 1,
            "action": "replace",
            "result": {
                "type": "text",
                "text": "synthetic exec output",
                "success": true,
            },
        })))
        .mount(&server)
        .await;

    let (session, turn) = turn_with_interceptor(server.uri()).await;
    let invocation = ToolInvocation {
        session: Arc::new(session),
        turn: Arc::new(turn),
        tracker: tracker(),
        call_id: "call-1".to_string(),
        tool_name: ToolName::plain("exec_command"),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({"cmd": "date"}).to_string(),
        },
    };

    let result = maybe_intercept(&invocation, None)
        .await?
        .expect("interceptor should replace");
    let response = result.into_response();

    let ResponseInputItem::FunctionCallOutput { call_id, output } = response else {
        panic!("expected function call output");
    };
    assert_eq!(call_id, "call-1");
    assert_eq!(output.success, Some(true));
    assert_eq!(
        output.body,
        FunctionCallOutputBody::Text("synthetic exec output".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn tool_call_passes_through_when_interceptor_passes() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tool-call"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "protocol_version": 1,
            "action": "continue",
        })))
        .mount(&server)
        .await;

    let (session, turn) = turn_with_interceptor(server.uri()).await;
    let invocation = ToolInvocation {
        session: Arc::new(session),
        turn: Arc::new(turn),
        tracker: tracker(),
        call_id: "call-pass".to_string(),
        tool_name: ToolName::plain("exec_command"),
        payload: ToolPayload::Function {
            arguments: serde_json::json!({"cmd": "date"}).to_string(),
        },
    };

    assert!(maybe_intercept(&invocation, None).await?.is_none());
    Ok(())
}

#[tokio::test]
async fn mcp_tool_call_uses_interceptor_mcp_result() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/tool-call"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "protocol_version": 1,
            "action": "replace",
            "result": {
                "type": "mcp",
                "value": {
                    "content": [{
                        "type": "text",
                        "text": "fake gmail in:inbox",
                    }],
                    "isError": false,
                },
            },
        })))
        .mount(&server)
        .await;

    let tool_name = ToolName::namespaced("mcp__codex_apps__gmail", "_search_emails");
    let (session, turn) = turn_with_interceptor(server.uri()).await;
    let invocation = ToolInvocation {
        session: Arc::new(session),
        turn: Arc::new(turn),
        tracker: tracker(),
        call_id: "mcp-call-1".to_string(),
        tool_name,
        payload: ToolPayload::Mcp {
            server: "codex_apps".to_string(),
            tool: "gmail.search".to_string(),
            raw_arguments: serde_json::json!({"query": "in:inbox"}).to_string(),
        },
    };

    let result = maybe_intercept(&invocation, None)
        .await?
        .expect("interceptor should replace");
    let response = result.into_response();

    let ResponseInputItem::FunctionCallOutput { call_id, output } = response else {
        panic!("expected function call output");
    };
    assert_eq!(call_id, "mcp-call-1");
    assert_eq!(output.success, Some(true));
    let Some(text) = output.body.to_text() else {
        panic!("expected text output");
    };
    assert!(text.contains("Wall time: "));
    assert!(text.contains("fake gmail in:inbox"));

    Ok(())
}
