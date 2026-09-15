//! Resolves an executor sandbox context to a concrete local sandbox implementation.

use codex_file_system::FileSystemSandboxContext;
use codex_file_system::WindowsSandboxSelection;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;

pub(crate) fn select_sandbox(
    manager: &SandboxManager,
    permission_profile: &PermissionProfile,
    sandbox_context: &FileSystemSandboxContext,
    has_managed_network_requirements: bool,
) -> (SandboxType, Option<WindowsSandboxLevel>) {
    let windows_sandbox_level = match sandbox_context.windows_sandbox_selection {
        WindowsSandboxSelection::Disabled => WindowsSandboxLevel::Disabled,
        WindowsSandboxSelection::RestrictedToken => WindowsSandboxLevel::RestrictedToken,
        WindowsSandboxSelection::Elevated => WindowsSandboxLevel::Elevated,
        WindowsSandboxSelection::Mxc => return (SandboxType::WindowsMxc, None),
    };
    let sandbox_type = manager.select_initial(
        permission_profile,
        SandboxablePreference::Require,
        windows_sandbox_level,
        has_managed_network_requirements,
    );
    (sandbox_type, Some(windows_sandbox_level))
}
