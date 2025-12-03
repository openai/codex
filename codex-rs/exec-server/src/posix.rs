//! This is an MCP that implements an alternative `shell` tool with fine-grained privilege
//! escalation based on a per-exec() policy.
//!
//! We spawn Bash process inside a sandbox. The Bash we spawn is patched to allow us to intercept
//! every exec() call it makes by invoking a wrapper program and passing in the arguments it would
//! have passed to exec(). The Bash process (and its descendants) inherit a communication socket
//! from us, and we give its fd number in the CODEX_ESCALATE_SOCKET environment variable.
//!
//! When we intercept an exec() call, we send a message over the socket back to the main
//! MCP process. The MCP process can then decide whether to allow the exec() call to proceed
//! or to escalate privileges and run the requested command with elevated permissions. In the
//! latter case, we send a message back to the child requesting that it forward its open FDs to us.
//! We then execute the requested command on its behalf, patching in the forwarded FDs.
//!
//!
//! ### The privilege escalation flow
//!
//! Child  MCP   Bash   Escalate Helper
//!         |
//!         o----->o
//!         |      |
//!         |      o--(exec)-->o
//!         |      |           |
//!         |o<-(EscalateReq)--o
//!         ||     |           |
//!         |o--(Escalate)---->o
//!         ||     |           |
//!         |o<---------(fds)--o
//!         ||     |           |
//!   o<-----o     |           |
//!   |     ||     |           |
//!   x----->o     |           |
//!         ||     |           |
//!         |x--(exit code)--->o
//!         |      |           |
//!         |      o<--(exit)--x
//!         |      |
//!         o<-----x
//!
//! ### The non-escalation flow
//!
//!  MCP   Bash   Escalate Helper   Child
//!   |
//!   o----->o
//!   |      |
//!   |      o--(exec)-->o
//!   |      |           |
//!   |o<-(EscalateReq)--o
//!   ||     |           |
//!   |o-(Run)---------->o
//!   |      |           |
//!   |      |           x--(exec)-->o
//!   |      |                       |
//!   |      o<--------------(exit)--x
//!   |      |
//!   o<-----x
//!
use std::path::Path;
use std::path::PathBuf;

use clap::Parser;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{self};

use crate::posix::mcp_escalation_policy::ExecPolicyOutcome;

mod escalate_client;
mod escalate_protocol;
mod escalate_server;
mod escalation_policy;
mod mcp;
mod mcp_escalation_policy;
mod socket;
mod stopwatch;

/// Default value of --execve option relative to the current executable.
/// Note this must match the name of the binary as specified in Cargo.toml.
const CODEX_EXECVE_WRAPPER_EXE_NAME: &str = "codex-execve-wrapper";

#[derive(Parser)]
#[clap(version)]
struct McpServerCli {
    /// Executable to delegate execve(2) calls to in Bash.
    #[arg(long = "execve")]
    execve_wrapper: Option<PathBuf>,

    /// Path to Bash that has been patched to support execve() wrapping.
    #[arg(long = "bash")]
    bash_path: Option<PathBuf>,
}

#[tokio::main]
pub async fn main_mcp_server() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let cli = McpServerCli::parse();
    let execve_wrapper = match cli.execve_wrapper {
        Some(path) => path,
        None => {
            let cwd = std::env::current_exe()?;
            cwd.parent()
                .map(|p| p.join(CODEX_EXECVE_WRAPPER_EXE_NAME))
                .ok_or_else(|| {
                    anyhow::anyhow!("failed to determine execve wrapper path from current exe")
                })?
        }
    };
    let bash_path = match cli.bash_path {
        Some(path) => path,
        None => mcp::get_bash_path()?,
    };

    tracing::info!("Starting MCP server");
    let service = mcp::serve(bash_path, execve_wrapper, dummy_exec_policy)
        .await
        .inspect_err(|e| {
            tracing::error!("serving error: {:?}", e);
        })?;

    service.waiting().await?;
    Ok(())
}

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
    let exit_code = escalate_client::run(file, argv).await?;
    std::process::exit(exit_code);
}

// TODO: replace with execpolicy

fn dummy_exec_policy(file: &Path, argv: &[String], _workdir: &Path) -> ExecPolicyOutcome {
    if file.ends_with("rm") {
        ExecPolicyOutcome::Forbidden
    } else if file.ends_with("git") {
        ExecPolicyOutcome::Prompt {
            run_with_escalated_permissions: false,
        }
    } else if file == Path::new("/opt/homebrew/bin/gh")
        && let [_, arg1, arg2, ..] = argv
        && arg1 == "issue"
        && arg2 == "list"
    {
        ExecPolicyOutcome::Allow {
            run_with_escalated_permissions: true,
        }
    } else {
        ExecPolicyOutcome::Allow {
            run_with_escalated_permissions: false,
        }
    }
}
