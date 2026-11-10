//! Bind filesystem and network inputs for sandbox preparation on the execution host.

use crate::SandboxExecRequest;
use crate::SandboxType;
use std::ffi::OsStr;
use std::ffi::OsString;
use std::io;
use std::process::Command;

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

/// A command prepared for its selected sandbox. Native Windows token sessions
/// must use the Windows session launcher.
#[must_use]
pub struct WrappedSandboxCommand {
    pub(super) request: SandboxExecRequest,
    // UTF-8 argv cannot represent every native executable path.
    pub(super) native_program: Option<OsString>,
}

impl WrappedSandboxCommand {
    /// Return the request with UTF-8 argv for callers that spawn the command themselves.
    pub fn into_command_request(self) -> SandboxExecRequest {
        self.request
    }

    /// Return a configured process command when argv carries all required sandbox enforcement.
    pub fn into_direct_command(self) -> io::Result<Command> {
        let request = self.request;
        if request.sandbox == SandboxType::WindowsRestrictedToken {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "a native Windows sandbox requires its session launcher",
            ));
        }
        let (program, args) = request.command.split_first().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "sandbox command is empty")
        })?;
        let mut command = Command::new(
            self.native_program
                .as_deref()
                .unwrap_or_else(|| OsStr::new(program)),
        );
        #[cfg(unix)]
        if let Some(arg0) = request.arg0 {
            use std::os::unix::process::CommandExt;
            command.arg0(arg0);
        }
        command
            .args(args)
            .current_dir(request.cwd.to_abs_path()?)
            .env_clear()
            .envs(request.env);
        Ok(command)
    }
}

/// The authorized profile, resolved on the host that will enforce it.
#[derive(Debug)]
pub(super) struct PreparedLaunchPolicy {
    pub(super) permissions: PermissionProfile,
}

impl PreparedLaunchPolicy {
    /// An empty workspace list stays empty, even when the policy contains `:workspace_roots`.
    pub(super) fn resolve(
        permissions: PermissionProfile,
        policy_cwd: &PathUri,
        roots: Option<&[PathUri]>,
        runtime_readable_paths: &[AbsolutePathBuf],
    ) -> io::Result<Self> {
        let Some(roots) = roots else {
            if !runtime_readable_paths.is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "runtime read access requires explicit workspace roots",
                ));
            }
            return Ok(Self { permissions });
        };
        let cwd = native_path(policy_cwd, "sandbox policy cwd")?;
        let workspace_roots = native_workspace_roots(roots)?;
        let (file_system, network) = permissions.to_runtime_permissions();
        for entry in &file_system.entries {
            if let FileSystemPath::Path { path } = &entry.path {
                native_path(path, "sandbox permission path")?;
            }
        }
        let file_system = file_system
            .materialize_project_roots_with_workspace_roots(&workspace_roots)
            .with_additional_readable_roots(&cwd, runtime_readable_paths);
        let permissions = PermissionProfile::from_runtime_permissions_with_enforcement(
            permissions.enforcement(),
            &file_system,
            network,
        );
        Ok(Self { permissions })
    }
}

pub(super) fn native_workspace_roots(roots: &[PathUri]) -> io::Result<Vec<AbsolutePathBuf>> {
    roots
        .iter()
        .map(|root| native_path(root, "sandbox workspace root"))
        .collect::<io::Result<Vec<_>>>()
}

fn native_path(path: &PathUri, field: &str) -> io::Result<AbsolutePathBuf> {
    path.to_abs_path()
        .map_err(|err| io::Error::new(err.kind(), format!("invalid {field}: {err}")))
}
