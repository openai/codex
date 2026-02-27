use async_trait::async_trait;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::request_permissions::RequestPermissionsArgs;

use crate::function_tool::FunctionCallError;
use crate::sandboxing::normalize_additional_permissions;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub(crate) fn request_permissions_tool_description() -> String {
    "Request additional permissions from the user and wait for the client to grant a subset of the requested permission profile."
        .to_string()
}

pub struct RequestPermissionsHandler;

#[async_trait]
impl ToolHandler for RequestPermissionsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
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

        let mut args: RequestPermissionsArgs =
            parse_arguments_with_base_path(&arguments, turn.cwd.as_path())?;
        args.permissions = normalize_additional_permissions(args.permissions)
            .map_err(FunctionCallError::RespondToModel)?;
        if args.permissions.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "request_permissions requires at least one permission".to_string(),
            ));
        }

        let response = session
            .request_permissions(turn.as_ref(), call_id, args)
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

        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}
