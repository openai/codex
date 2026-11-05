//! Bind filesystem and network inputs for sandbox preparation on the execution host.

use std::io;

use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPath;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_path_uri::PathUri;

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
    ) -> io::Result<Self> {
        let Some(roots) = roots else {
            return Ok(Self { permissions });
        };
        native_path(policy_cwd, "sandbox policy cwd")?;
        let workspace_roots = native_workspace_roots(roots)?;
        let (file_system, network) = permissions.to_runtime_permissions();
        for entry in &file_system.entries {
            if let FileSystemPath::Path { path } = &entry.path {
                native_path(path, "sandbox permission path")?;
            }
        }
        let file_system =
            file_system.materialize_project_roots_with_workspace_roots(&workspace_roots);
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
