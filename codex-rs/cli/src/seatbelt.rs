use codex_core::exec::StdioPolicy;
use codex_core::exec::spawn_command_under_seatbelt;
use codex_core::protocol::SandboxPolicy;

use crate::exit_status::handle_exit_status;

pub async fn run_seatbelt(
    command: Vec<String>,
    sandbox_policy: SandboxPolicy,
) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let mut child =
        spawn_command_under_seatbelt(command, &sandbox_policy, cwd, StdioPolicy::Inherit).await?;
    let status = child.wait().await?;
    handle_exit_status(status);
}
