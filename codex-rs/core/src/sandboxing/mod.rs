/*
Module: sandboxing

Build platform wrappers and produce ExecRequest for execution. Owns low-level
sandbox placement and transformation of portable CommandSpec into a
ready‑to‑spawn environment.
*/
mod policy_merge;
pub(crate) use policy_merge::extend_sandbox_policy;

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
use crate::seatbelt::create_seatbelt_command_args_with_extensions;
#[cfg(target_os = "macos")]
pub(crate) use crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions;
#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use crate::tools::sandboxing::SandboxablePreference;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
pub use codex_protocol::models::SandboxPermissions;
#[cfg(not(target_os = "macos"))]
pub(crate) type MacOsSeatbeltProfileExtensions = ();
use std::collections::HashMap;
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
    pub macos_seatbelt_profile_extensions: Option<&'a MacOsSeatbeltProfileExtensions>,
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
            macos_seatbelt_profile_extensions,
        } = request;
        let mut env = spec.env;
        if !policy.has_full_network_access() {
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
                let zsh_exec_bridge_wrapper_socket = env
                    .get(crate::zsh_exec_bridge::ZSH_EXEC_BRIDGE_WRAPPER_SOCKET_ENV_VAR)
                    .map(PathBuf::from);
                let zsh_exec_bridge_allowed_unix_sockets = zsh_exec_bridge_wrapper_socket
                    .as_ref()
                    .map_or_else(Vec::new, |path| vec![path.clone()]);
                let mut args = create_seatbelt_command_args_with_extensions(
                    command.clone(),
                    policy,
                    sandbox_policy_cwd,
                    enforce_managed_network,
                    network,
                    macos_seatbelt_profile_extensions,
                    &zsh_exec_bridge_allowed_unix_sockets,
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
                    policy,
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
    policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    execute_exec_env(env, policy, stdout_stream).await
}

#[cfg(test)]
mod tests {
    #[cfg(target_os = "macos")]
    use super::CommandSpec;
    use super::SandboxManager;
    #[cfg(target_os = "macos")]
    use super::SandboxTransformRequest;
    #[cfg(target_os = "macos")]
    use crate::exec::ExecExpiration;
    use crate::exec::SandboxType;
    use crate::protocol::SandboxPolicy;
    use crate::tools::sandboxing::SandboxablePreference;
    use codex_protocol::config_types::WindowsSandboxLevel;
    #[cfg(target_os = "macos")]
    use codex_protocol::models::SandboxPermissions;
    use pretty_assertions::assert_eq;
    #[cfg(target_os = "macos")]
    use std::collections::HashMap;

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

    #[cfg(target_os = "macos")]
    #[test]
    fn transform_applies_macos_seatbelt_profile_extensions_when_present() {
        use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
        use crate::seatbelt_permissions::MacOsAutomationPermission;
        use crate::seatbelt_permissions::MacOsPreferencesPermission;
        use crate::seatbelt_permissions::MacOsSeatbeltProfileExtensions;

        let manager = SandboxManager::new();
        let tempdir = tempfile::tempdir().expect("tempdir");
        let cwd = tempdir.path().to_path_buf();
        let spec = CommandSpec {
            program: "/bin/echo".to_string(),
            args: vec!["ok".to_string()],
            cwd: cwd.clone(),
            env: HashMap::new(),
            expiration: ExecExpiration::DefaultTimeout,
            sandbox_permissions: SandboxPermissions::UseDefault,
            justification: None,
        };
        let extensions = MacOsSeatbeltProfileExtensions {
            macos_preferences: MacOsPreferencesPermission::ReadWrite,
            macos_automation: MacOsAutomationPermission::BundleIds(vec![
                "com.apple.Notes".to_string(),
            ]),
            macos_accessibility: true,
            macos_calendar: false,
        };

        let transformed = manager
            .transform(SandboxTransformRequest {
                spec,
                policy: &SandboxPolicy::new_read_only_policy(),
                sandbox: SandboxType::MacosSeatbelt,
                enforce_managed_network: false,
                network: None,
                sandbox_policy_cwd: &cwd,
                codex_linux_sandbox_exe: None,
                use_linux_sandbox_bwrap: false,
                windows_sandbox_level: WindowsSandboxLevel::Disabled,
                macos_seatbelt_profile_extensions: Some(&extensions),
            })
            .expect("transform");

        assert_eq!(
            transformed.command.first(),
            Some(&MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string())
        );
        let policy_arg_idx = transformed
            .command
            .iter()
            .position(|arg| arg == "-p")
            .expect("contains -p policy");
        let policy = transformed
            .command
            .get(policy_arg_idx + 1)
            .expect("policy after -p");
        assert!(policy.contains("(allow user-preference-write)"));
        assert!(policy.contains("com.apple.Notes"));
        assert!(policy.contains("com.apple.axserver"));
    }
}
