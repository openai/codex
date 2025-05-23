use codex_core::config::Config;
use codex_core::exec::StdioPolicy;
use codex_core::exec::spawn_command_under_seatbelt;
use codex_core::exec_env::create_env;

use crate::exit_status::handle_exit_status;

pub async fn run_seatbelt(command: Vec<String>, config: &Config) -> anyhow::Result<()> {
    let cwd = std::env::current_dir()?;
    let env = create_env(&config.shell_environment_policy);
    let mut child = spawn_command_under_seatbelt(
        command,
        &config.sandbox_policy,
        cwd,
        StdioPolicy::Inherit,
        env,
    )
    .await?;
    let status = child.wait().await?;
    handle_exit_status(status);
}
