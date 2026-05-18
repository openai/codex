use codex_tools::LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME;
use codex_tools::ListAvailablePluginsToInstallResult;
use codex_tools::RequestPluginInstallEntry;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::list_available_plugins_to_install_spec::create_list_available_plugins_to_install_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

pub struct ListAvailablePluginsToInstallHandler {
    tools: Vec<RequestPluginInstallEntry>,
}

impl ListAvailablePluginsToInstallHandler {
    pub(crate) fn new(mut tools: Vec<RequestPluginInstallEntry>) -> Self {
        tools.sort_by(|left, right| {
            left.name
                .cmp(&right.name)
                .then_with(|| left.id.cmp(&right.id))
        });
        Self { tools }
    }
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ListAvailablePluginsToInstallHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_list_available_plugins_to_install_tool())
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation { payload, .. } = invocation;
        match payload {
            ToolPayload::Function { .. } => {}
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME} handler received unsupported payload"
                )));
            }
        }

        let content = serde_json::to_string(&ListAvailablePluginsToInstallResult {
            tools: self.tools.clone(),
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {LIST_AVAILABLE_PLUGINS_TO_INSTALL_TOOL_NAME} response: {err}"
            ))
        })?;

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for ListAvailablePluginsToInstallHandler {}
