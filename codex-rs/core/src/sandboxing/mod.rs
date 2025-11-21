//! # Sandboxing
//!
//! This module provides platform wrappers and constructs `ExecEnv` objects for
//! command execution. It owns low-level sandbox placement logic and transforms
//! portable `CommandSpec` structs into ready-to-spawn execution environments.

pub mod assessment;
pub mod linux;
pub mod mac;

use crate::exec::ExecToolCallOutput;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_env;
use crate::protocol::SandboxPolicy;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use crate::tools::sandboxing::SandboxablePreference;

use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
#[cfg(target_os = "macos")]
use mac::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
#[cfg(target_os = "macos")]
use mac::seatbelt::create_seatbelt_command_args;

#[cfg(target_os = "linux")]
use linux::landlock::create_linux_sandbox_command_args;

type TransformResult =
    Result<(Vec<String>, HashMap<String, String>, Option<String>), SandboxTransformError>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SandboxPermissions {
    UseDefault,
    RequireEscalated,
}

impl SandboxPermissions {
    pub fn requires_escalated_permissions(self) -> bool {
        matches!(self, SandboxPermissions::RequireEscalated)
    }
}

impl From<bool> for SandboxPermissions {
    fn from(with_escalated_permissions: bool) -> Self {
        if with_escalated_permissions {
            SandboxPermissions::RequireEscalated
        } else {
            SandboxPermissions::UseDefault
        }
    }
}

#[derive(Clone, Debug)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ExecEnv {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub sandboxed: bool,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

pub enum SandboxPreference {
    Auto,
    Require,
    Forbid,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SandboxTransformError {
    #[cfg(target_os = "linux")]
    #[error("missing codex-linux-sandbox executable path")]
    MissingLinuxSandboxExecutable,
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
    ) -> bool {
        match pref {
            SandboxablePreference::Forbid => false,
            SandboxablePreference::Require => {
                // Require a platform sandbox when available; on Windows this
                // respects the enable_experimental_windows_sandbox feature.
                crate::safety::get_platform_has_sandbox()
            }
            SandboxablePreference::Auto => match policy {
                SandboxPolicy::DangerFullAccess => false,
                _ => crate::safety::get_platform_has_sandbox(),
            },
        }
    }

    pub(crate) fn transform(
        &self,
        spec: &CommandSpec,
        policy: &SandboxPolicy,
        sandboxed: bool,
        sandbox_policy_cwd: &Path,
        codex_linux_sandbox_exe: Option<&PathBuf>,
    ) -> Result<ExecEnv, SandboxTransformError> {
        let mut env = spec.env.clone();
        if !policy.has_full_network_access() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }

        let mut command = Vec::with_capacity(1 + spec.args.len());
        command.push(spec.program.clone());
        command.extend(spec.args.iter().cloned());

        if !sandboxed {
            return Ok(ExecEnv {
                command,
                cwd: spec.cwd.clone(),
                env,
                timeout_ms: spec.timeout_ms,
                sandboxed,
                with_escalated_permissions: spec.with_escalated_permissions,
                justification: spec.justification.clone(),
                arg0: None,
            });
        }

        let (command, sandbox_env, arg0_override) =
            self.transform_platform(command, policy, sandbox_policy_cwd, codex_linux_sandbox_exe)?;

        env.extend(sandbox_env);

        Ok(ExecEnv {
            command,
            cwd: spec.cwd.clone(),
            env,
            timeout_ms: spec.timeout_ms,
            sandboxed,
            with_escalated_permissions: spec.with_escalated_permissions,
            justification: spec.justification.clone(),
            arg0: arg0_override,
        })
    }
}

impl SandboxManager {
    #[cfg(target_os = "macos")]
    fn transform_platform(
        &self,
        command: Vec<String>,
        policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        _codex_linux_sandbox_exe: Option<&PathBuf>,
    ) -> TransformResult {
        let mut seatbelt_env = HashMap::new();
        seatbelt_env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
        let mut args = create_seatbelt_command_args(command, policy, sandbox_policy_cwd);
        let mut full_command = Vec::with_capacity(1 + args.len());
        full_command.push(MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string());
        full_command.append(&mut args);
        Ok((full_command, seatbelt_env, None))
    }

    #[cfg(target_os = "linux")]
    fn transform_platform(
        &self,
        command: Vec<String>,
        policy: &SandboxPolicy,
        sandbox_policy_cwd: &Path,
        codex_linux_sandbox_exe: Option<&PathBuf>,
    ) -> TransformResult {
        let exe =
            codex_linux_sandbox_exe.ok_or(SandboxTransformError::MissingLinuxSandboxExecutable)?;
        let mut args =
            create_linux_sandbox_command_args(command.clone(), policy, sandbox_policy_cwd);
        let mut full_command = Vec::with_capacity(1 + args.len());
        full_command.push(exe.to_string_lossy().to_string());
        full_command.append(&mut args);
        Ok((
            full_command,
            HashMap::new(),
            Some("codex-linux-sandbox".to_string()),
        ))
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    // On Windows, the restricted token sandbox executes in-process via the
    // codex-windows-sandbox crate. We leave the command unchanged and branch
    // during execution based on the sandbox type.
    fn transform_platform(
        &self,
        command: Vec<String>,
        _policy: &SandboxPolicy,
        _sandbox_policy_cwd: &Path,
        _codex_linux_sandbox_exe: Option<&PathBuf>,
    ) -> TransformResult {
        Ok((command, HashMap::new(), None))
    }
}

pub async fn execute_env(
    env: &ExecEnv,
    policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    execute_exec_env(env.clone(), policy, stdout_stream).await
}
