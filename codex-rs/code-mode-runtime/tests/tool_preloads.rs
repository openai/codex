//! Each session selects its helpers independently and exposes them only with their tools.

use std::sync::Arc;
use std::sync::Mutex;
use std::sync::PoisonError;
use std::time::Duration;

use codex_code_mode_runtime::CellId;
use codex_code_mode_runtime::CodeModeNestedToolCall;
use codex_code_mode_runtime::CodeModeSessionDelegate;
use codex_code_mode_runtime::CodeModeToolKind;
use codex_code_mode_runtime::CodeModeToolResult;
use codex_code_mode_runtime::ExecuteRequest;
use codex_code_mode_runtime::FunctionCallOutputContentItem;
use codex_code_mode_runtime::InProcessCodeModeSession;
use codex_code_mode_runtime::NoopCodeModeSessionDelegate;
use codex_code_mode_runtime::NotificationFuture;
use codex_code_mode_runtime::RuntimeResponse;
use codex_code_mode_runtime::ToolDefinition;
use codex_code_mode_runtime::ToolInvocationFuture;
use codex_code_mode_runtime::ToolPreload;
use codex_protocol::ToolName;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio_util::sync::CancellationToken;

const SEND_BINDING: &str = "mcp__codex_apps__user_message_send_message";
const SEND_NAMESPACE: &str = "mcp__codex_apps";

#[derive(Default)]
struct RecordingDelegate(Mutex<Vec<ToolName>>);

impl CodeModeSessionDelegate for RecordingDelegate {
    fn invoke_tool<'a>(
        &'a self,
        invocation: CodeModeNestedToolCall,
        _cancellation_token: CancellationToken,
    ) -> ToolInvocationFuture<'a> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(invocation.tool_name);
        Box::pin(async { Ok(CodeModeToolResult::from(json!({"ok": true}))) })
    }

    fn notify<'a>(
        &'a self,
        _call_id: String,
        _cell_id: CellId,
        _text: String,
        _cancellation_token: CancellationToken,
    ) -> NotificationFuture<'a> {
        Box::pin(async { Ok(()) })
    }

    fn cell_closed(&self, _cell_id: &CellId) {}
}

fn definition(binding: &str, tool_name: ToolName) -> ToolDefinition {
    ToolDefinition {
        name: binding.to_string(),
        tool_name,
        description: String::new(),
        kind: CodeModeToolKind::Function,
        input_schema: None,
        input_schema_max_bytes: None,
        output_schema: None,
    }
}

async fn execute(
    session: &InProcessCodeModeSession,
    tools: Vec<ToolDefinition>,
    source: &str,
    delegate: Arc<dyn CodeModeSessionDelegate>,
) -> Result<(CellId, RuntimeResponse), String> {
    let started = session
        .execute(
            ExecuteRequest {
                tool_call_id: "test".to_string(),
                enabled_tools: tools,
                source: source.to_string(),
                yield_time_ms: None,
                max_output_tokens: None,
            },
            delegate,
            /*preempt*/ None,
        )
        .await?;
    let cell_id = started.cell_id.clone();
    Ok((cell_id, started.initial_response().await?))
}

fn result(cell_id: CellId, outcome: Result<&str, &str>) -> RuntimeResponse {
    let (content_items, error_text) = match outcome {
        Ok(text) => (
            vec![FunctionCallOutputContentItem::InputText {
                text: text.to_string(),
            }],
            None,
        ),
        Err(error) => (Vec::new(), Some(error.to_string())),
    };
    RuntimeResponse::Result {
        cell_id,
        content_items,
        error_text,
        result_buffered_for: Duration::ZERO,
        code_mode_host_duration: None,
    }
}

#[tokio::test]
async fn preloads_follow_the_session_and_the_current_cells_tool_binding() {
    let plain = InProcessCodeModeSession::new();
    let lookup = InProcessCodeModeSession::with_options_and_tool_preloads(
        Default::default(),
        /*task_failure_handler*/ None,
        &[ToolPreload {
            tool: "lookup",
            tool_name: "lookup",
            tool_namespace: None,
            source: "globalThis.findItem = tools.lookup;",
        }],
    );
    let alternate = InProcessCodeModeSession::with_options_and_tool_preloads(
        Default::default(),
        /*task_failure_handler*/ None,
        &[ToolPreload {
            tool: "lookup",
            tool_name: "lookup",
            tool_namespace: None,
            source: "globalThis.findItem = () => 'alternate';",
        }],
    );
    for (session, name, expected) in [
        (&lookup, "lookup", "true"),
        (&plain, "lookup", "undefined"),
        (&alternate, "lookup", "false"),
        (&lookup, "lookup_other", "undefined"),
        (&lookup, "lookup", "true"),
    ] {
        let started = session
            .execute(
                ExecuteRequest {
                    tool_call_id: name.to_string(),
                    enabled_tools: vec![ToolDefinition {
                        name: name.to_string(),
                        tool_name: ToolName::plain(name),
                        description: String::new(),
                        kind: CodeModeToolKind::Function,
                        input_schema: None,
                        input_schema_max_bytes: None,
                        output_schema: None,
                    }],
                    source: "text(typeof findItem === 'function' ? findItem === tools.lookup : typeof findItem);".to_string(),
                    yield_time_ms: None,
                    max_output_tokens: None,
                },
                Arc::new(NoopCodeModeSessionDelegate),
                /*preempt*/ None,
            )
            .await
            .unwrap();
        let cell_id = started.cell_id.clone();
        assert_eq!(
            started.initial_response().await.unwrap(),
            RuntimeResponse::Result {
                cell_id,
                content_items: vec![FunctionCallOutputContentItem::InputText {
                    text: expected.to_string(),
                }],
                error_text: None,
                result_buffered_for: std::time::Duration::ZERO,
                code_mode_host_duration: None,
            }
        );
    }
}

