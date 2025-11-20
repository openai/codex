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

use clap::Parser;
use clap::Subcommand;
use tracing_subscriber::EnvFilter;
use tracing_subscriber::{self};

use crate::posix::escalate_protocol::EscalateAction;
use crate::posix::escalate_server::EscalateServer;
use crate::posix::escalation_policy::EscalationPolicy;
use crate::posix::mcp_escalation_policy::ExecPolicyOutcome;

mod escalate_client;
mod escalate_protocol;
mod escalate_server;
mod escalation_policy;
mod mcp;
mod mcp_escalation_policy;
mod socket;

#[derive(Parser)]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    subcommand: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Escalate(EscalateArgs),
    ShellExec(ShellExecArgs),
}

/// Invoked from within the sandbox to (potentially) escalate permissions.
#[derive(Parser, Debug)]
struct EscalateArgs {
    file: String,

    #[arg(trailing_var_arg = true)]
    argv: Vec<String>,
}

impl EscalateArgs {
    /// This is the escalate client. It talks to the escalate server to determine whether to exec()
    /// the command directly or to proxy to the escalate server.
    async fn run(self) -> anyhow::Result<i32> {
        let EscalateArgs { file, argv } = self;
        escalate_client::run(file, argv).await
    }
}

/// Debugging command to emulate an MCP "shell" tool call.
#[derive(Parser, Debug)]
struct ShellExecArgs {
    command: String,
}

#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    match cli.subcommand {
        Some(Commands::Escalate(args)) => {
            std::process::exit(args.run().await?);
        }
        Some(Commands::ShellExec(args)) => {
            let bash_path = mcp::get_bash_path()?;
            let escalate_server = EscalateServer::new(bash_path, DummyEscalationPolicy {});
            let result = escalate_server
                .exec(
                    args.command.clone(),
                    std::env::vars().collect(),
                    std::env::current_dir()?,
                    None,
                )
                .await?;
            println!("{result:?}");
            std::process::exit(result.exit_code);
        }
        None => {
            let bash_path = mcp::get_bash_path()?;

            tracing::info!("Starting MCP server");
            let service = mcp::serve(bash_path, dummy_exec_policy)
                .await
                .inspect_err(|e| {
                    tracing::error!("serving error: {:?}", e);
                })?;

            service.waiting().await?;
            Ok(())
        }
    }
}

// TODO: replace with execpolicy2

struct DummyEscalationPolicy;

#[async_trait::async_trait]
impl EscalationPolicy for DummyEscalationPolicy {
    async fn determine_action(
        &self,
        file: &Path,
        argv: &[String],
        workdir: &Path,
    ) -> Result<EscalateAction, rmcp::ErrorData> {
        let outcome = dummy_exec_policy(file, argv, workdir);
        let action = match outcome {
            ExecPolicyOutcome::Allow {
                run_with_escalated_permissions,
            } => {
                if run_with_escalated_permissions {
                    EscalateAction::Escalate
                } else {
                    EscalateAction::Run
                }
            }
            ExecPolicyOutcome::Forbidden => EscalateAction::Deny {
                reason: Some("Execution forbidden by policy".to_string()),
            },
            ExecPolicyOutcome::Prompt { .. } => EscalateAction::Deny {
                reason: Some("Could not prompt user for permission".to_string()),
            },
        };
        Ok(action)
    }
}

fn dummy_exec_policy(file: &Path, argv: &[String], _workdir: &Path) -> ExecPolicyOutcome {
    if file.ends_with("/rm") {
        ExecPolicyOutcome::Forbidden
    } else if file.ends_with("/git") {
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
