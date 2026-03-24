use super::*;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use async_trait::async_trait;
use codex_protocol::models::ShellToolCallParams;
use pretty_assertions::assert_eq;

struct TestHandler;

#[async_trait]
impl ToolHandler for TestHandler {
    type Output = crate::tools::context::FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, _invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        unreachable!("test handler should not be invoked")
    }
}

#[test]
fn handler_looks_up_namespaced_aliases_explicitly() {
    let plain_handler = Arc::new(TestHandler) as Arc<dyn AnyToolHandler>;
    let namespaced_handler = Arc::new(TestHandler) as Arc<dyn AnyToolHandler>;
    let namespace = "mcp__codex_apps__gmail";
    let tool_name = "gmail_get_recent_emails";
    let namespaced_name = tool_handler_key(tool_name, Some(namespace));
    let registry = ToolRegistry::new(HashMap::from([
        (tool_name.to_string(), Arc::clone(&plain_handler)),
        (namespaced_name, Arc::clone(&namespaced_handler)),
    ]));

    let plain = registry.handler(tool_name, None);
    let namespaced = registry.handler(tool_name, Some(namespace));
    let missing_namespaced = registry.handler(tool_name, Some("mcp__codex_apps__calendar"));

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
fn pre_tool_use_command_uses_raw_shell_command_input() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "command": "printf shell command" }).to_string(),
    };

    assert_eq!(
        pre_tool_use_command("shell_command", &payload),
        Some("printf shell command".to_string())
    );
}

#[test]
fn pre_tool_use_command_shell_joins_vector_input() {
    let payload = ToolPayload::LocalShell {
        params: ShellToolCallParams {
            command: vec![
                "bash".to_string(),
                "-lc".to_string(),
                "printf hi".to_string(),
            ],
            workdir: None,
            timeout_ms: None,
            sandbox_permissions: None,
            prefix_rule: None,
            additional_permissions: None,
            justification: None,
        },
    };

    assert_eq!(
        pre_tool_use_command("local_shell", &payload),
        Some("bash -lc 'printf hi'".to_string())
    );
}

#[test]
fn pre_tool_use_command_uses_raw_exec_command_input() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({ "cmd": "printf exec command" }).to_string(),
    };

    assert_eq!(
        pre_tool_use_command("exec_command", &payload),
        Some("printf exec command".to_string())
    );
}

#[test]
fn pre_tool_use_command_skips_non_shell_tools() {
    let payload = ToolPayload::Function {
        arguments: serde_json::json!({
            "plan": [{ "step": "watch the tide", "status": "pending" }]
        })
        .to_string(),
    };

    assert_eq!(pre_tool_use_command("update_plan", &payload), None);
}
