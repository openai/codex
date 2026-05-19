use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::reload_plugins_spec::RELOAD_PLUGINS_TOOL_NAME;
use crate::tools::handlers::reload_plugins_spec::create_reload_plugins_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

pub struct ReloadPluginsHandler;

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for ReloadPluginsHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(RELOAD_PLUGINS_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_reload_plugins_tool())
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            payload,
            session,
            turn,
            ..
        } = invocation;

        match payload {
            ToolPayload::Function { arguments } if arguments.trim() == "{}" => {}
            ToolPayload::Function { arguments } if arguments.trim().is_empty() => {}
            ToolPayload::Function { .. } => {
                return Err(FunctionCallError::RespondToModel(
                    "reload_plugins does not accept arguments".to_string(),
                ));
            }
            _ => {
                return Err(FunctionCallError::Fatal(format!(
                    "{RELOAD_PLUGINS_TOOL_NAME} handler received unsupported payload"
                )));
            }
        }

        session.reload_user_config_layer().await;
        let config = session.get_config().await;
        let mcp_config = config
            .to_mcp_config(session.plugins_manager().as_ref())
            .await;
        session
            .refresh_mcp_servers_now(
                turn.as_ref(),
                mcp_config.configured_mcp_servers.clone(),
                mcp_config.mcp_oauth_credentials_store_mode,
                Some(session.mcp_elicitation_reviewer()),
            )
            .await;

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            "{\"reloaded\":true}".to_string(),
            Some(true),
        )))
    }
}

impl CoreToolRuntime for ReloadPluginsHandler {}
