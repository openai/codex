/*
Module: sandboxing

Build platform wrappers and produce ExecRequest for execution. Owns low-level
sandbox placement and transformation of portable CommandSpec into a
ready‑to‑spawn environment.
*/

pub(crate) mod macos_permissions;

use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_request;
use crate::landlock::allow_network_for_proxy;
use crate::landlock::create_linux_sandbox_command_args_for_policies;
use crate::protocol::SandboxPolicy;
#[cfg(target_os = "macos")]
use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
#[cfg(target_os = "macos")]
use crate::seatbelt::create_seatbelt_command_args_for_policies_with_extensions;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use crate::tools::sandboxing::SandboxablePreference;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::FileSystemPermissions;
use codex_protocol::models::MacOsSeatbeltProfileExtensions;
use codex_protocol::models::PermissionProfile;
pub use codex_protocol::models::SandboxPermissions;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemPath;
use codex_protocol::permissions::FileSystemSandboxEntry;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::NetworkAccess;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_utils_absolute_path::AbsolutePathBuf;
use dunce::canonicalize;
use macos_permissions::merge_macos_seatbelt_profile_extensions;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;

#[derive(Debug)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub expiration: ExecExpiration,
    pub sandbox_permissions: SandboxPermissions,
    pub additional_permissions: Option<PermissionProfile>,
    pub justification: Option<String>,
}

#[derive(Debug)]
pub struct ExecRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub expiration: ExecExpiration,
    pub sandbox: SandboxType,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub sandbox_permissions: SandboxPermissions,
    pub sandbox_policy: SandboxPolicy,
    pub file_system_sandbox_policy: FileSystemSandboxPolicy,
    pub network_sandbox_policy: NetworkSandboxPolicy,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

/// Bundled arguments for sandbox transformation.
///
/// This keeps call sites self-documenting when several fields are optional.
pub(crate) struct SandboxTransformRequest<'a> {
    pub spec: CommandSpec,
    pub policy: &'a SandboxPolicy,
    pub file_system_policy: &'a FileSystemSandboxPolicy,
    pub network_policy: NetworkSandboxPolicy,
    pub sandbox: SandboxType,
    pub enforce_managed_network: bool,
    // TODO(viyatb): Evaluate switching this to Option<Arc<NetworkProxy>>
    // to make shared ownership explicit across runtime/sandbox plumbing.
    pub network: Option<&'a NetworkProxy>,
    pub sandbox_policy_cwd: &'a Path,
    #[cfg(target_os = "macos")]
    pub macos_seatbelt_profile_extensions: Option<&'a MacOsSeatbeltProfileExtensions>,
    pub codex_linux_sandbox_exe: Option<&'a PathBuf>,
    pub use_linux_sandbox_bwrap: bool,
    pub windows_sandbox_level: WindowsSandboxLevel,
}

