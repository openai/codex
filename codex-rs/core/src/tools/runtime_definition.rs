use std::sync::Arc;

use codex_tool_api::ToolDefinition;
use codex_tools::ToolSpec;

use crate::tools::registry::AnyToolHandler;
use crate::tools::registry::ToolHandler;

pub(crate) type RuntimeToolDefinition = ToolDefinition<Arc<dyn AnyToolHandler>, ToolSpec>;

pub(crate) fn runtime_tool_definition<H>(
    handler: H,
    spec: ToolSpec,
) -> RuntimeToolDefinition
where
    H: ToolHandler + 'static,
{
    let handler = Arc::new(handler);
    let tool_name = handler.tool_name();
    let runtime: Arc<dyn AnyToolHandler> = handler;
    ToolDefinition::new(tool_name, spec, runtime)
}
