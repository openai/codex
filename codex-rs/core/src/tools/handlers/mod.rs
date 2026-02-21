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

use crate::function_tool::FunctionCallError;
use crate::protocol::SandboxPolicy;
pub use apply_patch::ApplyPatchHandler;
use codex_protocol::models::SandboxPermissions;
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

fn reject_explicit_escalation_if_deny_read_present(
    sandbox_permissions: SandboxPermissions,
    sandbox_policy: &SandboxPolicy,
) -> Result<(), FunctionCallError> {
    if sandbox_permissions.requires_escalated_permissions()
        && sandbox_policy.has_denied_read_paths()
    {
        return Err(FunctionCallError::RespondToModel(
            "filesystem deny_read policy is enforced; reject command — you cannot ask for escalated permissions because managed read restrictions must remain sandboxed".to_string(),
        ));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::reject_explicit_escalation_if_deny_read_present;
    use crate::function_tool::FunctionCallError;
    use crate::protocol::SandboxPolicy;
    use codex_protocol::models::SandboxPermissions;
    use codex_utils_absolute_path::AbsolutePathBuf;

    #[test]
    fn explicit_escalation_is_rejected_when_deny_read_paths_exist() {
        let mut policy = SandboxPolicy::new_workspace_write_policy();
        let path = AbsolutePathBuf::try_from("/tmp/deny-read-test").expect("absolute path");
        policy.append_deny_read_paths(&[path]);

        let result = reject_explicit_escalation_if_deny_read_present(
            SandboxPermissions::RequireEscalated,
            &policy,
        );

        let Err(FunctionCallError::RespondToModel(message)) = result else {
            panic!("expected managed deny_read explicit escalation rejection");
        };
        assert!(message.contains("filesystem deny_read policy is enforced"));
    }

    #[test]
    fn non_escalated_command_is_allowed_when_deny_read_paths_exist() {
        let mut policy = SandboxPolicy::new_workspace_write_policy();
        let path = AbsolutePathBuf::try_from("/tmp/deny-read-test").expect("absolute path");
        policy.append_deny_read_paths(&[path]);

        let result = reject_explicit_escalation_if_deny_read_present(
            SandboxPermissions::UseDefault,
            &policy,
        );

        assert!(result.is_ok());
    }
}