pub enum SandboxPreference {
    Auto,
    Require,
    Forbid,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SandboxTransformError {
    #[error("missing codex-linux-sandbox executable path")]
    MissingLinuxSandboxExecutable,
    #[cfg(not(target_os = "macos"))]
    #[error("seatbelt sandbox is only available on macOS")]
    SeatbeltUnavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveSandboxPermissions {
    pub(crate) sandbox_policy: SandboxPolicy,
    pub(crate) macos_seatbelt_profile_extensions: Option<MacOsSeatbeltProfileExtensions>,
}

impl EffectiveSandboxPermissions {
    pub(crate) fn new(
        sandbox_policy: &SandboxPolicy,
        macos_seatbelt_profile_extensions: Option<&MacOsSeatbeltProfileExtensions>,
        additional_permissions: Option<&PermissionProfile>,
    ) -> Self {
        let Some(additional_permissions) = additional_permissions else {
            return Self {
                sandbox_policy: sandbox_policy.clone(),
                macos_seatbelt_profile_extensions: macos_seatbelt_profile_extensions.cloned(),
            };
        };

        Self {
            sandbox_policy: sandbox_policy_with_additional_permissions(
                sandbox_policy,
                additional_permissions,
            ),
            macos_seatbelt_profile_extensions: merge_macos_seatbelt_profile_extensions(
                macos_seatbelt_profile_extensions,
                additional_permissions.macos.as_ref(),
            ),
        }
    }
}

pub(crate) fn normalize_additional_permissions(
    additional_permissions: PermissionProfile,
) -> Result<PermissionProfile, String> {
    Ok(PermissionProfile {
        network: additional_permissions.network,
        file_system: additional_permissions
            .file_system
            .map(|file_system| FileSystemPermissions {
                read: file_system
                    .read
                    .map(|paths| normalize_permission_paths(paths, "file_system.read")),
                write: file_system
                    .write
                    .map(|paths| normalize_permission_paths(paths, "file_system.write")),
            }),
        macos: additional_permissions.macos,
    })
}

fn normalize_permission_paths(
    paths: Vec<AbsolutePathBuf>,
    _permission_kind: &str,
) -> Vec<AbsolutePathBuf> {
    let mut out = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();

    for path in paths {
        let canonicalized = canonicalize(path.as_path())
            .ok()
            .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
            .unwrap_or(path);
        if seen.insert(canonicalized.clone()) {
            out.push(canonicalized);
        }
    }

    out
}

fn dedup_absolute_paths(paths: Vec<AbsolutePathBuf>) -> Vec<AbsolutePathBuf> {
    let mut out = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();
    for path in paths {
        if seen.insert(path.to_path_buf()) {
            out.push(path);
        }
    }
    out
}

fn additional_permission_roots(
    additional_permissions: &PermissionProfile,
) -> (Vec<AbsolutePathBuf>, Vec<AbsolutePathBuf>) {
    (
        dedup_absolute_paths(
            additional_permissions
                .file_system
                .as_ref()
                .and_then(|file_system| file_system.read.clone())
                .unwrap_or_default(),
        ),
        dedup_absolute_paths(
            additional_permissions
                .file_system
                .as_ref()
                .and_then(|file_system| file_system.write.clone())
                .unwrap_or_default(),
        ),
    )
}

fn merge_file_system_policy_with_additional_permissions(
    file_system_policy: &FileSystemSandboxPolicy,
    extra_reads: Vec<AbsolutePathBuf>,
    extra_writes: Vec<AbsolutePathBuf>,
) -> FileSystemSandboxPolicy {
    match file_system_policy.kind {
        FileSystemSandboxKind::Restricted => {
            let mut merged_policy = file_system_policy.clone();
            for path in extra_reads {
                let entry = FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path },
                    access: FileSystemAccessMode::Read,
                };
                if !merged_policy.entries.contains(&entry) {
                    merged_policy.entries.push(entry);
                }
            }
            for path in extra_writes {
                let entry = FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path },
                    access: FileSystemAccessMode::Write,
                };
                if !merged_policy.entries.contains(&entry) {
                    merged_policy.entries.push(entry);
                }
            }
            merged_policy
        }
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
            file_system_policy.clone()
        }
    }
}
fn merge_read_only_access_with_additional_reads(
    read_only_access: &ReadOnlyAccess,
    extra_reads: Vec<AbsolutePathBuf>,
) -> ReadOnlyAccess {
    match read_only_access {
        ReadOnlyAccess::FullAccess => ReadOnlyAccess::FullAccess,
        ReadOnlyAccess::Restricted {
            include_platform_defaults,
            readable_roots,
        } => {
            let mut merged = readable_roots.clone();
            merged.extend(extra_reads);
            ReadOnlyAccess::Restricted {
                include_platform_defaults: *include_platform_defaults,
                readable_roots: dedup_absolute_paths(merged),
            }
        }
    }
}

