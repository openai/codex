#[cfg(windows)]
use crate::SandboxExecRequest;
#[cfg(windows)]
use crate::SandboxTransformError;
use crate::WindowsSandboxProxySettingsMode;
#[cfg(windows)]
use crate::manager::SandboxBackend;
#[cfg(windows)]
use codex_network_proxy::NetworkProxy;
use std::collections::BTreeSet;
use std::collections::HashMap;
#[cfg(windows)]
use std::io;
use std::path::Path;
use std::path::PathBuf;

use codex_network_proxy::ManagedProxyRouting;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::PermissionProfile;
use codex_protocol::permissions::FileSystemPolicyProjection;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::compatibility_writable_roots_for_permission_profile;
use codex_protocol::protocol::WritableRoot;
use codex_utils_absolute_path::AbsolutePathBuf;

use crate::SandboxType;

pub(super) struct WindowsSessionPolicy {
    pub(super) workspace_roots: Vec<AbsolutePathBuf>,
    pub(super) filesystem_overrides: Option<WindowsSandboxFilesystemOverrides>,
    pub(super) proxy_enforced: bool,
    #[cfg(windows)]
    pub(super) restricting_sid: Option<String>,
    pub(super) proxy_settings_mode: WindowsSandboxProxySettingsMode,
}

#[cfg(windows)]
pub(super) struct WindowsToken<'a> {
    pub(super) proxy_settings: Option<WindowsSandboxProxySettingsMode>,
    pub(super) file_system_helper: bool,
    pub(super) sandbox_exe: Option<&'a Path>,
    pub(super) workspace_roots: Vec<AbsolutePathBuf>,
    pub(super) enforce_managed_network: bool,
    pub(super) network: Option<&'a NetworkProxy>,
    pub(super) environment_id: Option<&'a str>,
    pub(super) session: &'a mut Option<Box<WindowsSessionPolicy>>,
}

#[cfg(windows)]
impl SandboxBackend for WindowsToken<'_> {
    fn prepare(
        &mut self,
        exec: &mut SandboxExecRequest,
        _cwd: &AbsolutePathBuf,
        policy_cwd: &AbsolutePathBuf,
    ) -> Result<(), SandboxTransformError> {
        if self.file_system_helper {
            let codex_home = codex_utils_home_dir::find_codex_home()
                .map_err(|err| SandboxTransformError::WindowsSandboxPreparation(err.to_string()))?;
            crate::manager::wrap_windows_file_system_helper(
                exec,
                &self.workspace_roots,
                &codex_home,
                self.sandbox_exe,
            )?;
        } else if let Some(proxy_settings_mode) = self.proxy_settings {
            let filesystem_overrides = WindowsSandboxFilesystemOverrides::resolve(
                SandboxType::WindowsRestrictedToken,
                &exec.permission_profile,
                policy_cwd,
                exec.windows_sandbox_level,
                &exec.env,
            )
            .map_err(|err| SandboxTransformError::InvalidPolicy(io::Error::other(err)))?;
            *self.session = Some(Box::new(WindowsSessionPolicy {
                workspace_roots: std::mem::take(&mut self.workspace_roots),
                filesystem_overrides,
                proxy_enforced: self.enforce_managed_network,
                restricting_sid: crate::manager::windows_proxy_restricting_sid(
                    self.network,
                    self.environment_id,
                )?,
                proxy_settings_mode,
            }));
        }
        Ok(())
    }
}

/// Resolved filesystem overrides for the Windows sandbox backends.
///
/// The elevated Windows backend consumes extra deny-read paths plus explicit
/// read and write roots during setup/refresh. The unelevated restricted-token
/// backend only consumes extra deny-write carveouts on top of the legacy
/// `WorkspaceWrite` allow set. Read-root overrides are layered on top of the
/// baseline helper roots that the elevated setup path needs to launch the
/// sandboxed command; split policies that opt into platform defaults carry
/// that explicitly with the override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WindowsSandboxFilesystemOverrides {
    pub read_roots_override: Option<Vec<PathBuf>>,
    pub read_roots_include_platform_defaults: bool,
    pub write_roots_override: Option<Vec<PathBuf>>,
    pub additional_deny_read_paths: Vec<AbsolutePathBuf>,
    pub additional_deny_write_paths: Vec<AbsolutePathBuf>,
}

