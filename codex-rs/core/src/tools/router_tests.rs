use std::sync::Arc;

use crate::config::Config;
use crate::session::tests::make_session_and_context;
use crate::tools::context::ToolPayload;
use crate::turn_diff_tracker::TurnDiffTracker;
use codex_extension_api::ExtensionData;
use codex_extension_api::ExtensionRegistry;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall as ExtensionToolCall;
use codex_extension_api::ToolExecutor;
use codex_protocol::dynamic_tools::DynamicToolFunctionSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceSpec;
use codex_protocol::dynamic_tools::DynamicToolNamespaceTool;
use codex_protocol::dynamic_tools::DynamicToolSpec;
use codex_protocol::models::ContentItem;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use codex_tools::default_namespace_description;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::ToolCall;
use super::ToolCallSource;
use super::ToolRouter;
use super::ToolRouterParams;
use super::extension_tool_executors;

struct ExtensionEchoContributor;

impl codex_extension_api::ToolContributor for ExtensionEchoContributor {
    fn tools(
        &self,
        _session_store: &ExtensionData,
        _thread_store: &ExtensionData,
    ) -> Vec<Arc<dyn ToolExecutor<ExtensionToolCall>>> {
        vec![Arc::new(ExtensionEchoExecutor)]
    }
}

struct ExtensionEchoExecutor;

impl ToolExecutor<ExtensionToolCall> for ExtensionEchoExecutor {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced("extension/", "echo")
    }

    fn spec(&self) -> ToolSpec {
        ToolSpec::Namespace(ResponsesApiNamespace {
            name: "extension/".to_string(),
            description: default_namespace_description("extension/"),
            tools: vec![ResponsesApiNamespaceTool::Function(ResponsesApiTool {
                name: "echo".to_string(),
                description: "Echoes arguments through an extension tool.".to_string(),
                strict: true,
                parameters: codex_extension_api::parse_tool_input_schema(&json!({
                    "type": "object",
                    "properties": {
                        "message": { "type": "string" },
                    },
                    "required": ["message"],
                    "additionalProperties": false,
                }))
                .expect("extension schema should parse"),
                output_schema: None,
                defer_loading: None,
            })],
        })
    }

    fn handle(&self, call: ExtensionToolCall) -> codex_tools::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

impl ExtensionEchoExecutor {
    async fn handle_call(
        &self,
        call: ExtensionToolCall,
    ) -> Result<Box<dyn codex_tools::ToolOutput>, codex_tools::FunctionCallError> {
        let arguments: serde_json::Value =
            serde_json::from_str(call.function_arguments()?).expect("test arguments should parse");
        Ok(Box::new(codex_tools::JsonToolOutput::new(json!({
            "arguments": arguments,
            "callId": call.call_id,
            "conversationHistory": call.conversation_history.items(),
            "ok": true,
        }))) as Box<dyn codex_tools::ToolOutput>)
    }
}

fn extension_tool_test_registry() -> Arc<ExtensionRegistry<Config>> {
    let mut builder = ExtensionRegistryBuilder::new();
    builder.tool_contributor(Arc::new(ExtensionEchoContributor));
    Arc::new(builder.build())
}

#[tokio::test]
async fn parallel_support_does_not_match_namespaced_local_tool_names() -> anyhow::Result<()> {
    let (session, turn) = make_session_and_context().await;
    let mcp_tools = session
        .services
        .mcp_connection_manager
        .load_full()
        .list_all_tools()
        .await;
    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            tool_suggest_candidates: None,
            deferred_mcp_tools: None,
            mcp_tools: Some(mcp_tools),
            extension_tool_executors: Vec::new(),
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
        &Default::default(),
    );

    let parallel_tool_name = ["exec_command", "shell_command"]
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
    let tool_name = "create_event".to_string();

    let call = ToolRouter::build_tool_call(ResponseItem::FunctionCall {
        id: None,
        name: tool_name.clone(),
        namespace: Some("mcp__codex_apps__calendar".to_string()),
        arguments: "{}".to_string(),
        call_id: "call-namespace".to_string(),
        internal_chat_message_metadata_passthrough: None,
    })?
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
async fn mcp_parallel_support_uses_handler_data() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            tool_suggest_candidates: None,
            deferred_mcp_tools: None,
            mcp_tools: Some(vec![
                mcp_tool_info(
                    "echo",
                    /*supports_parallel_tool_calls*/ true,
                    "mcp__echo__",
                    "query_with_delay",
                ),
                mcp_tool_info(
                    "hello_echo",
                    /*supports_parallel_tool_calls*/ false,
                    "mcp__hello_echo__",
                    "query_with_delay",
                ),
            ]),
            extension_tool_executors: Vec::new(),
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
        &Default::default(),
    );

    let call = ToolCall {
        tool_name: ToolName::namespaced("mcp__echo__", "query_with_delay"),
        call_id: "call-handler".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    };
    assert!(router.tool_supports_parallel(&call));

    let different_server_call = ToolCall {
        tool_name: ToolName::namespaced("mcp__hello_echo__", "query_with_delay"),
        call_id: "call-other-server".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    };
    assert!(!router.tool_supports_parallel(&different_server_call));

    Ok(())
}

#[tokio::test]
async fn tools_without_handlers_do_not_support_parallel() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            tool_suggest_candidates: None,
            deferred_mcp_tools: None,
            mcp_tools: None,
            extension_tool_executors: Vec::new(),
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
        &Default::default(),
    );

    assert!(!router.tool_supports_parallel(&ToolCall {
        tool_name: ToolName::plain("web_search"),
        call_id: "call-web-search".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
    }));

    Ok(())
}