#[tokio::test]
async fn preload_requires_one_binding_with_the_exact_dispatch_identity() {
    let session = InProcessCodeModeSession::with_options_and_tool_preloads(
        Default::default(),
        /*task_failure_handler*/ None,
        &[ToolPreload {
            tool: SEND_BINDING,
            tool_name: SEND_BINDING,
            tool_namespace: None,
            source: "globalThis.sendMsgToUser = message => tools.mcp__codex_apps__user_message_send_message({text: message});",
        }],
    );
    let expected_name = ToolName::plain(SEND_BINDING);
    let canonical = definition(SEND_BINDING, expected_name.clone());
    let alias = definition(
        "mcp__codex_apps__user_message_send-message",
        ToolName::plain("mcp__codex_apps__user_message_send-message"),
    );
    let reassigned = definition(SEND_BINDING, ToolName::plain("unrelated"));
    let wrong_namespace = definition(
        SEND_BINDING,
        ToolName::namespaced("unrelated", SEND_BINDING),
    );
    let split_identity = definition(
        SEND_BINDING,
        ToolName::namespaced(SEND_NAMESPACE, "user_message_send_message"),
    );
    let delegate = Arc::new(RecordingDelegate::default());
    let source = "if (typeof sendMsgToUser === 'function') await sendMsgToUser('private'); text(typeof sendMsgToUser);";
    let ambiguous =
        "ambiguous binding: tool preload #1 for `mcp__codex_apps__user_message_send_message`";
    for (tools, expected) in [
        (vec![alias.clone()], Ok("undefined")),
        (vec![canonical.clone(), alias.clone()], Err(ambiguous)),
        (vec![alias, canonical.clone()], Err(ambiguous)),
        (vec![reassigned], Ok("undefined")),
        (vec![wrong_namespace], Ok("undefined")),
        (vec![split_identity.clone()], Ok("undefined")),
    ] {
        let (cell_id, response) = execute(&session, tools, source, delegate.clone())
            .await
            .unwrap();
        assert_eq!(response, result(cell_id, expected));
        assert_eq!(*delegate.0.lock().unwrap(), Vec::<ToolName>::new());
    }

    let (cell_id, response) = execute(&session, vec![canonical], source, delegate.clone())
        .await
        .unwrap();
    assert_eq!(response, result(cell_id, Ok("function")));
    assert_eq!(*delegate.0.lock().unwrap(), vec![expected_name]);

    let namespaced = InProcessCodeModeSession::with_options_and_tool_preloads(
        Default::default(),
        /*task_failure_handler*/ None,
        &[ToolPreload {
            tool: SEND_BINDING,
            tool_name: "user_message_send_message",
            tool_namespace: Some(SEND_NAMESPACE),
            source: "globalThis.sendMsgToUser = message => tools.mcp__codex_apps__user_message_send_message({text: message});",
        }],
    );
    let (cell_id, response) = execute(&namespaced, vec![split_identity], source, delegate.clone())
        .await
        .unwrap();
    assert_eq!(response, result(cell_id, Ok("function")));
    assert_eq!(
        *delegate.0.lock().unwrap(),
        vec![
            ToolName::plain(SEND_BINDING),
            ToolName::namespaced(SEND_NAMESPACE, "user_message_send_message")
        ]
    );
}

#[tokio::test]
async fn preload_compile_and_execution_failures_are_bounded_cell_errors() {
    for (preloads, expected) in [
        (
            &[ToolPreload {
                tool: "lookup",
                tool_name: "lookup",
                tool_namespace: None,
                source: "const =",
            }][..],
            "failed to compile tool preload #1 for `lookup`",
        ),
        (
            &[ToolPreload {
                tool: "lookup",
                tool_name: "lookup",
                tool_namespace: None,
                source: "throw 'x'.repeat(1_000_000)",
            }][..],
            "failed to execute tool preload #1 for `lookup`",
        ),
    ] {
        let session = InProcessCodeModeSession::with_options_and_tool_preloads(
            Default::default(),
            /*task_failure_handler*/ None,
            preloads,
        );
        let (cell_id, response) = execute(
            &session,
            vec![definition("lookup", ToolName::plain("lookup"))],
            "text('never runs')",
            Arc::new(NoopCodeModeSessionDelegate),
        )
        .await
        .unwrap();
        assert_eq!(response, result(cell_id, Err(expected)));
    }
}
