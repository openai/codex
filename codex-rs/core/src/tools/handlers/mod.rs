pub(crate) mod agent_jobs;
pub mod apply_patch;
mod dynamic;
mod js_repl;
mod list_dir;
mod mcp;
mod mcp_resource;
pub(crate) mod multi_agents;
pub(crate) mod multi_agents_common;
pub(crate) mod multi_agents_v2;
mod plan;
mod request_permissions;
mod request_user_input;
mod shell;
mod test_sync;
mod timers;
mod tool_search;
mod tool_suggest;
pub(crate) mod unified_exec;
mod view_image;

use codex_sandboxing::policy_transforms::intersect_permission_profiles;
use codex_sandboxing::policy_transforms::merge_permission_profiles;
use codex_sandboxing::policy_transforms::normalize_additional_permissions;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_absolute_path::AbsolutePathBufGuard;
use serde::Deserialize;
use serde_json::Value;
use std::path::Path;

use crate::codex::Session;
use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
pub(crate) use crate::tools::code_mode::CodeModeExecuteHandler;
pub(crate) use crate::tools::code_mode::CodeModeWaitHandler;
pub use apply_patch::ApplyPatchHandler;
use codex_protocol::models::PermissionProfile;
use codex_protocol::protocol::AskForApproval;
pub use dynamic::DynamicToolHandler;
pub use js_repl::JsReplHandler;
pub use js_repl::JsReplResetHandler;
pub use list_dir::ListDirHandler;
pub use mcp::McpHandler;
pub use mcp_resource::McpResourceHandler;
pub use plan::PlanHandler;
pub use request_permissions::RequestPermissionsHandler;
pub use request_user_input::RequestUserInputHandler;
pub use shell::ShellCommandHandler;
pub use shell::ShellHandler;
pub use test_sync::TestSyncHandler;
pub use timers::CreateTimerHandler;
pub use timers::DeleteTimerHandler;
pub use timers::ListTimersHandler;
pub use tool_search::ToolSearchHandler;
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
    base_path: &AbsolutePathBuf,
) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    let _guard = AbsolutePathBufGuard::new(base_path);
    parse_arguments(arguments)
}

fn resolve_workdir_base_path(
    arguments: &str,
    default_cwd: &AbsolutePathBuf,
) -> Result<AbsolutePathBuf, FunctionCallError> {
    let arguments: Value = parse_arguments(arguments)?;
    Ok(arguments
        .get("workdir")
        .and_then(Value::as_str)
        .filter(|workdir| !workdir.is_empty())
        .map_or_else(|| default_cwd.clone(), |workdir| default_cwd.join(workdir)))
}

/// Validates feature/policy constraints for `with_additional_permissions` and
/// normalizes any path-based permissions. Errors if the request is invalid.
pub(crate) fn normalize_and_validate_additional_permissions(
    additional_permissions_allowed: bool,
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

    if !permissions_preapproved
        && !additional_permissions_allowed
        && (uses_additional_permissions || additional_permissions.is_some())
    {
        return Err(
            "additional permissions are disabled; enable `features.exec_permission_approvals` before using `with_additional_permissions`"
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
                "missing `additional_permissions`; provide at least one of `network` or `file_system` when using `with_additional_permissions`"
                    .to_string(),
            );
        };
        let normalized = normalize_additional_permissions(additional_permissions)?;
        if normalized.is_empty() {
            return Err(
                "`additional_permissions` must include at least one requested permission in `network` or `file_system`"
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

pub(super) fn implicit_granted_permissions(
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<&PermissionProfile>,
    effective_additional_permissions: &EffectiveAdditionalPermissions,
) -> Option<PermissionProfile> {
    if !sandbox_permissions.uses_additional_permissions()
        && !matches!(sandbox_permissions, SandboxPermissions::RequireEscalated)
        && additional_permissions.is_none()
    {
        effective_additional_permissions
            .additional_permissions
            .clone()
    } else {
        None
    }
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
            intersect_permission_profiles(effective_permissions.clone(), granted_permissions)
                == *effective_permissions
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

#[cfg(test)]
mod tests {
    use super::EffectiveAdditionalPermissions;
    use super::implicit_granted_permissions;
    use super::normalize_and_validate_additional_permissions;
    use crate::sandboxing::SandboxPermissions;
    use codex_protocol::models::FileSystemPermissions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::protocol::AskForApproval;
    use codex_protocol::protocol::GranularApprovalConfig;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

    fn network_permissions() -> PermissionProfile {
        PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            ..Default::default()
        }
    }

    fn file_system_permissions(path: &std::path::Path) -> PermissionProfile {
        PermissionProfile {
            file_system: Some(FileSystemPermissions {
                read: None,
                write: Some(vec![
                    AbsolutePathBuf::from_absolute_path(path).expect("absolute path"),
                ]),
            }),
            ..Default::default()
        }
    }

    #[test]
    fn preapproved_permissions_work_when_request_permissions_tool_is_enabled_without_exec_permission_approvals_feature()
     {
        let cwd = tempdir().expect("tempdir");

        let normalized = normalize_and_validate_additional_permissions(
            /*additional_permissions_allowed*/ false,
            AskForApproval::Granular(GranularApprovalConfig {
                sandbox_approval: true,
                rules: true,
                skill_approval: true,
                request_permissions: false,
                mcp_elicitations: true,
            }),
            SandboxPermissions::WithAdditionalPermissions,
            Some(network_permissions()),
            /*permissions_preapproved*/ true,
            cwd.path(),
        )
        .expect("preapproved permissions should be allowed");

        assert_eq!(normalized, Some(network_permissions()));
    }

    #[test]
    fn fresh_additional_permissions_still_require_exec_permission_approvals_feature() {
        let cwd = tempdir().expect("tempdir");

        let err = normalize_and_validate_additional_permissions(
            /*additional_permissions_allowed*/ false,
            AskForApproval::OnRequest,
            SandboxPermissions::WithAdditionalPermissions,
            Some(network_permissions()),
            /*permissions_preapproved*/ false,
            cwd.path(),
        )
        .expect_err("fresh inline permission requests should remain disabled");

        assert_eq!(
            err,
            "additional permissions are disabled; enable `features.exec_permission_approvals` before using `with_additional_permissions`"
        );
    }

    #[test]
    fn implicit_sticky_grants_bypass_inline_permission_validation() {
        let cwd = tempdir().expect("tempdir");
        let granted_permissions = file_system_permissions(cwd.path());
        let implicit_permissions = implicit_granted_permissions(
            SandboxPermissions::UseDefault,
            /*additional_permissions*/ None,
            &EffectiveAdditionalPermissions {
                sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
                additional_permissions: Some(granted_permissions.clone()),
                permissions_preapproved: false,
            },
        );

        assert_eq!(implicit_permissions, Some(granted_permissions));
    }

    #[test]
    fn explicit_inline_permissions_do_not_use_implicit_sticky_grant_path() {
        let cwd = tempdir().expect("tempdir");
        let requested_permissions = file_system_permissions(cwd.path());
        let implicit_permissions = implicit_granted_permissions(
            SandboxPermissions::WithAdditionalPermissions,
            Some(&requested_permissions),
            &EffectiveAdditionalPermissions {
                sandbox_permissions: SandboxPermissions::WithAdditionalPermissions,
                additional_permissions: Some(requested_permissions.clone()),
                permissions_preapproved: false,
            },
        );

        assert_eq!(implicit_permissions, None);
    }
}
