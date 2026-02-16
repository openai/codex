/*
Module: sandboxing

Build platform wrappers and produce ExecRequest for execution. Owns low-level
sandbox placement and transformation of portable CommandSpec into a
ready‑to‑spawn environment.
*/

use crate::exec::ExecExpiration;
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_env;
use crate::landlock::allow_network_for_proxy;
use crate::landlock::create_linux_sandbox_command_args;
use crate::protocol::SandboxPolicy;
#[cfg(target_os = "macos")]
use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
#[cfg(target_os = "macos")]
use crate::seatbelt::create_seatbelt_command_args;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use crate::tools::sandboxing::SandboxablePreference;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::models::AdditionalPermissions;
pub use codex_protocol::models::SandboxPermissions;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_utils_absolute_path::AbsolutePathBuf;
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
    pub additional_permissions: Option<AdditionalPermissions>,
    pub justification: Option<String>,
}

#[derive(Debug)]
pub struct ExecRequest {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub network_attempt_id: Option<String>,
    pub expiration: ExecExpiration,
    pub sandbox: SandboxType,
    pub windows_sandbox_level: WindowsSandboxLevel,
    pub sandbox_permissions: SandboxPermissions,
    pub sandbox_policy: SandboxPolicy,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

/// Bundled arguments for sandbox transformation.
///
/// This keeps call sites self-documenting when several fields are optional.
pub(crate) struct SandboxTransformRequest<'a> {
    pub spec: CommandSpec,
    pub policy: &'a SandboxPolicy,
    pub sandbox: SandboxType,
    pub enforce_managed_network: bool,
    // TODO(viyatb): Evaluate switching this to Option<Arc<NetworkProxy>>
    // to make shared ownership explicit across runtime/sandbox plumbing.
    pub network: Option<&'a NetworkProxy>,
    pub sandbox_policy_cwd: &'a Path,
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
    #[error("invalid additional permissions path: {0}")]
    InvalidAdditionalPermissionsPath(String),
    #[cfg(not(target_os = "macos"))]
    #[error("seatbelt sandbox is only available on macOS")]
    SeatbeltUnavailable,
}

pub(crate) fn normalize_additional_permissions(
    additional_permissions: AdditionalPermissions,
    command_cwd: &Path,
) -> Result<AdditionalPermissions, String> {
    let fs_read =
        normalize_permission_paths(additional_permissions.fs_read, command_cwd, "fs_read")?;
    let fs_write =
        normalize_permission_paths(additional_permissions.fs_write, command_cwd, "fs_write")?;
    Ok(AdditionalPermissions { fs_read, fs_write })
}

fn normalize_permission_paths(
    paths: Vec<PathBuf>,
    command_cwd: &Path,
    permission_kind: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut out = Vec::with_capacity(paths.len());
    let mut seen = HashSet::new();

    for path in paths {
        if path.as_os_str().is_empty() {
            return Err(format!("{permission_kind} contains an empty path"));
        }

        let resolved = if path.is_absolute() {
            AbsolutePathBuf::from_absolute_path(path.clone()).map_err(|err| {
                format!(
                    "{permission_kind} path `{}` is invalid: {err}",
                    path.display()
                )
            })?
        } else {
            AbsolutePathBuf::resolve_path_against_base(&path, command_cwd).map_err(|err| {
                format!(
                    "{permission_kind} path `{}` cannot be resolved against cwd `{}`: {err}",
                    path.display(),
                    command_cwd.display()
                )
            })?
        };

        let canonicalized = resolved
            .as_path()
            .canonicalize()
            .ok()
            .and_then(|path| AbsolutePathBuf::from_absolute_path(path).ok())
            .unwrap_or(resolved);
        let canonicalized = canonicalized.to_path_buf();
        if seen.insert(canonicalized.clone()) {
            out.push(canonicalized);
        }
    }

    Ok(out)
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
    additional_permissions: &AdditionalPermissions,
) -> Result<(Vec<AbsolutePathBuf>, Vec<AbsolutePathBuf>), SandboxTransformError> {
    let to_abs = |paths: &[PathBuf]| {
        let mut out = Vec::with_capacity(paths.len());
        for path in paths {
            let absolute = AbsolutePathBuf::from_absolute_path(path.clone()).map_err(|err| {
                SandboxTransformError::InvalidAdditionalPermissionsPath(format!(
                    "`{}`: {err}",
                    path.display()
                ))
            })?;
            out.push(absolute);
        }
        Ok(dedup_absolute_paths(out))
    };

    Ok((
        to_abs(&additional_permissions.fs_read)?,
        to_abs(&additional_permissions.fs_write)?,
    ))
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

fn sandbox_policy_with_additional_permissions(
    sandbox_policy: &SandboxPolicy,
    additional_permissions: &AdditionalPermissions,
) -> Result<SandboxPolicy, SandboxTransformError> {
    if additional_permissions.is_empty() {
        return Ok(sandbox_policy.clone());
    }

    let (extra_reads, extra_writes) = additional_permission_roots(additional_permissions)?;

    let policy = match sandbox_policy {
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            sandbox_policy.clone()
        }
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
                network_access: *network_access,
                exclude_tmpdir_env_var: *exclude_tmpdir_env_var,
                exclude_slash_tmp: *exclude_slash_tmp,
            }
        }
        SandboxPolicy::ReadOnly { access } => {
            if extra_writes.is_empty() {
                SandboxPolicy::ReadOnly {
                    access: merge_read_only_access_with_additional_reads(access, extra_reads),
                }
            } else {
                SandboxPolicy::WorkspaceWrite {
                    writable_roots: dedup_absolute_paths(extra_writes),
                    read_only_access: merge_read_only_access_with_additional_reads(
                        access,
                        extra_reads,
                    ),
                    network_access: false,
                    exclude_tmpdir_env_var: false,
                    exclude_slash_tmp: false,
                }
            }
        }
    };

    Ok(policy)
}

