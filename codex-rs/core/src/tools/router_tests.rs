use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;

use crate::codex::make_session_and_context;
use crate::function_tool::FunctionCallError;
use crate::mcp_tool_exposure::McpToolExposure;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_mcp::ToolInfo;
use codex_protocol::models::ResponseItem;
use codex_tools::ToolName;

use super::ToolCall;
use super::ToolCallSource;
use super::ToolRouter;
use super::ToolRouterParams;

fn mcp_tool_exposure(
    direct_tools: Option<HashMap<String, ToolInfo>>,
    deferred_tools: Option<HashMap<String, ToolInfo>>,
) -> McpToolExposure {
    McpToolExposure {
        direct_tools,
        deferred_tools,
        unavailable_called_tools: Vec::new(),
    }
}

#[tokio::test]
async fn js_repl_tools_only_blocks_direct_tool_calls() -> anyhow::Result<()> {
    let (session, mut turn) = make_session_and_context().await;
    turn.tools_config.js_repl_tools_only = true;

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let deferred_mcp_tools = Some(mcp_tools.clone());
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            mcp_tool_exposure: mcp_tool_exposure(Some(mcp_tools), deferred_mcp_tools),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
    );

    let call = ToolCall {
        tool_name: ToolName::plain("shell"),
        call_id: "call-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    };
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let err = router
        .dispatch_tool_call_with_code_mode_result(
            session,
            turn,
            tracker,
            call,
            ToolCallSource::Direct,
        )
        .await
        .err()
        .expect("direct tool calls should be blocked");
    let FunctionCallError::RespondToModel(message) = err else {
        panic!("expected RespondToModel, got {err:?}");
    };
    assert!(message.contains("direct tool calls are disabled"));

    Ok(())
}

#[tokio::test]
async fn js_repl_tools_only_allows_js_repl_source_calls() -> anyhow::Result<()> {
    let (session, mut turn) = make_session_and_context().await;
    turn.tools_config.js_repl_tools_only = true;

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let deferred_mcp_tools = Some(mcp_tools.clone());
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            mcp_tool_exposure: mcp_tool_exposure(Some(mcp_tools), deferred_mcp_tools),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
    );

    let call = ToolCall {
        tool_name: ToolName::plain("shell"),
        call_id: "call-2".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    };
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let err = router
        .dispatch_tool_call_with_code_mode_result(
            session,
            turn,
            tracker,
            call,
            ToolCallSource::JsRepl,
        )
        .await
        .err()
        .expect("shell call with empty args should fail");
    let message = err.to_string();
    assert!(
        !message.contains("direct tool calls are disabled"),
        "js_repl source should bypass direct-call policy gate"
    );

    Ok(())
}

#[tokio::test]
async fn js_repl_tools_only_blocks_namespaced_js_repl_tool() -> anyhow::Result<()> {
    let (session, mut turn) = make_session_and_context().await;
    turn.tools_config.js_repl_tools_only = true;

    let session = Arc::new(session);
    let turn = Arc::new(turn);
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            mcp_tool_exposure: mcp_tool_exposure(
                /*direct_tools*/ None, /*deferred_tools*/ None,
            ),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
    );

    let call = ToolCall {
        tool_name: ToolName::namespaced("mcp__server__", "js_repl"),
        call_id: "call-namespaced-js-repl".to_string(),
        payload: ToolPayload::Mcp {
            server: "server".to_string(),
            tool: "js_repl".to_string(),
            raw_arguments: "{}".to_string(),
        },
    };
    let tracker = Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new()));
    let err = router
        .dispatch_tool_call_with_code_mode_result(
            session,
            turn,
            tracker,
            call,
            ToolCallSource::Direct,
        )
        .await
        .err()
        .expect("namespaced js_repl calls should be blocked");
    let FunctionCallError::RespondToModel(message) = err else {
        panic!("expected RespondToModel, got {err:?}");
    };
    assert!(message.contains("direct tool calls are disabled"));

    Ok(())
}

#[tokio::test]
async fn parallel_support_does_not_match_namespaced_local_tool_names() -> anyhow::Result<()> {
    let (session, turn) = make_session_and_context().await;
    let mcp_tools = session
        .services
        .mcp_connection_manager
        .read()
        .await
        .list_all_tools()
        .await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            mcp_tool_exposure: mcp_tool_exposure(Some(mcp_tools), /*deferred_tools*/ None),
            parallel_mcp_server_names: HashSet::new(),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
    );

    let parallel_tool_name = ["shell", "local_shell", "exec_command", "shell_command"]
        .into_iter()
        .find(|name| {
            router.tool_supports_parallel(&ToolCall {
                tool_name: ToolName::plain(*name),
                call_id: "call-parallel-tool".to_string(),
                payload: ToolPayload::Function {
                    arguments: "{}".to_string(),
                },
            })
        })
        .expect("test session should expose a parallel shell-like tool");

    assert!(!router.tool_supports_parallel(&ToolCall {
        tool_name: ToolName::namespaced("mcp__server__", parallel_tool_name),
        call_id: "call-namespaced-tool".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    }));

    Ok(())
}

#[tokio::test]
async fn build_tool_call_uses_namespace_for_registry_name() -> anyhow::Result<()> {
    let (session, _) = make_session_and_context().await;
    let session = Arc::new(session);
    let tool_name = "create_event".to_string();

    let call = ToolRouter::build_tool_call(
        &session,
        ResponseItem::FunctionCall {
            id: None,
            name: tool_name.clone(),
            namespace: Some("mcp__codex_apps__calendar".to_string()),
            arguments: "{}".to_string(),
            call_id: "call-namespace".to_string(),
        },
    )
    .await?
    .expect("function_call should produce a tool call");

    assert_eq!(
        call.tool_name,
        ToolName::namespaced("mcp__codex_apps__calendar", tool_name)
    );
    assert_eq!(call.call_id, "call-namespace");
    match call.payload {
        ToolPayload::Function { arguments } => {
            assert_eq!(arguments, "{}");
        }
        other => panic!("expected function payload, got {other:?}"),
    }

    Ok(())
}

#[tokio::test]
async fn mcp_parallel_support_uses_exact_payload_server() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_config(
        &turn.tools_config,
        ToolRouterParams {
            mcp_tool_exposure: mcp_tool_exposure(
                /*direct_tools*/ None, /*deferred_tools*/ None,
            ),
            parallel_mcp_server_names: HashSet::from(["echo".to_string()]),
            discoverable_tools: None,
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
    );

    let deferred_call = ToolCall {
        tool_name: ToolName::namespaced("mcp__echo__", "query_with_delay"),
        call_id: "call-deferred".to_string(),
        payload: ToolPayload::Mcp {
            server: "echo".to_string(),
            tool: "query_with_delay".to_string(),
            raw_arguments: "{}".to_string(),
        },
    };
    assert!(router.tool_supports_parallel(&deferred_call));

    let different_server_call = ToolCall {
        tool_name: ToolName::namespaced("mcp__hello_echo__", "query_with_delay"),
        call_id: "call-other-server".to_string(),
        payload: ToolPayload::Mcp {
            server: "hello_echo".to_string(),
            tool: "query_with_delay".to_string(),
            raw_arguments: "{}".to_string(),
        },
    };
    assert!(!router.tool_supports_parallel(&different_server_call));

    Ok(())
}
