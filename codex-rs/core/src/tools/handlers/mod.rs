pub(crate) mod agent_jobs;
pub mod apply_patch;
mod dynamic;
mod grep_files;
mod js_repl;
mod list_dir;
mod mcp;
mod mcp_resource;
pub(crate) mod multi_agents;
mod plan;
mod read_file;
mod request_user_input;
mod search_tool_bm25;
mod shell;
mod test_sync;
pub(crate) mod unified_exec;
mod view_image;

pub use plan::PLAN_TOOL;
use serde::Deserialize;
use std::path::Path;

use crate::function_tool::FunctionCallError;
use crate::sandboxing::SandboxPermissions;
use crate::sandboxing::normalize_additional_permissions;
pub use apply_patch::ApplyPatchHandler;
use codex_protocol::models::AdditionalPermissions;
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
pub use request_user_input::RequestUserInputHandler;
pub(crate) use request_user_input::request_user_input_tool_description;
pub(crate) use search_tool_bm25::DEFAULT_LIMIT as SEARCH_TOOL_BM25_DEFAULT_LIMIT;
pub(crate) use search_tool_bm25::SEARCH_TOOL_BM25_TOOL_NAME;
pub use search_tool_bm25::SearchToolBm25Handler;
pub use shell::ShellCommandHandler;
pub use shell::ShellHandler;
pub use test_sync::TestSyncHandler;
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

/// Validates feature/policy constraints for `with_additional_permissions` and
/// returns normalized absolute paths. Errors if paths are invalid.
pub(super) fn normalize_and_validate_additional_permissions(
    request_permission_enabled: bool,
    approval_policy: AskForApproval,
    sandbox_permissions: SandboxPermissions,
    additional_permissions: Option<AdditionalPermissions>,
    cwd: &Path,
) -> Result<Option<AdditionalPermissions>, String> {
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
        if !matches!(approval_policy, AskForApproval::OnRequest) {
            return Err(format!(
                "approval policy is {approval_policy:?}; reject command — you cannot request additional permissions unless the approval policy is OnRequest"
            ));
        }
        let Some(additional_permissions) = additional_permissions else {
            return Err(
                "missing `additional_permissions`; provide `fs_read` and/or `fs_write` when using `with_additional_permissions`"
                    .to_string(),
            );
        };
        let normalized = normalize_additional_permissions(additional_permissions, cwd)?;
        if normalized.is_empty() {
            return Err(
                "`additional_permissions` must include at least one path in `fs_read` or `fs_write`"
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
