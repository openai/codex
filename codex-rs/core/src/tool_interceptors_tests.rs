use std::collections::HashMap;
use std::sync::Arc;

use codex_config::types::ToolInterceptorHandlerToml;
use codex_config::types::ToolInterceptorRuleToml;
use codex_config::types::ToolInterceptorsToml;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ResponseInputItem;
use codex_tools::ToolName;
use pretty_assertions::assert_eq;

use super::maybe_intercept;
use crate::session::tests::make_session_and_context;
use crate::tools::context::SharedTurnDiffTracker;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;

fn python() -> Option<std::path::PathBuf> {
    which::which("python3")
        .or_else(|_| which::which("python"))
        .ok()
}

fn tracker() -> SharedTurnDiffTracker {
    Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()))
}

#[tokio::test]
async fn function_tool_call_uses_interceptor_output() -> anyhow::Result<()> {
    let Some(python) = python() else {
        return Ok(());
    };

    let temp_dir = tempfile::tempdir()?;
    let handler_path = temp_dir.path().join("handler.py");
    std::fs::write(
        &handler_path,
        r#"
import json
import sys

request = json.load(sys.stdin)
arguments = request["payload"]["arguments_json"]
print(json.dumps({
    "output": f"{request['operation']}:{request['tool_name']}:{arguments['cmd']}",
    "success": True,
}))
"#,
    )?;

    let (session, mut turn) = make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.tool_interceptors = Some(ToolInterceptorsToml {
        handlers: HashMap::from([(
            "python".to_string(),
            ToolInterceptorHandlerToml {
                command: python.to_string_lossy().to_string(),
                args: vec![handler_path.to_string_lossy().to_string()],
                cwd: None,
                env: HashMap::new(),
            },
        )]),
        rules: vec![ToolInterceptorRuleToml {
            tool: "exec_command".to_string(),
            handler: "python".to_string(),
            operation: Some("exec".to_string()),
        }],
    });
    turn.config = Arc::new(config);

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

    let result = maybe_intercept(&invocation)
        .await?
        .expect("interceptor should match");
    let response = result.into_response();

    let ResponseInputItem::FunctionCallOutput { call_id, output } = response else {
        panic!("expected function call output");
    };
    assert_eq!(call_id, "call-1");
    assert_eq!(output.success, Some(true));
    assert_eq!(
        output.body,
        FunctionCallOutputBody::Text("exec:exec_command:date".to_string())
    );

    Ok(())
}

#[tokio::test]
async fn mcp_tool_call_uses_interceptor_mcp_result() -> anyhow::Result<()> {
    let Some(python) = python() else {
        return Ok(());
    };

    let temp_dir = tempfile::tempdir()?;
    let handler_path = temp_dir.path().join("handler.py");
    std::fs::write(
        &handler_path,
        r#"
import json
import sys

request = json.load(sys.stdin)
print(json.dumps({
    "mcp_result": {
        "content": [{
            "type": "text",
            "text": f"fake gmail {request['payload']['arguments_json']['query']}",
        }],
        "isError": False,
    }
}))
"#,
    )?;

    let tool_name = ToolName::namespaced("mcp__codex_apps__gmail", "_search_emails");
    let (session, mut turn) = make_session_and_context().await;
    let mut config = (*turn.config).clone();
    config.tool_interceptors = Some(ToolInterceptorsToml {
        handlers: HashMap::from([(
            "python".to_string(),
            ToolInterceptorHandlerToml {
                command: python.to_string_lossy().to_string(),
                args: vec![handler_path.to_string_lossy().to_string()],
                cwd: None,
                env: HashMap::new(),
            },
        )]),
        rules: vec![ToolInterceptorRuleToml {
            tool: tool_name.display(),
            handler: "python".to_string(),
            operation: None,
        }],
    });
    turn.config = Arc::new(config);

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

    let result = maybe_intercept(&invocation)
        .await?
        .expect("interceptor should match");
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
