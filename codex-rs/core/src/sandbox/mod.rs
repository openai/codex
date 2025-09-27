mod apply_patch_adapter;
mod backend;
mod planner;

pub use backend::BackendRegistry;
pub use backend::DirectBackend;
pub use backend::LinuxBackend;
pub use backend::SeatbeltBackend;
pub use backend::SpawnBackend;
pub use planner::ExecPlan;
pub use planner::ExecRequest;
pub use planner::PatchExecRequest;
pub(crate) use planner::PreparedExec;
pub use planner::plan_apply_patch;
pub use planner::plan_exec;
pub(crate) use planner::prepare_exec_invocation;

use crate::error::Result;
use crate::exec::ExecParams;
use crate::exec::ExecToolCallOutput;
use crate::exec::StdoutStream;
use crate::protocol::SandboxPolicy;

pub struct ExecRuntimeContext<'a> {
    pub sandbox_policy: &'a SandboxPolicy,
    pub sandbox_cwd: &'a std::path::Path,
    pub codex_linux_sandbox_exe: &'a Option<std::path::PathBuf>,
    pub stdout_stream: Option<StdoutStream>,
}

pub async fn run_with_plan(
    params: ExecParams,
    plan: &ExecPlan,
    registry: &BackendRegistry,
    runtime_ctx: &ExecRuntimeContext<'_>,
) -> Result<ExecToolCallOutput> {
    let ExecPlan::Approved { sandbox, .. } = plan else {
        unreachable!("run_with_plan called without approved plan");
    };

    registry
        .for_type(*sandbox)
        .spawn(
            params,
            runtime_ctx.sandbox_policy,
            runtime_ctx.sandbox_cwd,
            runtime_ctx.codex_linux_sandbox_exe,
            runtime_ctx.stdout_stream.clone(),
        )
        .await
}
