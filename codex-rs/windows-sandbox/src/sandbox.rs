// Cargo.toml (for this crate)
// [dependencies]
// windows = { version = "0.56", features = [
//   "Win32_Foundation", "Win32_Security", "Win32_Security_Authorization", "Win32_System_Threading",
//   "Win32_System_JobObjects", "Win32_System_Console", "Win32_System_Com", "Win32_NetworkManagement_WindowsFirewall",
//   "Win32_Networking_WinSock", "Win32_System_Environment", "Win32_Security_Credentials",
//   "Win32_System_SystemServices", "Win32_System_Memory", "Win32_Storage_FileSystem",
//   "Win32_NetworkManagement_NetManagement", "Win32_System_UserEnv"
// ] }
// rand = "0.8"
// anyhow = "1"
// thiserror = "1"
// tracing = "0.1"

use crate::windows_restricted_token_v2;
use anyhow::Context;
use anyhow::Result;
use clap::Parser;
use codex_protocol::protocol::SandboxPolicy;
use std::collections::HashMap;
use std::env;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;

#[derive(Debug, Clone, Copy)]
pub(crate) enum StdioPolicy {
    Inherit,
}

#[derive(Debug, Parser)]
#[command(
    name = "codex-windows-sandbox",
    about = "Run a command inside a Windows restricted-token sandbox."
)]
struct WindowsSandboxCommand {
    /// Working directory that should be used when resolving relative sandbox policy paths.
    #[arg(long)]
    sandbox_policy_cwd: Option<PathBuf>,

    /// JSON-encoded SandboxPolicy definition.
    pub sandbox_policy: SandboxPolicy,

    /// Command and arguments to execute once sandboxing is configured.
    #[arg(trailing_var_arg = true, allow_hyphen_values = true, required = true)]
    pub command: Vec<String>,
}

pub fn run_main() -> ! {
    let args = WindowsSandboxCommand::parse();
    let WindowsSandboxCommand {
        sandbox_policy_cwd,
        sandbox_policy,
        command,
    } = args;

    if command.is_empty() {
        panic!("No command specified to execute.");
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

    let status = spawn_command_under_windows_low_il(
        command,
        current_dir,
        &sandbox_policy,
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

pub fn spawn_command_under_windows_low_il(
    command: Vec<String>,
    command_cwd: PathBuf,
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    stdio: StdioPolicy,
    env_map: HashMap<String, String>,
) -> Result<ExitStatus> {
    if command.is_empty() {
        anyhow::bail!("command args are empty");
    }

    // 1) Decide policy
    let mut env_map = env_map;
    if !sandbox_policy.has_full_network_access() {
        apply_best_effort_network_block(&mut env_map);
    }
    ensure_non_interactive_pager(&mut env_map);

    windows_restricted_token_v2::spawn_command_under_restricted_token_v2(
        command,
        command_cwd,
        sandbox_policy,
        sandbox_policy_cwd,
        map_stdio_policy(stdio),
        env_map,
    )
    .context("failed to spawn restricted-token sandbox process")
}

fn map_stdio_policy(policy: StdioPolicy) -> windows_restricted_token_v2::StdioPolicy {
    match policy {
        StdioPolicy::Inherit => windows_restricted_token_v2::StdioPolicy::Inherit,
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
