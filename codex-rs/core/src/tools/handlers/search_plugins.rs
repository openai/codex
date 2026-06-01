use codex_core_plugins::remote::RemotePluginServiceConfig;
use codex_core_plugins::remote::search_global_remote_plugins;
use codex_tools::JsonToolOutput;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::search_plugins_spec::SEARCH_PLUGINS_TOOL_NAME;
use crate::tools::handlers::search_plugins_spec::create_search_plugins_tool;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;

pub struct SearchPluginsHandler;

#[derive(Debug, Deserialize)]
struct SearchPluginsArgs {
    q: String,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for SearchPluginsHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(SEARCH_PLUGINS_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        create_search_plugins_tool()
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        true
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            ..
        } = invocation;
        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(format!(
                    "{SEARCH_PLUGINS_TOOL_NAME} handler received unsupported payload"
                )));
            }
        };
        let args: SearchPluginsArgs = parse_arguments(&arguments)?;
        let auth = session.services.auth_manager.auth().await;
        let result = search_global_remote_plugins(
            &RemotePluginServiceConfig {
                chatgpt_base_url: turn.config.chatgpt_base_url.clone(),
            },
            auth.as_ref(),
            &args.q,
        )
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("{SEARCH_PLUGINS_TOOL_NAME} failed: {err}"))
        })?;
        let value = serde_json::to_value(result).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize {SEARCH_PLUGINS_TOOL_NAME} response: {err}"
            ))
        })?;
        Ok(Box::new(JsonToolOutput::new(value)))
    }
}

impl CoreToolRuntime for SearchPluginsHandler {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_plugins_supports_parallel_calls() {
        assert!(SearchPluginsHandler.supports_parallel_tool_calls());
    }
}
