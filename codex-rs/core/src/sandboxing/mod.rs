/*
Module: sandboxing

Build platform wrappers and produce ExecEnv for execution. Owns low‑level
sandbox placement and transformation of portable CommandSpec into a
ready‑to‑spawn environment.
*/
use crate::exec::ExecToolCallOutput;
use crate::exec::SandboxType;
use crate::exec::StdoutStream;
use crate::exec::execute_sandbox_launch;
use crate::landlock::create_linux_sandbox_command_args;
use crate::protocol::SandboxPolicy;
use crate::seatbelt::MACOS_PATH_TO_SEATBELT_EXECUTABLE;
use crate::seatbelt::create_seatbelt_command_args;
use crate::spawn::CODEX_SANDBOX_ENV_VAR;
use crate::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
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
    pub program: String,
    pub args: Vec<String>,
    pub cwd: PathBuf,
    pub env: HashMap<String, String>,
    pub timeout_ms: Option<u64>,
    pub sandbox: SandboxType,
}

pub enum SandboxPreference {
    Auto,
    Require,
    Forbid,
}

#[derive(Debug)]
pub(crate) struct SandboxLaunch {
    pub sandbox_type: SandboxType,
    pub program: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum SandboxLaunchError {
    #[error("missing command line for sandbox launch")]
    MissingCommandLine,
    #[error("missing codex-linux-sandbox executable path")]
    MissingLinuxSandboxExecutable,
}

pub(crate) fn build_launch_for_sandbox(
    sandbox: SandboxType,
    command: &[String],
    sandbox_policy: &SandboxPolicy,
    sandbox_policy_cwd: &Path,
    codex_linux_sandbox_exe: Option<&PathBuf>,
) -> Result<SandboxLaunch, SandboxLaunchError> {
    let mut env = HashMap::new();
    if !sandbox_policy.has_full_network_access() {
        env.insert(
            CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
            "1".to_string(),
        );
    }

    match sandbox {
        SandboxType::None => {
            let (program, args) = command
                .split_first()
                .ok_or(SandboxLaunchError::MissingCommandLine)?;
            Ok(SandboxLaunch {
                sandbox_type: SandboxType::None,
                program: program.clone(),
                args: args.to_vec(),
                env,
            })
        }
        SandboxType::MacosSeatbelt => {
            env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
            let args =
                create_seatbelt_command_args(command.to_vec(), sandbox_policy, sandbox_policy_cwd);
            Ok(SandboxLaunch {
                sandbox_type: SandboxType::MacosSeatbelt,
                program: MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string(),
                args,
                env,
            })
        }
        SandboxType::LinuxSeccomp => {
            let exe =
                codex_linux_sandbox_exe.ok_or(SandboxLaunchError::MissingLinuxSandboxExecutable)?;
            let args = create_linux_sandbox_command_args(
                command.to_vec(),
                sandbox_policy,
                sandbox_policy_cwd,
            );
            Ok(SandboxLaunch {
                sandbox_type: SandboxType::LinuxSeccomp,
                program: exe.to_string_lossy().to_string(),
                args,
                env,
            })
        }
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
        policy: &SandboxPolicy,
        pref: super::orchestrator::SandboxablePreference,
    ) -> SandboxType {
        match pref {
            super::orchestrator::SandboxablePreference::Forbid => SandboxType::None,
            super::orchestrator::SandboxablePreference::Require => {
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
            super::orchestrator::SandboxablePreference::Auto => match policy {
                SandboxPolicy::DangerFullAccess => SandboxType::None,
                #[cfg(target_os = "macos")]
                _ => SandboxType::MacosSeatbelt,
                #[cfg(target_os = "linux")]
                _ => SandboxType::LinuxSeccomp,
            },
        }
    }

    pub fn transform(
        &self,
        spec: &CommandSpec,
        policy: &SandboxPolicy,
        sandbox: SandboxType,
        codex_linux_sandbox_exe: Option<&PathBuf>,
    ) -> ExecEnv {
        // Internally reuse existing builder but expose only a ready env.
        let launch = build_launch_for_sandbox(
            sandbox,
            &vec_iter_to_vec([&spec.program].into_iter().chain(spec.args.iter())),
            policy,
            &spec.cwd,
            codex_linux_sandbox_exe,
        )
        .expect("sandbox launch should be buildable with provided spec");

        let mut env = spec.env.clone();
        env.extend(launch.env);

        ExecEnv {
            program: launch.program,
            args: launch.args,
            cwd: spec.cwd.clone(),
            env,
            timeout_ms: spec.timeout_ms,
            sandbox,
        }
    }

    pub fn denied(&self, sandbox: SandboxType, out: &ExecToolCallOutput) -> bool {
        crate::exec::is_likely_sandbox_denied(sandbox, out)
    }
}

fn vec_iter_to_vec<'a, I>(iter: I) -> Vec<String>
where
    I: Iterator<Item = &'a String>,
{
    iter.cloned().collect()
}

pub async fn execute_env(
    env: &ExecEnv,
    policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> crate::error::Result<ExecToolCallOutput> {
    // Build sandbox launch again (wrapping program/args as needed) and delegate to existing runner.
    let launch = build_launch_for_sandbox(
        env.sandbox,
        &vec_iter_to_vec([&env.program].into_iter().chain(env.args.iter())),
        policy,
        &env.cwd,
        None,
    )
    .expect("sandbox launch should be buildable");

    let params = crate::exec::ExecParams {
        command: vec![env.program.clone()],
        cwd: env.cwd.clone(),
        timeout_ms: env.timeout_ms,
        env: env.env.clone(),
        with_escalated_permissions: None,
        justification: None,
    };

    execute_sandbox_launch(params, launch, env.sandbox, policy, stdout_stream).await
}