#[tokio::test]
async fn specs_filter_deferred_dynamic_tools() -> anyhow::Result<()> {
    let (_, turn) = make_session_and_context().await;
    let hidden_tool = "hidden_dynamic_tool";
    let visible_tool = "visible_dynamic_tool";
    let dynamic_tools = vec![DynamicToolSpec::Namespace(DynamicToolNamespaceSpec {
        name: "codex_app".to_string(),
        description: "Codex app tools.".to_string(),
        tools: vec![
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: hidden_tool.to_string(),
                description: "Hidden until discovered.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
                defer_loading: true,
            }),
            DynamicToolNamespaceTool::Function(DynamicToolFunctionSpec {
                name: visible_tool.to_string(),
                description: "Visible immediately.".to_string(),
                input_schema: json!({
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false,
                }),
                defer_loading: false,
            }),
        ],
    })];

    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            tool_suggest_candidates: None,
            deferred_mcp_tools: None,
            mcp_tools: None,
            extension_tool_executors: Vec::new(),
            dynamic_tools: &dynamic_tools,
        },
        &Default::default(),
    );

    assert_eq!(
        namespace_function_names(&router.model_visible_specs(), "codex_app"),
        vec![visible_tool.to_string()]
    );

    Ok(())
}

fn mcp_tool_info(
    server_name: &str,
    supports_parallel_tool_calls: bool,
    callable_namespace: &str,
    tool_name: &str,
) -> codex_mcp::ToolInfo {
    codex_mcp::ToolInfo {
        server_name: server_name.to_string(),
        supports_parallel_tool_calls,
        server_origin: None,
        callable_name: tool_name.to_string(),
        callable_namespace: callable_namespace.to_string(),
        namespace_description: None,
        tool: rmcp::model::Tool::new(
            tool_name.to_string(),
            "Test MCP tool",
            Arc::new(rmcp::model::object(json!({
                "type": "object",
            }))),
        ),
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
    }
}

#[tokio::test]
async fn extension_tool_executors_are_model_visible_and_dispatchable() -> anyhow::Result<()> {
    let (mut session, turn) = make_session_and_context().await;
    session.services.extensions = extension_tool_test_registry();
    let history_item = ResponseItem::Message {
        id: None,
        role: "user".to_string(),
        content: vec![ContentItem::InputText {
            text: "extension history".to_string(),
        }],
        phase: None,
        internal_chat_message_metadata_passthrough: None,
    };
    session
        .record_conversation_items(&turn, std::slice::from_ref(&history_item))
        .await;

    let router = ToolRouter::from_turn_context(
        &turn,
        ToolRouterParams {
            tool_suggest_candidates: None,
            deferred_mcp_tools: None,
            mcp_tools: None,
            extension_tool_executors: extension_tool_executors(&session),
            dynamic_tools: turn.dynamic_tools.as_slice(),
        },
        &Default::default(),
    );

    assert!(
        router.model_visible_specs().iter().any(
            |spec| matches!(spec, ToolSpec::Namespace(namespace)
            if namespace.name == "extension/"
                && namespace.tools.iter().any(|tool| matches!(
                    tool,
                    ResponsesApiNamespaceTool::Function(tool) if tool.name == "echo"
                )))
        ),
        "expected extension-provided tool to be visible to the model"
    );

    let call = ToolRouter::build_tool_call(ResponseItem::FunctionCall {
        id: None,
        name: "echo".to_string(),
        namespace: Some("extension/".to_string()),
        arguments: json!({ "message": "hello" }).to_string(),
        call_id: "call-extension".to_string(),
        internal_chat_message_metadata_passthrough: None,
    })?
    .expect("function_call should produce a tool call");
    let result = router
        .dispatch_tool_call_with_code_mode_result(
            Arc::new(session),
            Arc::new(turn),
            CancellationToken::new(),
            Arc::new(tokio::sync::Mutex::new(TurnDiffTracker::new())),
            call,
            ToolCallSource::Direct,
        )
        .await?;

    let response = result.into_response();
    match response {
        ResponseInputItem::FunctionCallOutput { call_id, output } => {
            assert_eq!(call_id, "call-extension");
            let FunctionCallOutputBody::Text(text) = output.body else {
                panic!("expected text function call output")
            };
            let value: serde_json::Value =
                serde_json::from_str(&text).expect("extension tool output should be json");
            assert_eq!(
                value,
                json!({
                    "arguments": { "message": "hello" },
                    "callId": "call-extension",
                    "conversationHistory": [history_item],
                    "ok": true,
                })
            );
        }
        other => panic!("expected function call output, got {other:?}"),
    }

    Ok(())
}

fn namespace_function_names(specs: &[ToolSpec], namespace_name: &str) -> Vec<String> {
    specs
        .iter()
        .find_map(|spec| match spec {
            ToolSpec::Namespace(namespace) if namespace.name == namespace_name => Some(
                namespace
                    .tools
                    .iter()
                    .map(|tool| match tool {
                        ResponsesApiNamespaceTool::Function(tool) => tool.name.clone(),
                    })
                    .collect(),
            ),
            ToolSpec::Function(_)
            | ToolSpec::Freeform(_)
            | ToolSpec::ToolSearch { .. }
            | ToolSpec::ImageGeneration { .. }
            | ToolSpec::WebSearch { .. }
            | ToolSpec::Namespace(_) => None,
        })
        .unwrap_or_default()
}