fn merge_network_access(
    base_network_access: bool,
    additional_permissions: &PermissionProfile,
) -> bool {
    base_network_access
        || additional_permissions
            .network
            .as_ref()
            .and_then(|network| network.enabled)
            .unwrap_or(false)
}

fn sandbox_policy_with_additional_permissions(
    sandbox_policy: &SandboxPolicy,
    additional_permissions: &PermissionProfile,
) -> SandboxPolicy {
    if additional_permissions.is_empty() {
        return sandbox_policy.clone();
    }

    let (extra_reads, extra_writes) = additional_permission_roots(additional_permissions);

    match sandbox_policy {
        SandboxPolicy::DangerFullAccess => SandboxPolicy::DangerFullAccess,
        SandboxPolicy::ExternalSandbox { network_access } => SandboxPolicy::ExternalSandbox {
            network_access: if merge_network_access(
                network_access.is_enabled(),
                additional_permissions,
            ) {
                NetworkAccess::Enabled
            } else {
                NetworkAccess::Restricted
            },
        },
        SandboxPolicy::WorkspaceWrite {
            writable_roots,
            read_only_access,
            network_access,
            exclude_tmpdir_env_var,
            exclude_slash_tmp,
        } => {
            let mut merged_writes = writable_roots.clone();
            merged_writes.extend(extra_writes);
            SandboxPolicy::WorkspaceWrite {
                writable_roots: dedup_absolute_paths(merged_writes),
                read_only_access: merge_read_only_access_with_additional_reads(
                    read_only_access,
                    extra_reads,
                ),
                network_access: merge_network_access(*network_access, additional_permissions),
                exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                exclude_slash_tmp: *exclude_slash_tmp,
            }
        }
        SandboxPolicy::ReadOnly {
            access,
            network_access,
        } => {
            if extra_writes.is_empty() {
                SandboxPolicy::ReadOnly {
                    access: merge_read_only_access_with_additional_reads(access, extra_reads),
                    network_access: merge_network_access(*network_access, additional_permissions),
                }
            } else {
                // todo(dylan) - for now, this grants more access than the request. We should restrict this,
                // but we should add a new SandboxPolicy variant to handle this. While the feature is still
                // UnderDevelopment, it's a useful approximation of the desired behavior.
                SandboxPolicy::WorkspaceWrite {
                    writable_roots: dedup_absolute_paths(extra_writes),
                    read_only_access: merge_read_only_access_with_additional_reads(
                        access,
                        extra_reads,
                    ),
                    network_access: merge_network_access(*network_access, additional_permissions),
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }
            }
        }
    }
}

pub(crate) fn should_require_platform_sandbox(
    file_system_policy: &FileSystemSandboxPolicy,
    network_policy: NetworkSandboxPolicy,
    has_managed_network_requirements: bool,
) -> bool {
    if has_managed_network_requirements {
        return true;
    }

    if !network_policy.is_enabled() {
        return !matches!(
            file_system_policy.kind,
            FileSystemSandboxKind::ExternalSandbox
        );
    }

    match file_system_policy.kind {
        FileSystemSandboxKind::Restricted => !file_system_policy.has_full_disk_write_access(),
        FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => false,
    }
}
#[derive(Default)]
pub struct SandboxManager;

impl SandboxManager {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn select_initial(
        &self,
        file_system_policy: &FileSystemSandboxPolicy,
        network_policy: NetworkSandboxPolicy,
        pref: SandboxablePreference,
        windows_sandbox_level: WindowsSandboxLevel,
        has_managed_network_requirements: bool,
    ) -> SandboxType {
        match pref {
            SandboxablePreference::Forbid => SandboxType::None,
            SandboxablePreference::Require => {
                // Require a platform sandbox when available; on Windows this
                // respects the experimental_windows_sandbox feature.
                crate::safety::get_platform_sandbox(
                    windows_sandbox_level != WindowsSandboxLevel::Disabled,
                )
                .unwrap_or(SandboxType::None)
            }
            SandboxablePreference::Auto => {
                if should_require_platform_sandbox(
                    file_system_policy,
                    network_policy,
                    has_managed_network_requirements,
                ) {
                    crate::safety::get_platform_sandbox(
                        windows_sandbox_level != WindowsSandboxLevel::Disabled,
                    )
                    .unwrap_or(SandboxType::None)
                } else {
                    SandboxType::None
                }
            }
        }
    }