impl WindowsSandboxFilesystemOverrides {
    /// Resolve filesystem overrides for the selected Windows sandbox using the child's environment.
    pub fn resolve(
        sandbox: SandboxType,
        permission_profile: &PermissionProfile,
        sandbox_policy_cwd: &AbsolutePathBuf,
        windows_sandbox_level: WindowsSandboxLevel,
        workload_env: &HashMap<String, String>,
    ) -> Result<Option<Self>, String> {
        let use_elevated = windows_sandbox_uses_elevated_backend(windows_sandbox_level);
        if use_elevated {
            resolve_windows_elevated_filesystem_overrides(
                sandbox,
                permission_profile,
                sandbox_policy_cwd,
                use_elevated,
                workload_env,
            )
        } else {
            resolve_windows_restricted_token_filesystem_overrides(
                sandbox,
                permission_profile,
                sandbox_policy_cwd,
                windows_sandbox_level,
            )
        }
    }
}

pub fn windows_sandbox_uses_elevated_backend(sandbox_level: WindowsSandboxLevel) -> bool {
    matches!(sandbox_level, WindowsSandboxLevel::Elevated)
}

/// MXC uses dedicated proxy listeners when the proxy runs on Windows.
pub fn managed_proxy_routing_for_windows_sandbox(sandbox_type: SandboxType) -> ManagedProxyRouting {
    if cfg!(windows) && sandbox_type == SandboxType::WindowsMxc {
        ManagedProxyRouting::DedicatedListeners
    } else {
        ManagedProxyRouting::SharedIngress
    }
}

/// A native Windows process has no sandbox token SID for shared proxy ingress.
/// An explicit sandbox context can request shared ingress even with Windows sandboxing disabled.
pub fn executor_managed_proxy_routing(
    windows_sandbox: Option<SandboxType>,
) -> Option<ManagedProxyRouting> {
    match windows_sandbox {
        Some(sandbox) => Some(managed_proxy_routing_for_windows_sandbox(sandbox)),
        None if cfg!(windows) => None,
        None => Some(ManagedProxyRouting::SharedIngress),
    }
}

pub fn permission_profile_supports_windows_restricted_token_sandbox(
    permission_profile: &PermissionProfile,
) -> bool {
    match permission_profile {
        PermissionProfile::Managed { file_system, .. } => {
            !file_system.to_sandbox_policy().has_full_disk_write_access()
        }
        PermissionProfile::Disabled | PermissionProfile::External { .. } => false,
    }
}

