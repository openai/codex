use super::*;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::goal_spec::GET_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_get_goal_tool;
use pretty_assertions::assert_eq;
use serde_json::json;

struct TestHandler {
    tool_name: codex_tools::ToolName,
}

impl ToolHandler for TestHandler {
    type Output = crate::tools::context::FunctionToolOutput;

    fn tool_name(&self) -> codex_tools::ToolName {
        self.tool_name.clone()
    }

    fn kind(&self) -> ToolKind {
        ToolKind::Function
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
fn register_handler_adds_handler_and_augments_specs_for_code_mode() {
    let mut builder = ToolRegistryBuilder::new(/*code_mode_enabled*/ true);
    builder.register_handler(Arc::new(GetGoalHandler));

    let (specs, registry) = builder.build();

    assert_eq!(specs.len(), 1);
    assert_eq!(
        specs[0].spec,
        codex_tools::augment_tool_spec_for_code_mode(create_get_goal_tool())
    );
    assert!(registry.has_handler(&codex_tools::ToolName::plain(GET_GOAL_TOOL_NAME)));
}

struct StubExtensionExecutor;

impl codex_tool_api::ToolExecutor for StubExtensionExecutor {
    fn execute<'a>(&'a self, _call: codex_tool_api::ToolCall) -> codex_tool_api::ToolFuture<'a> {
        Box::pin(async { Ok(json!({ "ok": true })) })
    }
}

#[tokio::test]
async fn bundled_tool_handler_exposes_generic_hook_payloads_and_is_conservatively_mutating() {
    let bundle = codex_tool_api::ToolBundle::new(
        codex_tool_api::FunctionToolSpec {
            name: "extension_echo".to_string(),
            description: "Echoes arguments.".to_string(),
            strict: true,
            parameters: json!({
                "type": "object",
                "properties": {
                    "message": { "type": "string" },
                },
                "required": ["message"],
                "additionalProperties": false,
            }),
        },
        Arc::new(StubExtensionExecutor),
    );
    let spec = extension_tool_spec(bundle.spec()).expect("extension spec should convert");
    let handler = BundledToolHandler::new(bundle, spec);
    let (session, turn) = crate::session::tests::make_session_and_context().await;
    let invocation = ToolInvocation {
        session: session.into(),
        turn: turn.into(),
        cancellation_token: tokio_util::sync::CancellationToken::new(),
        tracker: Arc::new(tokio::sync::Mutex::new(
            crate::turn_diff_tracker::TurnDiffTracker::new(),
        )),
        call_id: "call-extension".to_string(),
        tool_name: codex_tools::ToolName::plain("extension_echo"),
        source: crate::tools::context::ToolCallSource::Direct,
        payload: ToolPayload::Function {
            arguments: json!({ "message": "hello" }).to_string(),
        },
    };
    let output = BundledToolOutput {
        value: json!({ "ok": true }),
    };

    assert!(ToolHandler::is_mutating(&handler, &invocation).await);
    assert_eq!(
        ToolHandler::pre_tool_use_payload(&handler, &invocation),
        Some(PreToolUsePayload {
            tool_name: HookToolName::new("extension_echo"),
            tool_input: json!({ "message": "hello" }),
        })
    );
    assert_eq!(
        ToolHandler::post_tool_use_payload(&handler, &invocation, &output),
        Some(PostToolUsePayload {
            tool_name: HookToolName::new("extension_echo"),
            tool_use_id: "call-extension".to_string(),
            tool_input: json!({ "message": "hello" }),
            tool_response: json!({ "ok": true }),
        })
    );
}
