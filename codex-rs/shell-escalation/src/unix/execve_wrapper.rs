//! Entrypoints for execve interception helper binaries.

use clap::Parser;
use tracing_subscriber::EnvFilter;

#[derive(Parser)]
pub struct ExecveWrapperCli {
    file: String,

    #[arg(trailing_var_arg = true)]
    argv: Vec<String>,
}

#[tokio::main]
pub async fn main_execve_wrapper() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let ExecveWrapperCli { file, argv } = ExecveWrapperCli::parse();
    let exit_code = crate::run_shell_escalation_execve_wrapper(file, argv).await?;
    std::process::exit(exit_code);
}
