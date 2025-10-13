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

use crate::low_integrity;
use crate::process;
use crate::temp_user;
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
    about = "Run a command inside a Windows AppContainer sandbox."
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
    let wants_network = sandbox_policy.has_full_network_access(); // same as mac/linux flags
    let (writable_roots, read_only_overrides) =
        roots_from_policy(sandbox_policy, sandbox_policy_cwd);

    // 2) Create / reuse ephemeral user
    let user =
        temp_user::EphemeralUser::create().context("failed to create ephemeral sandbox user")?;

    // 3) Network isolation (skip if policy says full network)
    let _fw_guard: Option<crate::firewall::OutboundBlockGuard> = if wants_network {
        None
    } else {
        // Temporarily disable firewall installation while investigating setup failures.
        Some(
            firewall::install_for_user(&user)
                .context("failed to install per-user firewall block rule")?,
        )
    };

    low_integrity::enable_required_privileges()
        .context("failed to enable SeSecurityPrivilege for integrity adjustments")?;

    // 4) Mark allowed roots Low-Integrity (writeable by Low-IL)
    for root in writable_roots.iter() {
        low_integrity::ensure_low_integrity_dir(root)
            .with_context(|| format!("failed to mark low-integrity: {}", root.display()))?;
    }
    for ro in read_only_overrides.iter() {
        // Explicitly force Medium IL if needed
        low_integrity::ensure_medium_integrity_dir(ro)
            .with_context(|| format!("failed to mark medium-integrity: {}", ro.display()))?;
    }

    // 5) Launch under Low IL + job object
    process::spawn_as_user_low_il(&user, command, command_cwd, stdio, env_map)
}
fn roots_from_policy(policy: &SandboxPolicy, base: &Path) -> (Vec<PathBuf>, Vec<PathBuf>) {
    match policy {
        SandboxPolicy::DangerFullAccess => (vec![], vec![]),
        SandboxPolicy::ReadOnly => (vec![], vec![]),
        SandboxPolicy::WorkspaceWrite { .. } => {
            let roots = policy.get_writable_roots_with_cwd(base);
            let mut allows = Vec::new();
            let mut ro = Vec::new();
            for w in roots {
                allows.push(w.root);
                ro.extend(w.read_only_subpaths);
            }
            (allows, ro)
        }
    }
}