pub fn resolve_windows_restricted_token_filesystem_overrides(
    sandbox: SandboxType,
    permission_profile: &PermissionProfile,
    sandbox_policy_cwd: &AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> std::result::Result<Option<WindowsSandboxFilesystemOverrides>, String> {
    if sandbox != SandboxType::WindowsRestrictedToken
        || windows_sandbox_level == WindowsSandboxLevel::Elevated
    {
        return Ok(None);
    }

    let (mut file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();

    let needs_direct_runtime_enforcement = file_system_sandbox_policy
        .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd);

    if permission_profile_supports_windows_restricted_token_sandbox(permission_profile)
        && !needs_direct_runtime_enforcement
    {
        return Ok(None);
    }

    if !permission_profile_supports_windows_restricted_token_sandbox(permission_profile) {
        let permission_profile_name = permission_profile_display_name(permission_profile);
        return Err(format!(
            "windows sandbox backend cannot enforce file_system={:?}, network={network_sandbox_policy:?}, permission_profile={permission_profile_name}; refusing to run unsandboxed",
            file_system_sandbox_policy.kind,
        ));
    }

    // Windows protects existing metadata paths through the legacy writable root
    // projection. Do not turn skip-missing entries into newly-created
    // deny-write sentinels.
    file_system_sandbox_policy.remove_skip_missing_path_entries();

    // The restricted-token backend can still enforce split write restrictions,
    // but its WRITE_RESTRICTED token does not make capability SID deny-read ACEs
    // participate in read access checks. Read restrictions therefore require the
    // elevated backend, even when the filesystem root remains readable.
    if !windows_policy_has_root_read_access(&file_system_sandbox_policy, sandbox_policy_cwd) {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce split filesystem read restrictions directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let resolved = file_system_sandbox_policy
        .resolve_for_enforcement(sandbox_policy_cwd, FileSystemPolicyProjection::Native)
        .map_err(|err| err.to_string())?;
    let additional_deny_read_paths =
        codex_windows_sandbox::expand_windows_deny_read_paths(&resolved, sandbox_policy_cwd)?;
    if !additional_deny_read_paths.is_empty() {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce deny-read restrictions directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let legacy_writable_roots =
        compatibility_writable_roots_for_permission_profile(permission_profile, sandbox_policy_cwd);
    let split_writable_roots = resolved.writable_roots;
    let legacy_root_paths: BTreeSet<PathBuf> = legacy_writable_roots
        .iter()
        .map(|root| normalize_windows_override_path(root.root.as_path()))
        .collect::<std::result::Result<_, _>>()?;
    let split_root_paths: BTreeSet<PathBuf> = split_writable_roots
        .iter()
        .map(|root| normalize_windows_override_path(root.root.as_path()))
        .collect::<std::result::Result<_, _>>()?;

    if legacy_root_paths != split_root_paths {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce split writable root sets directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    if has_reopened_writable_descendant(&split_writable_roots) {
        return Err(
            "windows unelevated restricted-token sandbox cannot reopen writable descendants under read-only carveouts directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let mut additional_deny_write_paths = BTreeSet::new();
    for split_root in &split_writable_roots {
        let split_root_path = normalize_windows_override_path(split_root.root.as_path())?;
        let Some(legacy_root) = legacy_writable_roots.iter().find(|candidate| {
            normalize_windows_override_path(candidate.root.as_path())
                .is_ok_and(|candidate_path| candidate_path == split_root_path)
        }) else {
            return Err(
                "windows unelevated restricted-token sandbox cannot enforce split writable root sets directly; refusing to run unsandboxed"
                    .to_string(),
            );
        };

        for read_only_subpath in &split_root.read_only_subpaths {
            if !legacy_root
                .read_only_subpaths
                .iter()
                .any(|candidate| candidate == read_only_subpath)
            {
                additional_deny_write_paths.insert(normalize_windows_override_path(
                    read_only_subpath.as_path(),
                )?);
            }
        }
    }

    if additional_deny_read_paths.is_empty() && additional_deny_write_paths.is_empty() {
        return Ok(None);
    }

    Ok(Some(WindowsSandboxFilesystemOverrides {
        read_roots_override: None,
        read_roots_include_platform_defaults: false,
        write_roots_override: None,
        additional_deny_read_paths,
        additional_deny_write_paths: additional_deny_write_paths
            .into_iter()
            .map(|path| AbsolutePathBuf::from_absolute_path(path).map_err(|err| err.to_string()))
            .collect::<std::result::Result<_, _>>()?,
    }))
}

pub fn resolve_windows_elevated_filesystem_overrides(
    sandbox: SandboxType,
    permission_profile: &PermissionProfile,
    sandbox_policy_cwd: &AbsolutePathBuf,
    use_windows_elevated_backend: bool,
    workload_env: &HashMap<String, String>,
) -> std::result::Result<Option<WindowsSandboxFilesystemOverrides>, String> {
    if sandbox != SandboxType::WindowsRestrictedToken || !use_windows_elevated_backend {
        return Ok(None);
    }

    let (mut file_system_sandbox_policy, network_sandbox_policy) =
        permission_profile.to_runtime_permissions();

    if !permission_profile_supports_windows_restricted_token_sandbox(permission_profile) {
        let permission_profile_name = permission_profile_display_name(permission_profile);
        return Err(format!(
            "windows sandbox backend cannot enforce file_system={:?}, network={network_sandbox_policy:?}, permission_profile={permission_profile_name}; refusing to run unsandboxed",
            file_system_sandbox_policy.kind,
        ));
    }

    // Windows protects existing metadata paths through the legacy writable root
    // projection. Do not turn skip-missing entries into newly-created
    // deny-write sentinels.
    file_system_sandbox_policy.remove_skip_missing_path_entries();

    #[cfg(target_os = "windows")]
    codex_windows_sandbox::resolve_workload_temp_paths(
        &mut file_system_sandbox_policy,
        workload_env,
    );
    #[cfg(not(target_os = "windows"))]
    let _ = workload_env;

    let resolved = file_system_sandbox_policy
        .resolve_for_enforcement(sandbox_policy_cwd, FileSystemPolicyProjection::Native)
        .map_err(|err| err.to_string())?;
    let additional_deny_read_paths =
        codex_windows_sandbox::expand_windows_deny_read_paths(&resolved, sandbox_policy_cwd)?;

    let split_writable_roots = resolved.writable_roots;
    if has_reopened_writable_descendant(&split_writable_roots) {
        return Err(
            "windows elevated sandbox cannot reopen writable descendants under read-only carveouts directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let needs_direct_runtime_enforcement = file_system_sandbox_policy
        .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd);
    let has_explicit_write_carveouts = split_writable_roots.iter().any(|writable_root| {
        writable_root.read_only_subpaths.iter().any(|path| {
            file_system_sandbox_policy.has_explicit_non_write_entry_for_local_path_with_cwd(
                path.as_path(),
                sandbox_policy_cwd,
            )
        })
    });
    let normalize_path = |path: PathBuf| dunce::canonicalize(&path).unwrap_or(path);
    let legacy_writable_roots =
        compatibility_writable_roots_for_permission_profile(permission_profile, sandbox_policy_cwd);
    let legacy_root_paths: BTreeSet<PathBuf> = legacy_writable_roots
        .iter()
        .map(|root| normalize_path(root.root.to_path_buf()))
        .collect();
    let split_readable_roots: Vec<PathBuf> = resolved
        .readable_roots
        .into_iter()
        .map(AbsolutePathBuf::into_path_buf)
        .map(&normalize_path)
        .collect();
    let split_root_paths: Vec<PathBuf> = split_writable_roots
        .iter()
        .map(|root| normalize_path(root.root.to_path_buf()))
        .collect();
    let split_root_path_set: BTreeSet<PathBuf> = split_root_paths.iter().cloned().collect();

    // `has_full_disk_read_access()` is intentionally false when deny-read
    // entries exist. For Windows setup overrides, the important question is
    // whether the baseline still reads from the filesystem root and only needs
    // additional deny ACLs layered on top.
    let split_has_root_read_access =
        windows_policy_has_root_read_access(&file_system_sandbox_policy, sandbox_policy_cwd);
    let read_roots_override = if split_has_root_read_access {
        None
    } else {
        Some(split_readable_roots)
    };

    let write_roots_override = if split_root_path_set == legacy_root_paths {
        None
    } else {
        Some(split_root_paths)
    };

    let additional_deny_write_paths = if needs_direct_runtime_enforcement
        || has_explicit_write_carveouts
    {
        let mut deny_paths = BTreeSet::new();
        for writable_root in &split_writable_roots {
            let writable_root_path = normalize_path(writable_root.root.to_path_buf());
            let legacy_root = legacy_writable_roots.iter().find(|candidate| {
                normalize_path(candidate.root.to_path_buf()) == writable_root_path
            });
            for read_only_subpath in &writable_root.read_only_subpaths {
                let read_only_subpath_suffix = read_only_subpath
                    .as_path()
                    .strip_prefix(writable_root.root.as_path())
                    .ok();
                let already_denied_by_legacy = legacy_root.is_some_and(|legacy_root| {
                    legacy_root.read_only_subpaths.iter().any(|candidate| {
                        candidate
                            .as_path()
                            .strip_prefix(legacy_root.root.as_path())
                            .ok()
                            == read_only_subpath_suffix
                    })
                });
                let explicitly_configured = file_system_sandbox_policy
                    .has_explicit_non_write_entry_for_local_path_with_cwd(
                        read_only_subpath.as_path(),
                        sandbox_policy_cwd,
                    );
                if explicitly_configured || !already_denied_by_legacy {
                    deny_paths.insert(normalize_path(read_only_subpath.to_path_buf()));
                }
            }
        }
        deny_paths
            .into_iter()
            .map(|path| AbsolutePathBuf::from_absolute_path(path).map_err(|err| err.to_string()))
            .collect::<std::result::Result<_, _>>()?
    } else {
        Vec::new()
    };

    if read_roots_override.is_none()
        && write_roots_override.is_none()
        && additional_deny_read_paths.is_empty()
        && additional_deny_write_paths.is_empty()
    {
        return Ok(None);
    }

    Ok(Some(WindowsSandboxFilesystemOverrides {
        read_roots_include_platform_defaults: read_roots_override.is_some()
            && resolved.include_platform_defaults,
        read_roots_override,
        write_roots_override,
        additional_deny_read_paths,
        additional_deny_write_paths,
    }))
}

fn normalize_windows_override_path(path: &Path) -> std::result::Result<PathBuf, String> {
    AbsolutePathBuf::from_absolute_path(dunce::simplified(path))
        .map(AbsolutePathBuf::into_path_buf)
        .map_err(|err| err.to_string())
}

fn windows_policy_has_root_read_access(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    cwd: &AbsolutePathBuf,
) -> bool {
    let Some(root) = cwd.as_path().ancestors().last() else {
        return false;
    };
    file_system_sandbox_policy.can_read_local_path_with_cwd(root, cwd.as_path())
}

fn permission_profile_display_name(permission_profile: &PermissionProfile) -> &'static str {
    match permission_profile {
        PermissionProfile::Managed { .. } => "Managed",
        PermissionProfile::Disabled => "Disabled",
        PermissionProfile::External { .. } => "External",
    }
}

fn has_reopened_writable_descendant(writable_roots: &[WritableRoot]) -> bool {
    writable_roots.iter().any(|writable_root| {
        writable_root
            .read_only_subpaths
            .iter()
            .any(|read_only_subpath| {
                writable_roots.iter().any(|candidate| {
                    candidate.root.as_path() != writable_root.root.as_path()
                        && candidate
                            .root
                            .as_path()
                            .starts_with(read_only_subpath.as_path())
                })
            })
    })
}

#[cfg(all(test, target_os = "windows"))]
#[path = "windows_tests.rs"]
mod tests;
