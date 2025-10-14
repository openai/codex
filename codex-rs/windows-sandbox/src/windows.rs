//! Codex Windows sandbox: same-user restricted token with per-directory allow lists.
//! - Writes are only permitted where the sandbox allow-list grants access to the Logon SID.
//! - “Deny most network” via env/proxy/tool flags (no firewall rules).
//! - Process tree attached to a Job Object with KILL_ON_JOB_CLOSE.
//! - No AppContainer; no global ACL edits; no extra users.

use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside a Windows restricted-token sandbox (no admin)"
)]
struct WindowsSandboxCommand {
    /// Working directory used when resolving relative sandbox policy paths.
    #[arg(long)]
    sandbox_policy_cwd: Option<PathBuf>,

    /// Sandbox policy to apply. Accepts preset names ('workspace-write', 'read-only', 'danger-full-access')
    /// or a JSON-encoded SandboxPolicy.
    pub sandbox_policy: SandboxPolicyArg,

    /// Command and arguments to execute once sandboxing is configured.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

#[derive(Debug, Clone)]
struct SandboxPolicyArg(SandboxPolicy);

impl SandboxPolicyArg {
    fn into_policy(self) -> SandboxPolicy {
        self.0
    }
}

impl std::str::FromStr for SandboxPolicyArg {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "read-only" => Ok(Self(SandboxPolicy::new_read_only_policy())),
            "workspace-write" => Ok(Self(SandboxPolicy::new_workspace_write_policy())),
            "danger-full-access" => Ok(Self(SandboxPolicy::DangerFullAccess)),
            other => serde_json::from_str::<SandboxPolicy>(other)
                .map(Self)
                .map_err(|err| format!("failed to parse sandbox policy: {err}")),
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum StdioPolicy {
    Inherit,
}

pub fn run() -> ! {
    let args = WindowsSandboxCommand::parse();
    let WindowsSandboxCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        command,
    } = args;

    let policy = sandbox_policy.into_policy();

    if command.is_empty() {
        eprintln!("No command specified to execute.");
        std::process::exit(2);
    }

    let current_dir = match env::current_dir() {
        Ok(p) => p,
        Err(e) => {
            eprintln!("failed to get current dir: {e}");
            std::process::exit(1);
        }
    };
    let sandbox_policy_cwd = sandbox_policy_cwd.unwrap_or_else(|| current_dir.clone());
    let env_map: HashMap<String, String> = env::vars().collect();

    let status = imp::spawn_command_under_windows_low_il(
        command,
        current_dir,
        &policy,
        sandbox_policy_cwd.as_path(),
        StdioPolicy::Inherit,
        env_map,
    );

    match status {
        Ok(exit_status) => {
            if let Some(code) = exit_status.code() {
                std::process::exit(code);
            }
            if exit_status.success() {
                std::process::exit(0);
            }
            std::process::exit(1);
        }
        Err(err) => {
            eprintln!("failed to run sandboxed command: {err}");
            std::process::exit(1);
        }
    }
}

// -------------------------- Windows implementation --------------------------
#[cfg(target_os = "windows")]
mod imp {
    use super::SandboxPolicy;
    use super::StdioPolicy;
    use std::collections::HashMap;
    use std::io::ErrorKind;
    use std::io::{self};
    use std::path::Path;
    use std::path::PathBuf;
    use std::process::ExitStatus;

    pub(super) fn spawn_command_under_windows_low_il(
        command: Vec<String>,
        command_cwd: PathBuf,
        policy: &SandboxPolicy,
        policy_cwd: &Path,
        stdio: StdioPolicy,
        mut env_map: HashMap<String, String>,
    ) -> io::Result<ExitStatus> {
        if command.is_empty() {
            return Err(io::Error::new(
                ErrorKind::InvalidInput,
                "command args are empty",
            ));
        }

        if !policy.has_full_network_access() {
            apply_best_effort_network_block(&mut env_map);
        }
        ensure_non_interactive_pager(&mut env_map);

        crate::windows_restricted_token_v2::spawn_command_under_restricted_token_v2(
            command,
            command_cwd,
            policy,
            policy_cwd,
            map_stdio_policy(stdio),
            env_map,
        )
    }

    fn map_stdio_policy(policy: StdioPolicy) -> crate::windows_restricted_token_v2::StdioPolicy {
        match policy {
            StdioPolicy::Inherit => crate::windows_restricted_token_v2::StdioPolicy::Inherit,
        }
    }

    fn ensure_non_interactive_pager(env_map: &mut HashMap<String, String>) {
        env_map
            .entry("GIT_PAGER".into())
            .or_insert_with(|| "more.com".into());
        env_map
            .entry("PAGER".into())
            .or_insert_with(|| "more.com".into());
        env_map.entry("LESS".into()).or_insert_with(String::new);
    }

    fn apply_best_effort_network_block(env_map: &mut HashMap<String, String>) {
        let sink = "http://127.0.0.1:9";
        env_map
            .entry("HTTP_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("HTTPS_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("ALL_PROXY".into())
            .or_insert_with(|| sink.into());
        env_map
            .entry("NO_PROXY".into())
            .or_insert_with(|| "localhost,127.0.0.1,::1".into());
        env_map
            .entry("PIP_NO_INDEX".into())
            .or_insert_with(|| "1".into());
        env_map
            .entry("PIP_DISABLE_PIP_VERSION_CHECK".into())
            .or_insert_with(|| "1".into());
        env_map
            .entry("NPM_CONFIG_OFFLINE".into())
            .or_insert_with(|| "true".into());
        env_map
            .entry("CARGO_NET_OFFLINE".into())
            .or_insert_with(|| "true".into());
    }
}
