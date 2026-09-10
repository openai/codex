//! Native Windows process security environment availability and launch.

#[cfg(windows)]
pub mod native;

use codex_protocol::models::PermissionProfile;
use std::path::PathBuf;

/// Typed inputs for the native policy adapter.
pub mod transport;
use serde::Deserialize;
use serde::Serialize;

#[derive(Serialize, Deserialize)]
pub struct MxcCommand {
    pub permissions: PermissionProfile,
    pub sandbox_policy_cwd: PathBuf,
    pub command: Vec<String>,
}

pub mod policy;

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

#[cfg(test)]
#[path = "policy_tests.rs"]
mod tests;
