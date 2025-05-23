use std::io;
use std::path::Path;
use std::sync::Arc;

use crate::error::CodexErr;
use crate::error::Result;
use crate::exec::ExecParams;
use crate::exec::RawExecToolCallOutput;
use crate::exec::StdioPolicy;
use crate::exec::consume_truncated_output;
use crate::exec::spawn_child_async;
use crate::protocol::SandboxPolicy;

use tokio::sync::Notify;

pub fn exec_linux(
    params: ExecParams,
    ctrl_c: Arc<Notify>,
    sandbox_policy: &SandboxPolicy,
) -> Result<RawExecToolCallOutput> {
    // Allow READ on /
    // Allow WRITE on /dev/null
    let ctrl_c_copy = ctrl_c.clone();
    let sandbox_policy = sandbox_policy.clone();

    // Isolate thread to run the sandbox from
    let tool_call_output = std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()?;

        rt.block_on(async {
            let ExecParams {
                command,
                cwd,
                timeout_ms,
                env,
            } = params;
            apply_sandbox_policy_to_current_thread(&sandbox_policy, &cwd)?;
            let child = spawn_child_async(
                command,
                cwd,
                &sandbox_policy,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;
            consume_truncated_output(child, ctrl_c_copy, timeout_ms).await
        })
    })
    .join();

    match tool_call_output {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(e)) => Err(e),
        Err(e) => Err(CodexErr::Io(io::Error::other(format!(
            "thread join failed: {e:?}"
        )))),
    }
}

#[cfg(target_os = "linux")]
pub fn apply_sandbox_policy_to_current_thread(
    sandbox_policy: &SandboxPolicy,
    cwd: &Path,
) -> Result<()> {
    crate::landlock::apply_sandbox_policy_to_current_thread(sandbox_policy, cwd)
}

#[cfg(not(target_os = "linux"))]
pub fn apply_sandbox_policy_to_current_thread(
    _sandbox_policy: &SandboxPolicy,
    _cwd: &Path,
) -> Result<()> {
    Err(CodexErr::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        "linux sandbox is not supported on this platform",
    )))
}
