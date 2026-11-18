//! Prepare the Linux sandbox helper command for bubblewrap or legacy Landlock.

#[cfg(target_os = "linux")]
use crate::SandboxTransformError;
use codex_network_proxy::ManagedNetworkSandboxContext;
use codex_protocol::models::PermissionProfile;
use std::path::Path;

/// Managed proxy and socket grants belong to bubblewrap. Legacy Landlock is a compatibility mode.
pub(super) enum LinuxSandboxMode<'a> {
    Bubblewrap {
        managed_network: Option<&'a ManagedNetworkSandboxContext>,
    },
    LegacyLandlock,
}

impl<'a> LinuxSandboxMode<'a> {
    pub(super) fn resolve(
        use_legacy_landlock: bool,
        managed_network: Option<&'a ManagedNetworkSandboxContext>,
    ) -> Self {
        if use_legacy_landlock && managed_network.is_none() {
            Self::LegacyLandlock
        } else {
            Self::Bubblewrap { managed_network }
        }
    }

    #[cfg(target_os = "linux")]
    pub(super) fn ensure_supported_on_wsl1(
        &self,
        permission_profile: &PermissionProfile,
    ) -> Result<(), SandboxTransformError> {
        let requires_bubblewrap = match self {
            Self::Bubblewrap { managed_network } => {
                managed_network.is_some()
                    || !permission_profile
                        .file_system_sandbox_policy()
                        .has_full_disk_write_access()
            }
            Self::LegacyLandlock => false,
        };
        if requires_bubblewrap {
            return Err(SandboxTransformError::Wsl1UnsupportedForBubblewrap);
        }
        Ok(())
    }
}

/// Basename used when the Codex executable self-invokes as the Linux sandbox
/// helper.
pub const CODEX_LINUX_SANDBOX_ARG0: &str = "codex-linux-sandbox";

/// Converts the permission profile into the CLI invocation for
/// `codex-linux-sandbox`.
///
/// A managed context selects proxy isolation; a default context keeps its
/// restrictive policy when an older caller supplies no additional details.
///
/// The helper performs the actual sandboxing (bubblewrap by default + seccomp)
/// after parsing these arguments. The profile JSON flag is emitted before
/// helper feature flags so the argv order matches the helper's CLI shape. See
/// `docs/linux_sandbox.md` for the Linux semantics.
pub(super) fn create_linux_sandbox_command_args_for_permission_profile(
    command: Vec<String>,
    command_cwd: &Path,
    permission_profile: &PermissionProfile,
    sandbox_policy_cwd: &Path,
    mode: LinuxSandboxMode<'_>,
) -> Vec<String> {
    let permission_profile_json = serde_json::to_string(permission_profile)
        .unwrap_or_else(|err| panic!("failed to serialize permission profile: {err}"));
    let sandbox_policy_cwd = sandbox_policy_cwd
        .to_str()
        .unwrap_or_else(|| panic!("cwd must be valid UTF-8"))
        .to_string();
    let command_cwd = command_cwd
        .to_str()
        .unwrap_or_else(|| panic!("command cwd must be valid UTF-8"))
        .to_string();

    let mut linux_cmd: Vec<String> = vec![
        "--sandbox-policy-cwd".to_string(),
        sandbox_policy_cwd,
        "--command-cwd".to_string(),
        command_cwd,
        "--permission-profile".to_string(),
        permission_profile_json,
    ];
    match mode {
        LinuxSandboxMode::Bubblewrap {
            managed_network: Some(context),
        } => {
            linux_cmd.push("--managed-network".to_string());
            linux_cmd.push(serde_json::to_string(context).unwrap_or_else(|err| {
                panic!("failed to serialize managed network context: {err}")
            }));
        }
        LinuxSandboxMode::Bubblewrap {
            managed_network: None,
        } => {}
        LinuxSandboxMode::LegacyLandlock => linux_cmd.push("--use-legacy-landlock".to_string()),
    }
    linux_cmd.push("--".to_string());
    linux_cmd.extend(command);
    linux_cmd
}

#[cfg(test)]
#[path = "linux_tests.rs"]
mod tests;