#[derive(Default)]
pub struct SandboxManager;

impl SandboxManager {
    pub fn new() -> Self {
        Self
    }

    pub(crate) fn select_initial(
        &self,
        policy: &SandboxPolicy,
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
            SandboxablePreference::Auto => match policy {
                SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
                    if has_managed_network_requirements {
                        crate::safety::get_platform_sandbox(
                            windows_sandbox_level != WindowsSandboxLevel::Disabled,
                        )
                        .unwrap_or(SandboxType::None)
                    } else {
                        SandboxType::None
                    }
                }
                _ => crate::safety::get_platform_sandbox(
                    windows_sandbox_level != WindowsSandboxLevel::Disabled,
                )
                .unwrap_or(SandboxType::None),
            },
        }
    }

    pub(crate) fn transform(
        &self,
        request: SandboxTransformRequest<'_>,
    ) -> Result<ExecRequest, SandboxTransformError> {
        let SandboxTransformRequest {
            mut spec,
            policy,
            sandbox,
            enforce_managed_network,
            network,
            sandbox_policy_cwd,
            codex_linux_sandbox_exe,
            use_linux_sandbox_bwrap,
            windows_sandbox_level,
        } = request;
        let effective_policy =
            if let Some(additional_permissions) = spec.additional_permissions.take() {
                sandbox_policy_with_additional_permissions(policy, &additional_permissions)?
            } else {
                policy.clone()
            };
        let mut env = spec.env;
        if !effective_policy.has_full_network_access() {
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
                let mut args = create_seatbelt_command_args(
                    command.clone(),
                    &effective_policy,
                    sandbox_policy_cwd,
                    enforce_managed_network,
                    network,
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
                let mut args = create_linux_sandbox_command_args(
                    command.clone(),
                    &effective_policy,
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
            network_attempt_id: None,
            expiration: spec.expiration,
            sandbox,
            windows_sandbox_level,
            sandbox_permissions: spec.sandbox_permissions,
            sandbox_policy: effective_policy,
            justification: spec.justification,
            arg0: arg0_override,
        })
    }

    pub fn denied(&self, sandbox: SandboxType, out: &ExecToolCallOutput) -> bool {
        crate::exec::is_likely_sandbox_denied(sandbox, out)
    }
}

pub async fn execute_env(
    env: ExecRequest,
    _policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    let effective_policy = env.sandbox_policy.clone();
    execute_exec_env(env, &effective_policy, stdout_stream).await
}

#[cfg(test)]
mod tests {
    use super::SandboxManager;
    use crate::exec::SandboxType;
    use crate::protocol::SandboxPolicy;
    use crate::tools::sandboxing::SandboxablePreference;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use pretty_assertions::assert_eq;

    #[test]
    fn danger_full_access_defaults_to_no_sandbox_without_network_requirements() {
        let manager = SandboxManager::new();
        let sandbox = manager.select_initial(
            &SandboxPolicy::DangerFullAccess,
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
            &SandboxPolicy::DangerFullAccess,
            SandboxablePreference::Auto,
            WindowsSandboxLevel::Disabled,
            true,
        );
        assert_eq!(sandbox, expected);
    }
}
