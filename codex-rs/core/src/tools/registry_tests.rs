use super::*;
use crate::tools::context::McpToolOutput;
use crate::tools::context::ModelVisibleRewriteOutput;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::goal_spec::GET_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_get_goal_tool;
use codex_protocol::mcp::CallToolResult;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::time::Duration;

struct TestHandler {
    tool_name: codex_tools::ToolName,
}

impl ToolHandler for TestHandler {
    type Output = crate::tools::context::FunctionToolOutput;

    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        Ok(crate::tools::context::FunctionToolOutput::from_text(
            "ok".to_string(),
            Some(true),
        ))
    }
}

#[test]
fn handler_looks_up_namespaced_aliases_explicitly() {
    let namespace = "mcp__codex_apps__gmail";
    let tool_name = "gmail_get_recent_emails";
    let plain_name = codex_tools::ToolName::plain(tool_name);
    let namespaced_name = codex_tools::ToolName::namespaced(namespace, tool_name);
    let plain_handler = Arc::new(TestHandler {
        tool_name: plain_name.clone(),
    }) as Arc<dyn AnyToolHandler>;
    let namespaced_handler = Arc::new(TestHandler {
        tool_name: namespaced_name.clone(),
    }) as Arc<dyn AnyToolHandler>;
    let registry = ToolRegistry::new(HashMap::from([
        (plain_name.clone(), Arc::clone(&plain_handler)),
        (namespaced_name.clone(), Arc::clone(&namespaced_handler)),
    ]));

    let plain = registry.handler(&plain_name);
    let namespaced = registry.handler(&namespaced_name);
    let missing_namespaced = registry.handler(&codex_tools::ToolName::namespaced(
        "mcp__codex_apps__calendar",
        tool_name,
    ));

    assert_eq!(plain.is_some(), true);
    assert_eq!(namespaced.is_some(), true);
    assert_eq!(missing_namespaced.is_none(), true);
    assert!(
        plain
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &plain_handler))
    );
    assert!(
        namespaced
            .as_ref()
            .is_some_and(|handler| Arc::ptr_eq(handler, &namespaced_handler))
    );
}

#[test]
fn model_visible_rewrite_preserves_code_mode_result() {
    let result = mcp_result_with_model_visible_rewrite();

    match result.into_response() {
        ResponseInputItem::FunctionCallOutput { call_id, output } => {
            assert_eq!(call_id, "mcp-call-1");
            assert_eq!(
                output.body.to_text().as_deref(),
                Some(r#"{"echo":"rewritten"}"#)
            );
        }
        other => panic!("expected FunctionCallOutput, got {other:?}"),
    }

    assert_eq!(
        mcp_result_with_model_visible_rewrite().code_mode_result(),
        json!({
            "content": [],
            "structuredContent": {
                "echo": "original",
            },
            "isError": false,
        })
    );
}

fn mcp_result_with_model_visible_rewrite() -> AnyToolResult {
    AnyToolResult {
        call_id: "mcp-call-1".to_string(),
        payload: ToolPayload::Function {
            arguments: "{}".to_string(),
        },
        result: Box::new(ModelVisibleRewriteOutput::new(
            Box::new(McpToolOutput {
                result: CallToolResult {
                    content: Vec::new(),
                    structured_content: Some(json!({ "echo": "original" })),
                    is_error: Some(false),
                    meta: None,
                },
                tool_input: json!({}),
                wall_time: Duration::ZERO,
                original_image_detail_supported: false,
                truncation_policy: codex_utils_output_truncation::TruncationPolicy::Bytes(1024),
            }),
            json!({ "echo": "rewritten" }),
        )),
        post_tool_use_payload: None,
    }
}

#[test]
fn register_handler_adds_handler_and_spec() {
    let mut builder = ToolRegistryBuilder::new();
    builder.register_handler(Arc::new(GetGoalHandler));

    let (specs, registry) = builder.build();

    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0], create_get_goal_tool());
    assert!(registry.has_handler(&codex_tools::ToolName::plain(GET_GOAL_TOOL_NAME)));
}
