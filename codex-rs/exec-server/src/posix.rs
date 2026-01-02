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
use std::sync::Arc;

use anyhow::Context as _;
use clap::Parser;
use codex_core::config::find_codex_home;
use codex_core::is_dangerous_command::command_might_be_dangerous;
use codex_core::sandboxing::SandboxPermissions;
use codex_execpolicy::Decision;
use codex_execpolicy::Policy;
use codex_execpolicy::RuleMatch;
use rmcp::ErrorData as McpError;
use tokio::sync::RwLock;
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

pub use mcp::ExecResult;

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

    /// Preserve program paths when applying execpolicy (e.g., keep /usr/bin/echo instead of echo).
    /// Note: this does change the actual program being run.
    #[arg(long)]
    preserve_program_paths: bool,
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
    let policy = Arc::new(RwLock::new(load_exec_policy().await?));

    tracing::info!("Starting MCP server");
    let service = mcp::serve(
        bash_path,
        execve_wrapper,
        policy,
        cli.preserve_program_paths,
    )
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

/// Decide how to handle an exec() call for a specific command.
///
/// `file` is the absolute, canonical path to the executable to run, i.e. the first arg to exec.
/// `argv` is the argv, including the program name (`argv[0]`).
pub(crate) fn evaluate_exec_policy(
    policy: &Policy,
    file: &Path,
    argv: &[String],
    preserve_program_paths: bool,
) -> Result<ExecPolicyOutcome, McpError> {
    let program_name = format_program_name(file, preserve_program_paths).ok_or_else(|| {
        McpError::internal_error(
            format!("failed to format program name for `{}`", file.display()),
            None,
        )
    })?;
    let command: Vec<String> = std::iter::once(program_name)
        // Use the normalized program name instead of argv[0].
        .chain(argv.iter().skip(1).cloned())
        .collect();
    let evaluation = policy.check(&command, &|cmd| {
        if command_might_be_dangerous(cmd) {
            Decision::Prompt
        } else {
            Decision::Allow
        }
    });

    // decisions driven by policy should run outside sandbox
    let decision_driven_by_policy = evaluation.matched_rules.iter().any(|rule_match| {
        !matches!(rule_match, RuleMatch::HeuristicsRuleMatch { .. })
            && rule_match.decision() == evaluation.decision
    });

    let sandbox_permissions = if decision_driven_by_policy {
        SandboxPermissions::RequireEscalated
    } else {
        SandboxPermissions::UseDefault
    };

    Ok(match evaluation.decision {
        Decision::Forbidden => ExecPolicyOutcome::Forbidden,
        Decision::Prompt => ExecPolicyOutcome::Prompt {
            sandbox_permissions,
        },
        Decision::Allow => ExecPolicyOutcome::Allow {
            sandbox_permissions,
        },
    })
}

fn format_program_name(path: &Path, preserve_program_paths: bool) -> Option<String> {
    if preserve_program_paths {
        path.to_str().map(str::to_string)
    } else {
        path.file_name()?.to_str().map(str::to_string)
    }
}

async fn load_exec_policy() -> anyhow::Result<Policy> {
    let codex_home = find_codex_home().context("failed to resolve codex_home for execpolicy")?;

    // TODO(mbolin): At a minimum, `cwd` should be configurable via
    // `codex/sandbox-state/update` or some other custom MCP call.
    let cwd = None;
    let cli_overrides = Vec::new();
    let overrides = codex_core::config_loader::LoaderOverrides::default();
    let config_layer_stack = codex_core::config_loader::load_config_layers_state(
        &codex_home,
        cwd,
        &cli_overrides,
        overrides,
    )
    .await?;

    codex_core::load_exec_policy(&config_layer_stack)
        .await
        .map_err(anyhow::Error::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_core::sandboxing::SandboxPermissions;
    use codex_execpolicy::Decision;
    use codex_execpolicy::Policy;
    use pretty_assertions::assert_eq;
    use std::path::Path;

    #[test]
    fn evaluate_exec_policy_uses_heuristics_for_dangerous_commands() {
        let policy = Policy::empty();
        let file = Path::new("/bin/rm");
        let argv = vec!["rm".to_string(), "-rf".to_string(), "/".to_string()];

        let outcome = evaluate_exec_policy(&policy, file, &argv, false).expect("policy evaluation");

        assert_eq!(
            outcome,
            ExecPolicyOutcome::Prompt {
                sandbox_permissions: SandboxPermissions::UseDefault
            }
        );
    }

    #[test]
    fn evaluate_exec_policy_respects_preserve_program_paths() {
        let mut policy = Policy::empty();
        policy
            .add_prefix_rule(
                &[
                    "/usr/local/bin/custom-cmd".to_string(),
                    "--flag".to_string(),
                ],
                Decision::Allow,
            )
            .expect("policy rule should be added");
        let file = Path::new("/usr/local/bin/custom-cmd");
        let argv = vec![
            "/usr/local/bin/custom-cmd".to_string(),
            "--flag".to_string(),
            "value".to_string(),
        ];

        let outcome = evaluate_exec_policy(&policy, file, &argv, true).expect("policy evaluation");

        assert_eq!(
            outcome,
            ExecPolicyOutcome::Allow {
                sandbox_permissions: SandboxPermissions::RequireEscalated
            }
        );
    }
}
