//! Translates Windows sandbox selection and validates explicit remote MXC requests.

use codex_file_system::FileSystemSandboxContext;
use codex_file_system::WindowsSandboxSelection;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_sandboxing::SandboxSelection;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;

pub(crate) fn validate_remote_mxc(info: &crate::EnvironmentInfo) -> std::io::Result<()> {
    if !info.capabilities.windows_mxc {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "exec-server does not support the requested MXC Windows sandbox; update the executor",
        ));
    }
    Ok(())
}

pub(crate) fn windows_sandbox_backend(selection: WindowsSandboxSelection) -> SandboxType {
    match selection {
        WindowsSandboxSelection::Disabled => SandboxType::None,
        WindowsSandboxSelection::RestrictedToken | WindowsSandboxSelection::Elevated => {
            SandboxType::WindowsRestrictedToken
        }
        WindowsSandboxSelection::Mxc => SandboxType::WindowsMxc,
    }
}

pub(crate) fn sandbox_selection(
    context: &FileSystemSandboxContext,
) -> (SandboxSelection, WindowsSandboxLevel) {
    let windows_sandbox = windows_sandbox_backend(context.windows_sandbox_selection);
    let level = match context.windows_sandbox_selection {
        WindowsSandboxSelection::Disabled => WindowsSandboxLevel::Disabled,
        WindowsSandboxSelection::RestrictedToken => WindowsSandboxLevel::RestrictedToken,
        WindowsSandboxSelection::Elevated => WindowsSandboxLevel::Elevated,
        WindowsSandboxSelection::Mxc => {
            return (
                SandboxSelection::Explicit(windows_sandbox),
                WindowsSandboxLevel::Disabled,
            );
        }
    };
    (
        SandboxSelection::Platform {
            preference: SandboxablePreference::Require,
            windows_sandbox,
        },
        level,
    )
}
