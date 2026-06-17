use std::sync::Arc;

use codex_extension_api::FunctionCallError;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_tools::DiscoverableTool;
use codex_tools::DiscoverableToolAction;

use crate::RequestPluginInstallsArgs;
use crate::RequestPluginInstallsBackend;
use crate::RequestPluginInstallsRequest;
use crate::ToolSuggestPresentation;
use crate::spec::create_request_plugin_installs_tool;
use crate::spec::create_request_plugin_installs_tool_for_tui;
use crate::validation::validate_request_plugin_install_picker_args;

pub const REQUEST_PLUGIN_INSTALLS_TOOL_NAME: &str = "request_plugin_installs";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequestPluginInstallsMode {
    SingleEntry,
    MultipleEntries,
}

struct RequestPluginInstallsTool {
    backend: Arc<dyn RequestPluginInstallsBackend>,
    discoverable_tools: Vec<DiscoverableTool>,
    presentation: ToolSuggestPresentation,
    mode: RequestPluginInstallsMode,
}

pub fn request_plugin_installs_tool(
    backend: Arc<dyn RequestPluginInstallsBackend>,
    discoverable_tools: Vec<DiscoverableTool>,
    presentation: ToolSuggestPresentation,
    mode: RequestPluginInstallsMode,
) -> Arc<dyn ToolExecutor<ToolCall>> {
    Arc::new(RequestPluginInstallsTool {
        backend,
        discoverable_tools,
        presentation,
        mode,
    })
}

impl ToolExecutor<ToolCall> for RequestPluginInstallsTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(REQUEST_PLUGIN_INSTALLS_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        match self.mode {
            RequestPluginInstallsMode::SingleEntry => {
                create_request_plugin_installs_tool_for_tui(self.presentation)
            }
            RequestPluginInstallsMode::MultipleEntries => {
                create_request_plugin_installs_tool(self.presentation)
            }
        }
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

impl RequestPluginInstallsTool {
    async fn handle_call(&self, call: ToolCall) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let arguments = call.function_arguments()?;
        let args: RequestPluginInstallsArgs = serde_json::from_str(arguments).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
        })?;
        if args.action_type != DiscoverableToolAction::Install {
            return Err(FunctionCallError::RespondToModel(
                "plugin install requests currently support only action_type=\"install\""
                    .to_string(),
            ));
        }
        let client_name = match self.mode {
            RequestPluginInstallsMode::SingleEntry => Some("codex-tui"),
            RequestPluginInstallsMode::MultipleEntries => None,
        };
        let resolved_entries = validate_request_plugin_install_picker_args(
            &args,
            &self.discoverable_tools,
            client_name,
            self.presentation,
        )?;
        let result = self
            .backend
            .execute(RequestPluginInstallsRequest {
                call_id: call.call_id,
                turn_id: call.turn_id,
                args,
                resolved_entries,
            })
            .await?;
        let value = serde_json::to_value(result).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {REQUEST_PLUGIN_INSTALLS_TOOL_NAME} response: {err}"
            ))
        })?;
        Ok(Box::new(JsonToolOutput::with_success(value, Some(true))))
    }
}
