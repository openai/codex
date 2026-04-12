use codex_protocol::request_permissions::PermissionGrantScope;
use codex_protocol::request_permissions::RequestPermissionProfile;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_protocol::request_permissions::RequestPermissionsResponse;
use codex_sandboxing::policy_transforms::normalize_additional_permissions;
use serde::Serialize;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments_with_base_path;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub struct RequestPermissionsHandler;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum RequestPermissionsToolStatus {
    Granted,
    Denied,
}

#[derive(Debug, Serialize, PartialEq, Eq)]
struct RequestPermissionsToolOutput {
    status: RequestPermissionsToolStatus,
    scope: PermissionGrantScope,
    permissions: RequestPermissionProfile,
    message: &'static str,
}

fn output_for_response(response: RequestPermissionsResponse) -> RequestPermissionsToolOutput {
    if response.permissions.is_empty() {
        RequestPermissionsToolOutput {
            status: RequestPermissionsToolStatus::Denied,
            scope: response.scope,
            permissions: response.permissions,
            message: "The user has already denied or declined this permission request. Do not say that approval is still pending.",
        }
    } else {
        RequestPermissionsToolOutput {
            status: RequestPermissionsToolStatus::Granted,
            scope: response.scope,
            permissions: response.permissions,
            message: "The user has already approved this permission request. These permissions are active now; do not ask the user to approve them again.",
        }
    }
}

impl ToolHandler for RequestPermissionsHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
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
            parse_arguments_with_base_path(&arguments, &turn.cwd)?;
        args.permissions = normalize_additional_permissions(args.permissions.into())
            .map(codex_protocol::request_permissions::RequestPermissionProfile::from)
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

        let tool_output = output_for_response(response);
        let content = serde_json::to_string(&tool_output).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize request_permissions response: {err}"
            ))
        })?;

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

#[cfg(test)]
mod tests {
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::request_permissions::PermissionGrantScope;
    use codex_protocol::request_permissions::RequestPermissionProfile;
    use codex_protocol::request_permissions::RequestPermissionsResponse;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::output_for_response;

    #[test]
    fn request_permissions_tool_output_marks_granted_permissions_as_active() {
        let permissions = RequestPermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: None,
        };

        let output = output_for_response(RequestPermissionsResponse {
            permissions,
            scope: PermissionGrantScope::Session,
        });

        assert_eq!(
            serde_json::to_value(output).expect("serialize tool output"),
            json!({
                "status": "granted",
                "scope": "session",
                "permissions": {
                    "network": {
                        "enabled": true,
                    },
                    "file_system": null,
                },
                "message": "The user has already approved this permission request. These permissions are active now; do not ask the user to approve them again.",
            })
        );
    }

    #[test]
    fn request_permissions_tool_output_marks_empty_permissions_as_denied() {
        let output = output_for_response(RequestPermissionsResponse {
            permissions: RequestPermissionProfile::default(),
            scope: PermissionGrantScope::Turn,
        });

        assert_eq!(
            serde_json::to_value(output).expect("serialize tool output"),
            json!({
                "status": "denied",
                "scope": "turn",
                "permissions": {
                    "network": null,
                    "file_system": null,
                },
                "message": "The user has already denied or declined this permission request. Do not say that approval is still pending.",
            })
        );
    }
}
