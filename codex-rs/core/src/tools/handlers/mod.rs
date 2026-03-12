pub(crate) mod agent_jobs;
pub mod apply_patch;
mod artifacts;
mod dynamic;
mod grep_files;
mod js_repl;
mod list_dir;
mod mcp;
mod mcp_resource;
pub(crate) mod multi_agents;
mod plan;
mod read_file;
mod request_permissions;
mod request_user_input;
mod shell;
mod test_sync;
mod tool_search;
mod tool_suggest;
pub(crate) mod unified_exec;
mod view_image;

use codex_utils_absolute_path::AbsolutePathBufGuard;
pub use plan::PLAN_TOOL;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;
use std::path::PathBuf;

use crate::codex::Session;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::merge_permission_profiles;
use crate::sandboxing::normalize_additional_permissions;
pub(crate) use crate::tools::code_mode::CodeModeExecuteHandler;
pub(crate) use crate::tools::code_mode::CodeModeWaitHandler;
pub use apply_patch::ApplyPatchHandler;
pub use artifacts::ArtifactsHandler;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
pub use dynamic::DynamicToolHandler;
pub use grep_files::GrepFilesHandler;
pub use js_repl::JsReplHandler;
pub use js_repl::JsReplResetHandler;
pub use list_dir::ListDirHandler;
pub use mcp::McpHandler;
pub use mcp_resource::McpResourceHandler;
pub use multi_agents::MultiAgentHandler;
pub use plan::PlanHandler;
pub use read_file::ReadFileHandler;
pub use request_permissions::RequestPermissionsHandler;
pub(crate) use request_permissions::request_permissions_tool_description;
pub use request_user_input::RequestUserInputHandler;
pub(crate) use request_user_input::request_user_input_tool_description;
pub use shell::ShellCommandHandler;
pub use shell::ShellHandler;
pub use test_sync::TestSyncHandler;
pub(crate) use tool_search::DEFAULT_LIMIT as TOOL_SEARCH_DEFAULT_LIMIT;
pub(crate) use tool_search::TOOL_SEARCH_TOOL_NAME;
pub use tool_search::ToolSearchHandler;
pub(crate) use tool_suggest::TOOL_SUGGEST_TOOL_NAME;
pub use tool_suggest::ToolSuggestHandler;
pub use unified_exec::UnifiedExecHandler;
pub use view_image::ViewImageHandler;

fn parse_arguments<T>(arguments: &str) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    serde_json::from_str(arguments).map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to parse function arguments: {err}"))
    })
}

fn parse_arguments_with_base_path<T>(
    arguments: &str,
    base_path: &Path,
) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    let _guard = AbsolutePathBufGuard::new(base_path);
    parse_arguments(arguments)
}

fn resolve_workdir_base_path(
    arguments: &str,
    default_cwd: &Path,
) -> Result<PathBuf, FunctionCallError> {
    let arguments: Value = parse_arguments(arguments)?;
    Ok(arguments
        .get("workdir")
        .and_then(Value::as_str)
        .filter(|workdir| !workdir.is_empty())
        .map(PathBuf::from)
        .map_or_else(
            || default_cwd.to_path_buf(),
            |workdir| crate::util::resolve_path(default_cwd, &workdir),
        ))
}

/// Validates feature/policy constraints for `with_additional_permissions` and
/// normalizes any path-based permissions. Errors if the request is invalid.
pub(crate) fn normalize_and_validate_additional_permissions(
    request_permission_enabled: bool,
    approval_policy: AskForApproval,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<PermissionProfile>,
    permissions_preapproved: bool,
    _cwd: &Path,
) -> Result<Option<PermissionProfile>, String> {
    let uses_additional_permissions = matches!(
        sandbox_permissions,
        SandboxPermissions::WithAdditionalPermissions
    );

    if !request_permission_enabled
        && (uses_additional_permissions || additional_permissions.is_some())
    {
        return Err(
            "additional permissions are disabled; enable `features.request_permission` before using `with_additional_permissions`"
                .to_string(),
        );
    }

    if uses_additional_permissions {
        if !permissions_preapproved && !matches!(approval_policy, AskForApproval::OnRequest) {
            return Err(format!(
                "approval policy is {approval_policy:?}; reject command — you cannot request additional permissions unless the approval policy is OnRequest"
            ));
        }
        let Some(additional_permissions) = additional_permissions else {
            return Err(
                "missing `additional_permissions`; provide at least one of `network`, `file_system`, or `macos` when using `with_additional_permissions`"
                    .to_string(),
            );
        };
        #[cfg(not(target_os = "macos"))]
        if additional_permissions.macos.is_some() {
            return Err("`additional_permissions.macos` is only supported on macOS".to_string());
        }
        let normalized = normalize_additional_permissions(additional_permissions)?;
        if normalized.is_empty() {
            return Err(
                "`additional_permissions` must include at least one requested permission in `network`, `file_system`, or `macos`"
                    .to_string(),
            );
        }
        return Ok(Some(normalized));
    }

    if additional_permissions.is_some() {
        Err(
            "`additional_permissions` requires `sandbox_permissions` set to `with_additional_permissions`"
                .to_string(),
        )
    } else {
        Ok(None)
    }
}

pub(super) struct EffectiveAdditionalPermissions {
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub permissions_preapproved: bool,
}

pub(super) async fn apply_granted_turn_permissions(
    session: &Session,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<PermissionProfile>,
) -> EffectiveAdditionalPermissions {
    if matches!(sandbox_permissions, SandboxPermissions::RequireEscalated) {
        return EffectiveAdditionalPermissions {
            sandbox_permissions,
            additional_permissions,
            permissions_preapproved: false,
        };
    }

    let granted_session_permissions = session.granted_session_permissions().await;
    let granted_turn_permissions = session.granted_turn_permissions().await;
    let granted_permissions = merge_permission_profiles(
        granted_session_permissions.as_ref(),
        granted_turn_permissions.as_ref(),
    );
    let effective_permissions = merge_permission_profiles(
        additional_permissions.as_ref(),
        granted_permissions.as_ref(),
    );
    let permissions_preapproved = match (effective_permissions.as_ref(), granted_permissions) {
        (Some(effective_permissions), Some(granted_permissions)) => {
            crate::sandboxing::intersect_permission_profiles(
                effective_permissions.clone(),
                granted_permissions,
            ) == *effective_permissions
        }
        _ => false,
    };

    let sandbox_permissions =
        if effective_permissions.is_some() && !sandbox_permissions.uses_additional_permissions() {
            SandboxPermissions::WithAdditionalPermissions
        } else {
            sandbox_permissions
        };

    EffectiveAdditionalPermissions {
        sandbox_permissions,
        additional_permissions: effective_permissions,
        permissions_preapproved,
    }
}