    pub(crate) fn transform(
        &self,
        request: SandboxTransformRequest<'_>,
    ) -> Result<ExecRequest, SandboxTransformError> {
        let SandboxTransformRequest {
            mut spec,
            policy,
            file_system_policy,
            network_policy,
            sandbox,
            enforce_managed_network,
            network,
            sandbox_policy_cwd,
            #[cfg(target_os = "macos")]
            macos_seatbelt_profile_extensions,
            codex_linux_sandbox_exe,
            use_linux_sandbox_bwrap,
            windows_sandbox_level,
        } = request;
        #[cfg(not(target_os = "macos"))]
        let macos_seatbelt_profile_extensions = None;
        let additional_permissions = spec.additional_permissions.take();
        let EffectiveSandboxPermissions {
            sandbox_policy: effective_policy,
            macos_seatbelt_profile_extensions: _effective_macos_seatbelt_profile_extensions,
        } = EffectiveSandboxPermissions::new(
            policy,
            macos_seatbelt_profile_extensions,
            additional_permissions.as_ref(),
        );
        let (effective_file_system_policy, effective_network_policy) =
            if let Some(additional_permissions) = additional_permissions {
                let (extra_reads, extra_writes) =
                    additional_permission_roots(&additional_permissions);
                let file_system_sandbox_policy =
                    if extra_reads.is_empty() && extra_writes.is_empty() {
                        file_system_policy.clone()
                    } else {
                        merge_file_system_policy_with_additional_permissions(
                            file_system_policy,
                            extra_reads,
                            extra_writes,
                        )
                    };
                let network_sandbox_policy =
                    if merge_network_access(network_policy.is_enabled(), &additional_permissions) {
                        NetworkSandboxPolicy::Enabled
                    } else {
                        NetworkSandboxPolicy::Restricted
                    };
                (file_system_sandbox_policy, network_sandbox_policy)
            } else {
                (file_system_policy.clone(), network_policy)
            };
        let mut env = spec.env;
        if !effective_network_policy.is_enabled() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }

        let mut command = Vec::with_capacity(1 + spec.args.len());
        command.push(spec.program);
        command.append(&mut spec.args);

        let (command, sandbox_env, arg0_override) = match sandbox {
            SandboxType::None => (command, HashMap::new(), None),
            #[cfg(target_os = "macos")]
            SandboxType::MacosSeatbelt => {
                let mut seatbelt_env = HashMap::new();
                seatbelt_env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
                let mut args = create_seatbelt_command_args_for_policies_with_extensions(
                    command.clone(),
                    &effective_file_system_policy,
                    effective_network_policy,
                    sandbox_policy_cwd,
                    enforce_managed_network,
                    network,
                    _effective_macos_seatbelt_profile_extensions.as_ref(),
                );
                let mut full_command = Vec::with_capacity(1 + args.len());
                full_command.push(MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string());
                full_command.append(&mut args);
                (full_command, seatbelt_env, None)
            }
            #[cfg(not(target_os = "macos"))]
            SandboxType::MacosSeatbelt => return Err(SandboxTransformError::SeatbeltUnavailable),
            SandboxType::LinuxSeccomp => {
                let exe = codex_linux_sandbox_exe
                    .ok_or(SandboxTransformError::MissingLinuxSandboxExecutable)?;
                let allow_proxy_network = allow_network_for_proxy(enforce_managed_network);
                let mut args = create_linux_sandbox_command_args_for_policies(
                    command.clone(),
                    &effective_policy,
                    &effective_file_system_policy,
                    effective_network_policy,
                    sandbox_policy_cwd,
                    use_linux_sandbox_bwrap,
                    allow_proxy_network,
                );
                let mut full_command = Vec::with_capacity(1 + args.len());
                full_command.push(exe.to_string_lossy().to_string());
                full_command.append(&mut args);
                (
                    full_command,
                    HashMap::new(),
                    Some("codex-linux-sandbox".to_string()),
                )
            }
            // On Windows, the restricted token sandbox executes in-process via the
            // codex-windows-sandbox crate. We leave the command unchanged here and
            // branch during execution based on the sandbox type.
            #[cfg(target_os = "windows")]
            SandboxType::WindowsRestrictedToken => (command, HashMap::new(), None),
            // When building for non-Windows targets, this variant is never constructed.
            #[cfg(not(target_os = "windows"))]
            SandboxType::WindowsRestrictedToken => (command, HashMap::new(), None),
        };

