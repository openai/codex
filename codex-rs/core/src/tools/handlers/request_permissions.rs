use codex_protocol::permissions::FileSystemSandboxPolicyContext;
use codex_protocol::request_permissions::RequestPermissionsArgs;
use codex_sandboxing::policy_transforms::normalize_additional_permissions_with_context;
use codex_utils_path_uri::LegacyAppPathString;
use serde_json::Value;

use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::context::boxed_tool_output;
use crate::tools::handlers::parse_arguments;
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

impl ToolExecutor<ToolInvocation> for RequestPermissionsHandler {
    fn tool_name(&self) -> ToolName {
        ToolName::plain("request_permissions")
    }

    fn spec(&self) -> ToolSpec {
        create_request_permissions_tool(request_permissions_tool_description())
    }

    fn handle<'a>(&'a self, invocation: ToolInvocation) -> codex_tools::ToolExecutorFuture<'a>
    where
        ToolInvocation: 'a,
    {
        Box::pin(self.handle_call(invocation))
    }
}

impl RequestPermissionsHandler {
    async fn handle_call(
        &self,
        invocation: ToolInvocation,
    ) -> Result<Box<dyn crate::tools::context::ToolOutput>, FunctionCallError> {
        let ToolInvocation {
            session,
            step_context,
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
        let Some(turn_environment) = resolve_tool_environment(
            &step_context.environments,
            environment_args.environment_id.as_deref(),
        )?
        else {
            return Err(FunctionCallError::RespondToModel(
                "request_permissions requires a primary environment".to_string(),
            ));
        };
        let sandbox_context =
            turn_environment.sandbox_context(/*additional_permissions*/ None);
        let context = sandbox_context.policy_context();
        let mut arguments: Value = parse_arguments(&arguments)?;
        resolve_permission_path_strings(&mut arguments, &context)?;
        let mut args: RequestPermissionsArgs =
            serde_json::from_value(arguments).map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to parse function arguments: {err}"
                ))
            })?;
        args.permissions =
            normalize_additional_permissions_with_context(args.permissions.into(), &context)
                .map(codex_protocol::request_permissions::RequestPermissionProfile::from)
                .map_err(FunctionCallError::RespondToModel)?;
        if args.permissions.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "request_permissions requires at least one permission".to_string(),
            ));
        }

        let response = session
            .request_permissions_for_environment(
                &step_context,
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

fn resolve_permission_path_strings(
    arguments: &mut Value,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Result<(), FunctionCallError> {
    let Some(file_system) = arguments
        .pointer_mut("/permissions/file_system")
        .and_then(Value::as_object_mut)
    else {
        return Ok(());
    };
    let mut legacy_entries = Vec::new();
    for (field, access) in [("read", "read"), ("write", "write")] {
        if let Some(paths) = file_system.remove(field) {
            if paths.is_null() {
                continue;
            }
            let Value::Array(paths) = paths else {
                file_system.insert(field.to_string(), paths);
                continue;
            };
            for mut path in paths {
                resolve_permission_path_string(&mut path, context)?;
                legacy_entries.push(serde_json::json!({
                    "path": {"type": "path", "path": path},
                    "access": access,
                }));
            }
        }
    }
    if !legacy_entries.is_empty() {
        file_system
            .entry("entries".to_string())
            .or_insert_with(|| Value::Array(Vec::new()))
            .as_array_mut()
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(
                    "request_permissions file_system.entries must be an array".to_string(),
                )
            })?
            .extend(legacy_entries);
    }
    if let Some(entries) = file_system.get_mut("entries").and_then(Value::as_array_mut) {
        for entry in entries {
            if entry.pointer("/path/type").and_then(Value::as_str) == Some("path")
                && let Some(path) = entry.pointer_mut("/path/path")
            {
                resolve_permission_path_string(path, context)?;
            }
        }
    }
    Ok(())
}

fn resolve_permission_path_string(
    value: &mut Value,
    context: &FileSystemSandboxPolicyContext<'_>,
) -> Result<(), FunctionCallError> {
    let Some(path) = value.as_str() else {
        return Ok(());
    };
    let cwd = context.cwd;
    let convention = cwd.infer_path_convention().ok_or_else(|| {
        FunctionCallError::RespondToModel(format!(
            "request_permissions cwd `{cwd}` has no path convention"
        ))
    })?;
    let path = LegacyAppPathString::from_string(path)
        .resolve_against(cwd, context.user_home_dir)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
    if path.is_opaque() || std::str::from_utf8(&path.decoded_path_bytes()).is_err() {
        return Err(FunctionCallError::RespondToModel(
            "permission path cannot be represented losslessly".to_string(),
        ));
    }
    let path = LegacyAppPathString::from_path_uri(&path, convention)
        .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?;
    *value = Value::String(path.into_string());
    Ok(())
}
