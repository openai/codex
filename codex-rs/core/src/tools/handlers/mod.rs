pub mod apply_patch;
pub(crate) mod collab;
mod dynamic;
mod grep_files;
mod list_dir;
mod mcp;
mod mcp_resource;
mod plan;
mod read_file;
mod request_user_input;
mod shell;
mod test_sync;
mod unified_exec;
mod view_image;

pub use plan::PLAN_TOOL;
use serde::Deserialize;

use crate::function_tool::FunctionCallError;
pub use apply_patch::ApplyPatchHandler;
pub use collab::CollabHandler;
pub use dynamic::DynamicToolHandler;
pub use grep_files::GrepFilesHandler;
pub use list_dir::ListDirHandler;
pub use mcp::McpHandler;
pub use mcp_resource::McpResourceHandler;
pub use plan::PlanHandler;
pub use read_file::ReadFileHandler;
pub use request_user_input::RequestUserInputHandler;
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
