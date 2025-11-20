/*
Module: sandboxing

Build platform wrappers and produce ExecEnv for execution. Owns low‑level
sandbox placement and transformation of portable CommandSpec into a
ready‑to‑spawn environment.
*/

pub mod assessment;

use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::execute_exec_env;
use crate::landlock::create_linux_sandbox_command_args;
use crate::protocol::SandboxPolicy;
use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use crate::seatbelt::create_seatbelt_command_args;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use crate::tools::sandboxing::SandboxablePreference;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;

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
    pub sandbox: SandboxType,
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
    ) -> SandboxType {
        match pref {
            SandboxablePreference::Forbid => SandboxType::None,
            SandboxablePreference::Require => {
                #[cfg(target_os = "macos")]
                {
                    return SandboxType::MacosSeatbelt;
                }
                #[cfg(target_os = "linux")]
                {
                    return SandboxType::LinuxSeccomp;
                }
                #[allow(unreachable_code)]
                SandboxType::None
            }
            SandboxablePreference::Auto => match policy {
                SandboxPolicy::DangerFullAccess => SandboxType::None,
                #[cfg(target_os = "macos")]
                _ => SandboxType::MacosSeatbelt,
                #[cfg(target_os = "linux")]
                _ => SandboxType::LinuxSeccomp,
                #[cfg(not(any(target_os = "macos", target_os = "linux")))]
                _ => SandboxType::None,
            },
        }
    }

    pub(crate) fn transform(
        &self,
        spec: &CommandSpec,
        policy: &SandboxPolicy,
        sandbox: SandboxType,
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

        let (command, sandbox_env, arg0_override) = match sandbox {
            SandboxType::None => (command, HashMap::new(), None),
            SandboxType::MacosSeatbelt => {
                let mut seatbelt_env = HashMap::new();
                seatbelt_env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
                let mut args =
                    create_seatbelt_command_args(command.clone(), policy, sandbox_policy_cwd);
                let mut full_command = Vec::with_capacity(1 + args.len());
                full_command.push(MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string());
                full_command.append(&mut args);
                (full_command, seatbelt_env, None)
            }
            SandboxType::LinuxSeccomp => {
                let exe = codex_linux_sandbox_exe
                    .ok_or(SandboxTransformError::MissingLinuxSandboxExecutable)?;
                let mut args =
                    create_linux_sandbox_command_args(command.clone(), policy, sandbox_policy_cwd);
                let mut full_command = Vec::with_capacity(1 + args.len());
                full_command.push(exe.to_string_lossy().to_string());
                full_command.append(&mut args);
                (
                    full_command,
                    HashMap::new(),
                    Some("codex-linux-sandbox".to_string()),
                )
            }
        };

        env.extend(sandbox_env);

        Ok(ExecEnv {
            command,
            cwd: spec.cwd.clone(),
            env,
            timeout_ms: spec.timeout_ms,
            sandbox,
            with_escalated_permissions: spec.with_escalated_permissions,
            justification: spec.justification.clone(),
            arg0: arg0_override,
        })
    }

    pub fn denied(&self, sandbox: SandboxType, out: &ExecToolCallOutput) -> bool {
        crate::exec::is_likely_sandbox_denied(sandbox, out)
    }
}

pub async fn execute_env(
    env: &ExecEnv,
    policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    execute_exec_env(env.clone(), policy, stdout_stream).await
}