        env.extend(sandbox_env);

        Ok(ExecRequest {
            command,
            cwd: spec.cwd,
            env,
            network: network.cloned(),
            expiration: spec.expiration,
            sandbox,
            windows_sandbox_level,
            sandbox_permissions: spec.sandbox_permissions,
            sandbox_policy: effective_policy,
            file_system_sandbox_policy: effective_file_system_policy,
            network_sandbox_policy: effective_network_policy,
            justification: spec.justification,
            arg0: arg0_override,
        })
    }

    pub fn denied(&self, sandbox: SandboxType, out: &ExecToolCallOutput) -> bool {
        crate::exec::is_likely_sandbox_denied(sandbox, out)
    }
}

pub async fn execute_env(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    let effective_policy = exec_request.sandbox_policy.clone();
    execute_exec_request(exec_request, &effective_policy, stdout_stream, None).await
}

pub async fn execute_exec_request_with_after_spawn(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> crate::error::Result<ExecToolCallOutput> {
    let effective_policy = exec_request.sandbox_policy.clone();
    execute_exec_request(exec_request, &effective_policy, stdout_stream, after_spawn).await
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::EffectiveSandboxPermissions;
    use super::SandboxManager;
    use super::merge_file_system_policy_with_additional_permissions;
    use super::normalize_additional_permissions;
    use super::sandbox_policy_with_additional_permissions;
    use super::should_require_platform_sandbox;
    use crate::exec::SandboxType;
    use crate::protocol::NetworkAccess;
    use crate::protocol::ReadOnlyAccess;
    use crate::protocol::SandboxPolicy;
    use crate::tools::sandboxing::SandboxablePreference;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_protocol::models::FileSystemPermissions;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsAutomationPermission;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsPreferencesPermission;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::MacOsSeatbeltProfileExtensions;
    use codex_protocol::models::NetworkPermissions;
    use codex_protocol::models::PermissionProfile;
    use codex_protocol::permissions::FileSystemAccessMode;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSandboxPolicy;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::permissions::NetworkSandboxPolicy;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use dunce::canonicalize;
    use pretty_assertions::assert_eq;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[test]
    fn danger_full_access_defaults_to_no_sandbox_without_network_requirements() {
        let manager = SandboxManager::new();
        let sandbox = manager.select_initial(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Enabled,
            SandboxablePreference::Auto,
            WindowsSandboxLevel::Disabled,
            false,
        );
        assert_eq!(sandbox, SandboxType::None);
    }

    #[test]
    fn danger_full_access_uses_platform_sandbox_with_network_requirements() {
        let manager = SandboxManager::new();
        let expected = crate::safety::get_platform_sandbox(false).unwrap_or(SandboxType::None);
        let sandbox = manager.select_initial(
            &FileSystemSandboxPolicy::unrestricted(),
            NetworkSandboxPolicy::Enabled,
            SandboxablePreference::Auto,
            WindowsSandboxLevel::Disabled,
            true,
        );
        assert_eq!(sandbox, expected);
    }

    #[test]
    fn restricted_file_system_uses_platform_sandbox_without_managed_network() {
        let manager = SandboxManager::new();
        let expected = crate::safety::get_platform_sandbox(false).unwrap_or(SandboxType::None);
        let sandbox = manager.select_initial(
            &FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
                path: FileSystemPath::Special {
                    value: FileSystemSpecialPath::Root,
                },
                access: FileSystemAccessMode::Read,
            }]),
            NetworkSandboxPolicy::Enabled,
            SandboxablePreference::Auto,
            WindowsSandboxLevel::Disabled,
            false,
        );
        assert_eq!(sandbox, expected);
    }

    #[test]
    fn full_access_restricted_policy_skips_platform_sandbox_when_network_is_enabled() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]);

        assert_eq!(
            should_require_platform_sandbox(&policy, NetworkSandboxPolicy::Enabled, false),
            false
        );
    }

    #[test]
    fn full_access_restricted_policy_still_uses_platform_sandbox_for_restricted_network() {
        let policy = FileSystemSandboxPolicy::restricted(vec![FileSystemSandboxEntry {
            path: FileSystemPath::Special {
                value: FileSystemSpecialPath::Root,
            },
            access: FileSystemAccessMode::Write,
        }]);

        assert_eq!(
            should_require_platform_sandbox(&policy, NetworkSandboxPolicy::Restricted, false),
            true
        );
    }

    #[test]
    fn transform_preserves_unrestricted_file_system_policy_for_restricted_network() {
        let manager = SandboxManager::new();
        let cwd = std::env::current_dir().expect("current dir");
        let exec_request = manager
            .transform(super::SandboxTransformRequest {
                spec: super::CommandSpec {
                    program: "true".to_string(),
                    args: Vec::new(),
                    cwd: cwd.clone(),
                    env: HashMap::new(),
                    expiration: crate::exec::ExecExpiration::DefaultTimeout,
                    sandbox_permissions: super::SandboxPermissions::UseDefault,
                    additional_permissions: None,
                    justification: None,
                },
                policy: &SandboxPolicy::ExternalSandbox {
                    network_access: crate::protocol::NetworkAccess::Restricted,
                },
                file_system_policy: &FileSystemSandboxPolicy::unrestricted(),
                network_policy: NetworkSandboxPolicy::Restricted,
                sandbox: SandboxType::None,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: cwd.as_path(),
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: None,
                codex_linux_sandbox_exe: None,
                use_linux_sandbox_bwrap: false,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
            })
            .expect("transform");

        assert_eq!(
            exec_request.file_system_sandbox_policy,
            FileSystemSandboxPolicy::unrestricted()
        );
        assert_eq!(
            exec_request.network_sandbox_policy,
            NetworkSandboxPolicy::Restricted
        );
    }

    #[test]
    fn normalize_additional_permissions_preserves_network() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let path = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let permissions = normalize_additional_permissions(PermissionProfile {
            network: Some(NetworkPermissions {
                enabled: Some(true),
            }),
            file_system: Some(FileSystemPermissions {
                read: Some(vec![path.clone()]),
                write: Some(vec![path.clone()]),
            }),
            ..Default::default()
        })
        .expect("permissions");

        assert_eq!(
            permissions.network,
            Some(NetworkPermissions {
                enabled: Some(true),
            })
        );
        assert_eq!(
            permissions.file_system,
            Some(FileSystemPermissions {
                read: Some(vec![path.clone()]),
                write: Some(vec![path]),
            })
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn normalize_additional_permissions_preserves_macos_permissions() {
        let permissions = normalize_additional_permissions(PermissionProfile {
            macos: Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string(),
                ]),
                macos_accessibility: true,
                macos_calendar: true,
            }),
            ..Default::default()
        })
        .expect("permissions");

        assert_eq!(
            permissions.macos,
            Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Notes".to_string(),
                ]),
                macos_accessibility: true,
                macos_calendar: true,
            })
        );
    }

    #[test]
    fn read_only_additional_permissions_can_enable_network_without_writes() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let path = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let policy = sandbox_policy_with_additional_permissions(
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![path.clone()],
                },
                network_access: false,
            },
            &PermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![path.clone()]),
                    write: Some(Vec::new()),
                }),
                ..Default::default()
            },
        );

        assert_eq!(
            policy,
            SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![path],
                },
                network_access: true,
            }
        );
    }
    #[cfg(target_os = "macos")]
    #[test]
    fn effective_permissions_merge_macos_extensions_with_additional_permissions() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let path = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let effective_permissions = EffectiveSandboxPermissions::new(
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: true,
                    readable_roots: vec![path.clone()],
                },
                network_access: false,
            },
            Some(&MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadOnly,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Calendar".to_string(),
                ]),
                macos_accessibility: false,
                macos_calendar: false,
            }),
            Some(&PermissionProfile {
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![path]),
                    write: Some(Vec::new()),
                }),
                macos: Some(MacOsSeatbeltProfileExtensions {
                    macos_preferences: MacOsPreferencesPermission::ReadWrite,
                    macos_automation: MacOsAutomationPermission::BundleIds(vec![
                        "com.apple.Notes".to_string(),
                    ]),
                    macos_accessibility: true,
                    macos_calendar: true,
                }),
                ..Default::default()
            }),
        );

        assert_eq!(
            effective_permissions.macos_seatbelt_profile_extensions,
            Some(MacOsSeatbeltProfileExtensions {
                macos_preferences: MacOsPreferencesPermission::ReadWrite,
                macos_automation: MacOsAutomationPermission::BundleIds(vec![
                    "com.apple.Calendar".to_string(),
                    "com.apple.Notes".to_string(),
                ]),
                macos_accessibility: true,
                macos_calendar: true,
            })
        );
    }

    #[test]
    fn external_sandbox_additional_permissions_can_enable_network() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let path = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let policy = sandbox_policy_with_additional_permissions(
            &SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Restricted,
            },
            &PermissionProfile {
                network: Some(NetworkPermissions {
                    enabled: Some(true),
                }),
                file_system: Some(FileSystemPermissions {
                    read: Some(vec![path]),
                    write: Some(Vec::new()),
                }),
                ..Default::default()
            },
        );

        assert_eq!(
            policy,
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            }
        );
    }

    #[test]
    fn transform_additional_permissions_enable_network_for_external_sandbox() {
        let manager = SandboxManager::new();
        let cwd = std::env::current_dir().expect("current dir");
        let temp_dir = TempDir::new().expect("create temp dir");
        let path = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let exec_request = manager
            .transform(super::SandboxTransformRequest {
                spec: super::CommandSpec {
                    program: "true".to_string(),
                    args: Vec::new(),
                    cwd: cwd.clone(),
                    env: HashMap::new(),
                    expiration: crate::exec::ExecExpiration::DefaultTimeout,
                    sandbox_permissions: super::SandboxPermissions::WithAdditionalPermissions,
                    additional_permissions: Some(PermissionProfile {
                        network: Some(NetworkPermissions {
                            enabled: Some(true),
                        }),
                        file_system: Some(FileSystemPermissions {
                            read: Some(vec![path]),
                            write: Some(Vec::new()),
                        }),
                        ..Default::default()
                    }),
                    justification: None,
                },
                policy: &SandboxPolicy::ExternalSandbox {
                    network_access: NetworkAccess::Restricted,
                },
                file_system_policy: &FileSystemSandboxPolicy::unrestricted(),
                network_policy: NetworkSandboxPolicy::Restricted,
                sandbox: SandboxType::None,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: cwd.as_path(),
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: None,
                codex_linux_sandbox_exe: None,
                use_linux_sandbox_bwrap: false,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
            })
            .expect("transform");

        assert_eq!(
            exec_request.sandbox_policy,
            SandboxPolicy::ExternalSandbox {
                network_access: NetworkAccess::Enabled,
            }
        );
        assert_eq!(
            exec_request.network_sandbox_policy,
            NetworkSandboxPolicy::Enabled
        );
    }

    #[test]
    fn transform_additional_permissions_preserves_denied_entries() {
        let manager = SandboxManager::new();
        let cwd = std::env::current_dir().expect("current dir");
        let temp_dir = TempDir::new().expect("create temp dir");
        let workspace_root = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let allowed_path = workspace_root.join("allowed").expect("allowed path");
        let denied_path = workspace_root.join("denied").expect("denied path");
        let exec_request = manager
            .transform(super::SandboxTransformRequest {
                spec: super::CommandSpec {
                    program: "true".to_string(),
                    args: Vec::new(),
                    cwd: cwd.clone(),
                    env: HashMap::new(),
                    expiration: crate::exec::ExecExpiration::DefaultTimeout,
                    sandbox_permissions: super::SandboxPermissions::WithAdditionalPermissions,
                    additional_permissions: Some(PermissionProfile {
                        file_system: Some(FileSystemPermissions {
                            read: None,
                            write: Some(vec![allowed_path.clone()]),
                        }),
                        ..Default::default()
                    }),
                    justification: None,
                },
                policy: &SandboxPolicy::ReadOnly {
                    access: ReadOnlyAccess::FullAccess,
                    network_access: false,
                },
                file_system_policy: &FileSystemSandboxPolicy::restricted(vec![
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Special {
                            value: FileSystemSpecialPath::Root,
                        },
                        access: FileSystemAccessMode::Read,
                    },
                    FileSystemSandboxEntry {
                        path: FileSystemPath::Path {
                            path: denied_path.clone(),
                        },
                        access: FileSystemAccessMode::None,
                    },
                ]),
                network_policy: NetworkSandboxPolicy::Restricted,
                sandbox: SandboxType::None,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: cwd.as_path(),
                #[cfg(target_os = "macos")]
                macos_seatbelt_profile_extensions: None,
                codex_linux_sandbox_exe: None,
                use_linux_sandbox_bwrap: false,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
            })
            .expect("transform");

        assert_eq!(
            exec_request.file_system_sandbox_policy,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: denied_path },
                    access: FileSystemAccessMode::None,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: allowed_path },
                    access: FileSystemAccessMode::Write,
                },
            ])
        );
        assert_eq!(
            exec_request.network_sandbox_policy,
            NetworkSandboxPolicy::Restricted
        );
    }

    #[test]
    fn merge_file_system_policy_with_additional_permissions_preserves_unreadable_roots() {
        let temp_dir = TempDir::new().expect("create temp dir");
        let cwd = AbsolutePathBuf::from_absolute_path(
            canonicalize(temp_dir.path()).expect("canonicalize temp dir"),
        )
        .expect("absolute temp dir");
        let allowed_path = cwd.join("allowed").expect("allowed path");
        let denied_path = cwd.join("denied").expect("denied path");
        let merged_policy = merge_file_system_policy_with_additional_permissions(
            &FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::Root,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: denied_path.clone(),
                    },
                    access: FileSystemAccessMode::None,
                },
            ]),
            vec![allowed_path.clone()],
            Vec::new(),
        );

        assert_eq!(
            merged_policy.entries.contains(&FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: denied_path },
                access: FileSystemAccessMode::None,
            }),
            true
        );
        assert_eq!(
            merged_policy.entries.contains(&FileSystemSandboxEntry {
                path: FileSystemPath::Path { path: allowed_path },
                access: FileSystemAccessMode::Read,
            }),
            true
        );
    }
}
