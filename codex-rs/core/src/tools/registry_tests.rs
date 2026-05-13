use super::*;
use crate::tools::handlers::GetGoalHandler;
use crate::tools::handlers::goal_spec::GET_GOAL_TOOL_NAME;
use crate::tools::handlers::goal_spec::create_get_goal_tool;
use pretty_assertions::assert_eq;

struct TestHandler {
    tool_name: codex_tools::ToolName,
}

impl ToolExecutor<ToolInvocation> for TestHandler {
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

impl ToolHandler for TestHandler {}

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
fn register_handler_adds_handler_and_spec() {
    let mut builder = ToolRegistryBuilder::new();
    builder.register_handler(Arc::new(GetGoalHandler));

    let (specs, registry) = builder.build();

    assert_eq!(specs.len(), 1);
    assert_eq!(specs[0], create_get_goal_tool());
    assert!(registry.has_handler(&codex_tools::ToolName::plain(GET_GOAL_TOOL_NAME)));
}
