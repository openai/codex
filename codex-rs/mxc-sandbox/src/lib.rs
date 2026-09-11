//! Native Windows MXC helper. Policy conversion is portable; execution requires
//! a usable process security environment and never enters MXC's ACL fallbacks.

#[cfg(windows)]
mod native;
#[cfg(any(windows, test))]
mod policy;
mod transport;
#[cfg(windows)]
mod windows;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

use anyhow::Context;
use anyhow::Result;
use anyhow::ensure;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::models::PermissionProfile;
use serde::Deserialize;
use serde::Serialize;

pub const CODEX_WINDOWS_MXC_ARG1: &str = "--__codex-windows-mxc";
const CLIENT_ONLY_LOOPBACK_UNSUPPORTED: &str = "MXC cannot enforce managed networking with allow_local_binding=false: native host-loopback access is bidirectional";

fn validate_managed_network(network: &ManagedNetworkSandboxContext) -> Result<()> {
    ensure!(
        !network.loopback_ports.is_empty(),
        "MXC managed networking requires dedicated proxy ports"
    );
    ensure!(
        !network.loopback_ports.contains(&0),
        "MXC proxy ports must be nonzero"
    );
    ensure!(
        network.allow_local_binding,
        CLIENT_ONLY_LOOPBACK_UNSUPPORTED
    );
    Ok(())
}

#[derive(Serialize, Deserialize)]
struct MxcCommand {
    permissions: PermissionProfile,
    sandbox_policy_cwd: PathBuf,
    managed_network: Option<ManagedNetworkSandboxContext>,
    command: Vec<String>,
}

/// Inputs used to build an MXC helper invocation.
#[derive(Debug)]
pub struct CreateMxcCommandArgsParams<'a> {
    pub command: Vec<String>,
    pub permission_profile: &'a PermissionProfile,
    pub sandbox_policy_cwd: &'a Path,
    pub managed_network: Option<&'a ManagedNetworkSandboxContext>,
    pub env: &'a mut HashMap<String, String>,
}

/// Wrap exact argv and add bounded launcher-only environment variables. The
/// helper removes these variables before starting the sandboxed command.
pub fn create_command_args(args: CreateMxcCommandArgsParams<'_>) -> Result<Vec<String>> {
    let CreateMxcCommandArgsParams {
        command,
        permission_profile,
        sandbox_policy_cwd,
        managed_network,
        env,
    } = args;
    sandbox_policy_cwd
        .to_str()
        .context("MXC requires a Unicode policy working directory")?;
    if let Some(network) = managed_network {
        validate_managed_network(network)?;
    }
    transport::encode(
        &MxcCommand {
            permissions: permission_profile.clone(),
            sandbox_policy_cwd: sandbox_policy_cwd.to_owned(),
            managed_network: managed_network.cloned(),
            command,
        },
        env,
    )?;
    Ok(vec![CODEX_WINDOWS_MXC_ARG1.to_owned()])
}

/// Whether the executor can create a native MXC process security environment.
/// This deliberately excludes MXC's older AppContainer fallback backends.
pub fn is_available() -> bool {
    #[cfg(windows)]
    {
        appcontainer_common::base_container_runner::BaseContainerRunner::is_process_security_environment_usable()
    }
    #[cfg(not(windows))]
    {
        false
    }
}

/// Entry point dispatched before ordinary Codex CLI parsing.
pub fn run_main() -> ! {
    #[cfg(windows)]
    {
        match windows::run() {
            Ok(exit_code) => std::process::exit(exit_code),
            Err(error) => {
                eprintln!("MXC sandbox: {error:#}");
                std::process::exit(1);
            }
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("MXC sandbox execution requires Windows");
        std::process::exit(1);
    }
}

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
