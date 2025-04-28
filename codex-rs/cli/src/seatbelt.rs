use codex_core::exec::create_seatbelt_command;
use codex_core::protocol::SandboxPolicy;
use std::path::PathBuf;

pub(crate) async fn run_seatbelt(
    command: Vec<String>,
    sandbox_policy: SandboxPolicy,
    writable_roots: Vec<PathBuf>,
) -> anyhow::Result<()> {
    let seatbelt_command = create_seatbelt_command(command, sandbox_policy, &writable_roots);
    let status = tokio::process::Command::new(seatbelt_command[0].clone())
        .args(&seatbelt_command[1..])
        .spawn()
        .map_err(|e| anyhow::anyhow!("Failed to spawn command: {}", e))?
        .wait()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to wait for command: {}", e))?;
    std::process::exit(status.code().unwrap_or(1));
}
