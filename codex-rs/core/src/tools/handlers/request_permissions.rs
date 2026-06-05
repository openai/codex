use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_sandboxing::policy_transforms::normalize_additional_permissions;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::handlers::resolve_tool_environment;
use crate::tools::handlers::shell_spec::create_request_permissions_tool;
use crate::tools::handlers::shell_spec::request_permissions_tool_description;
use crate::tools::registry::CoreToolRuntime;
use crate::tools::registry::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;

pub struct RequestPermissionsHandler;

#[derive(Deserialize)]
struct RequestPermissionsEnvironmentArgs {
    #[serde(default, rename = "environment_id", alias = "environmentId")]
    environment_id: Option<String>,
}

#[async_trait::async_trait]
impl ToolExecutor<ToolInvocation> for RequestPermissionsHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("request_permissions")
    }

    fn spec(&self) -> ToolSpec {
        create_request_permissions_tool(request_permissions_tool_description())
    }

    async fn handle(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            cancellation_token,
            call_id,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "request_permissions handler received unsupported payload".to_string(),
                ));
            }
        };

        let environment_args: RequestPermissionsEnvironmentArgs = parse_arguments(&arguments)?;
        let Some(turn_environment) =
            resolve_tool_environment(turn.as_ref(), environment_args.environment_id.as_deref())?
        else {
            return Err(FunctionCallError::RespondToModel(
                "request_permissions requires a primary environment".to_string(),
            ));
        };
        let mut args: RequestPermissionsArgs =
            parse_arguments_with_base_path(&arguments, &turn_environment.cwd)?;
        args.permissions = normalize_additional_permissions(args.permissions.into())
            .map(codex_protocol::request_permissions::RequestPermissionProfile::from)
            .map_err(FunctionCallError::RespondToModel)?;
        if args.permissions.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "request_permissions requires at least one permission".to_string(),
            ));
        }

        let response = session
            .request_permissions_for_environment(
                &turn,
                call_id,
                args,
                turn_environment.selection(),
                cancellation_token,
            )
            .await
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "request_permissions was cancelled before receiving a response".to_string(),
                )
            })?;

        let content = serde_json::to_string(&response).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize request_permissions response: {err}"
            ))
        })?;

        Ok(boxed_tool_output(FunctionToolOutput::from_text(
            content,
            Some(true),
        )))
    }
}

impl CoreToolRuntime for RequestPermissionsHandler {}
