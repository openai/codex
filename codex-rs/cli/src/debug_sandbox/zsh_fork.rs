use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;

use anyhow::Context as _;
use codex_core::config::Config;
use codex_core::config::NetworkProxyAuditMetadata;
use codex_core::seatbelt::create_seatbelt_command_args_for_policies_with_extensions;
use codex_core::spawn::CODEX_SANDBOX_ENV_VAR;
use codex_core::spawn::CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR;
use codex_network_proxy::NetworkProxy;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_shell_escalation::EscalateServer;
use codex_shell_escalation::EscalationDecision;
use codex_shell_escalation::EscalationExecution;
use codex_shell_escalation::EscalationPermissions;
use codex_shell_escalation::EscalationPolicy;
use codex_shell_escalation::ExecParams;
use codex_shell_escalation::ExecResult;
use codex_shell_escalation::PreparedExec;
use codex_shell_escalation::ShellCommandExecutor;
use codex_utils_absolute_path::AbsolutePathBuf;
use tokio::process::Child;
use tokio::process::Command;
use tokio_util::sync::CancellationToken;

use super::seatbelt::DenialLogger;

pub(crate) async fn run_command_under_zsh_fork(
    command: Vec<String>,
    config: Config,
    cwd: PathBuf,
    env: HashMap<String, String>,
    log_denials: bool,
) -> anyhow::Result<()> {
    let parsed = ParsedShellCommand::extract(&command)?;
    let shell_command = config
        .zsh_path
        .as_ref()
        .map(|zsh_path| {
            vec![
                zsh_path.to_string_lossy().to_string(),
                parsed.flag().to_string(),
                parsed.script.clone(),
            ]
        })
        .unwrap_or(command);

    let main_execve_wrapper_exe = config
        .main_execve_wrapper_exe
        .clone()
        .context("`codex sandbox macos --zsh-fork` requires main_execve_wrapper_exe")?;

    let managed_network_requirements_enabled = config.managed_network_requirements_enabled();
    let network_proxy = match config.permissions.network.as_ref() {
        Some(spec) => Some(
            spec.start_proxy(
                config.permissions.sandbox_policy.get(),
                /*policy_decider*/ None,
                /*blocked_request_observer*/ None,
                managed_network_requirements_enabled,
                NetworkProxyAuditMetadata::default(),
            )
            .await
            .map_err(|err| anyhow::anyhow!("failed to start managed network proxy: {err}"))?,
        ),
        None => None,
    };
    let network = network_proxy
        .as_ref()
        .map(codex_core::config::StartedNetworkProxy::proxy);

    let denial_logger = Arc::new(Mutex::new(log_denials.then(DenialLogger::new).flatten()));
    let executor = DebugShellCommandExecutor {
        command: shell_command,
        cwd: cwd.clone(),
        env,
        file_system_sandbox_policy: config.permissions.file_system_sandbox_policy.clone(),
        network_sandbox_policy: config.permissions.network_sandbox_policy,
        network,
        macos_seatbelt_profile_extensions: config
            .permissions
            .macos_seatbelt_profile_extensions
            .clone(),
        sandbox_policy_cwd: cwd,
        denial_logger: Arc::clone(&denial_logger),
    };
    let zsh_path = config.zsh_path.clone().unwrap_or_else(|| {
        PathBuf::from(
            executor
                .command
                .first()
                .cloned()
                .unwrap_or_else(|| "/bin/zsh".to_string()),
        )
    });
    let escalation_server = EscalateServer::new(
        zsh_path,
        main_execve_wrapper_exe,
        TurnDefaultEscalationPolicy,
    );
    let exec_result = escalation_server
        .exec(
            ExecParams {
                command: parsed.script,
                workdir: executor.cwd.to_string_lossy().to_string(),
                timeout_ms: None,
                login: Some(parsed.login),
            },
            CancellationToken::new(),
            Arc::new(executor),
        )
        .await?;

    print_exec_result(&exec_result);

    let denial_logger = denial_logger
        .lock()
        .ok()
        .and_then(|mut logger| logger.take());
    if let Some(denial_logger) = denial_logger {
        let denials = denial_logger.finish().await;
        eprintln!("\n=== Sandbox denials ===");
        if denials.is_empty() {
            eprintln!("None found.");
        } else {
            for denial in denials {
                eprintln!("({}) {}", denial.name, denial.capability);
            }
        }
    }

    std::process::exit(exec_result.exit_code);
}

struct ParsedShellCommand {
    script: String,
    login: bool,
}

impl ParsedShellCommand {
    fn extract(command: &[String]) -> anyhow::Result<Self> {
        if let Some((login, script)) = command.windows(3).find_map(|parts| match parts {
            [_, flag, script] if flag == "-c" => Some((false, script.clone())),
            [_, flag, script] if flag == "-lc" => Some((true, script.clone())),
            _ => None,
        }) {
            return Ok(Self { script, login });
        }

        anyhow::bail!(
            "`codex sandbox macos --zsh-fork` expects a `zsh -c ...` or `zsh -lc ...` command"
        )
    }

    fn flag(&self) -> &'static str {
        if self.login { "-lc" } else { "-c" }
    }
}

struct TurnDefaultEscalationPolicy;

#[async_trait::async_trait]
impl EscalationPolicy for TurnDefaultEscalationPolicy {
    async fn determine_action(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
    ) -> anyhow::Result<EscalationDecision> {
        tracing::debug!("zsh-fork debug escalation for {program:?} {argv:?} in {workdir:?}");
        Ok(EscalationDecision::escalate(
            EscalationExecution::TurnDefault,
        ))
    }
}

struct DebugShellCommandExecutor {
    command: Vec<String>,
    cwd: PathBuf,
    env: HashMap<String, String>,
    file_system_sandbox_policy: FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    network: Option<NetworkProxy>,
    macos_seatbelt_profile_extensions:
        Option<codex_protocol::models::MacOsSeatbeltProfileExtensions>,
    sandbox_policy_cwd: PathBuf,
    denial_logger: Arc<Mutex<Option<DenialLogger>>>,
}

#[async_trait::async_trait]
impl ShellCommandExecutor for DebugShellCommandExecutor {
    async fn run(
        &self,
        _command: Vec<String>,
        _cwd: PathBuf,
        env_overlay: HashMap<String, String>,
        _cancel_rx: CancellationToken,
        after_spawn: Option<Box<dyn FnOnce() + Send>>,
    ) -> anyhow::Result<ExecResult> {
        let prepared = self.prepare_seatbelt_exec(
            self.command.clone(),
            self.cwd.clone(),
            overlay_env(&self.env, env_overlay),
            &self.file_system_sandbox_policy,
            self.network_sandbox_policy,
            self.macos_seatbelt_profile_extensions.as_ref(),
        );
        let child = spawn_prepared_command(&prepared)?;
        if let Ok(mut logger) = self.denial_logger.lock()
            && let Some(denial_logger) = logger.as_mut()
        {
            denial_logger.on_child_spawn(&child);
        }
        if let Some(after_spawn) = after_spawn {
            after_spawn();
        }
        wait_for_output(child).await
    }

    async fn prepare_escalated_exec(
        &self,
        program: &AbsolutePathBuf,
        argv: &[String],
        workdir: &AbsolutePathBuf,
        env: HashMap<String, String>,
        execution: EscalationExecution,
    ) -> anyhow::Result<PreparedExec> {
        let command = join_program_and_argv(program, argv);
        let Some(first_arg) = argv.first() else {
            anyhow::bail!("intercepted exec request must contain argv[0]");
        };

        match execution {
            EscalationExecution::TurnDefault => Ok(self.prepare_seatbelt_exec(
                command,
                workdir.to_path_buf(),
                env,
                &self.file_system_sandbox_policy,
                self.network_sandbox_policy,
                self.macos_seatbelt_profile_extensions.as_ref(),
            )),
            EscalationExecution::Unsandboxed => Ok(PreparedExec {
                command,
                cwd: workdir.to_path_buf(),
                env,
                arg0: Some(first_arg.clone()),
            }),
            EscalationExecution::Permissions(EscalationPermissions::Permissions(permissions)) => {
                Ok(self.prepare_seatbelt_exec(
                    command,
                    workdir.to_path_buf(),
                    env,
                    &permissions.file_system_sandbox_policy,
                    permissions.network_sandbox_policy,
                    permissions.macos_seatbelt_profile_extensions.as_ref(),
                ))
            }
            EscalationExecution::Permissions(EscalationPermissions::PermissionProfile(_)) => {
                anyhow::bail!(
                    "`codex sandbox macos --zsh-fork` does not yet support permission-profile escalations"
                )
            }
        }
    }
}

impl DebugShellCommandExecutor {
    fn prepare_seatbelt_exec(
        &self,
        command: Vec<String>,
        cwd: PathBuf,
        mut env: HashMap<String, String>,
        file_system_sandbox_policy: &FileSystemSandboxPolicy,
        network_sandbox_policy: NetworkSandboxPolicy,
        macos_seatbelt_profile_extensions: Option<
            &codex_protocol::models::MacOsSeatbeltProfileExtensions,
        >,
    ) -> PreparedExec {
        let args = create_seatbelt_command_args_for_policies_with_extensions(
            command,
            file_system_sandbox_policy,
            network_sandbox_policy,
            self.sandbox_policy_cwd.as_path(),
            false,
            self.network.as_ref(),
            macos_seatbelt_profile_extensions,
        );

        env.insert(CODEX_SANDBOX_ENV_VAR.to_string(), "seatbelt".to_string());
        if let Some(network) = self.network.as_ref() {
            network.apply_to_env(&mut env);
        }
        if !network_sandbox_policy.is_enabled() {
            env.insert(
                CODEX_SANDBOX_NETWORK_DISABLED_ENV_VAR.to_string(),
                "1".to_string(),
            );
        }

        PreparedExec {
            command: std::iter::once("/usr/bin/sandbox-exec".to_string())
                .chain(args)
                .collect(),
            cwd,
            env,
            arg0: None,
        }
    }
}

fn overlay_env(
    base_env: &HashMap<String, String>,
    env_overlay: HashMap<String, String>,
) -> HashMap<String, String> {
    let mut env = base_env.clone();
    for (key, value) in env_overlay {
        env.insert(key, value);
    }
    env
}

fn spawn_prepared_command(prepared: &PreparedExec) -> std::io::Result<Child> {
    let (program, args) = prepared.command.split_first().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "prepared command must not be empty",
        )
    })?;

    let mut command = Command::new(program);
    command
        .args(args)
        .arg0(prepared.arg0.clone().unwrap_or_else(|| program.to_string()))
        .env_clear()
        .envs(prepared.env.clone())
        .current_dir(&prepared.cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()
}

async fn wait_for_output(child: Child) -> anyhow::Result<ExecResult> {
    let output = child.wait_with_output().await?;
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    let exit_code = output.status.code().unwrap_or(1);

    Ok(ExecResult {
        exit_code,
        output: format!("{stdout}{stderr}"),
        stdout,
        stderr,
        duration: Default::default(),
        timed_out: false,
    })
}

fn print_exec_result(exec_result: &ExecResult) {
    if !exec_result.stdout.is_empty() {
        print!("{}", exec_result.stdout);
    }
    if !exec_result.stderr.is_empty() {
        eprint!("{}", exec_result.stderr);
    }
}

fn join_program_and_argv(program: &AbsolutePathBuf, argv: &[String]) -> Vec<String> {
    std::iter::once(program.to_string_lossy().to_string())
        .chain(argv.iter().skip(1).cloned())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::ParsedShellCommand;
    use pretty_assertions::assert_eq;

    #[test]
    fn extract_accepts_non_login_zsh_command() {
        let parsed = ParsedShellCommand::extract(&[
            "/bin/zsh".to_string(),
            "-c".to_string(),
            "echo hi".to_string(),
        ])
        .expect("parse zsh command");

        assert_eq!(parsed.script, "echo hi");
        assert!(!parsed.login);
    }

    #[test]
    fn extract_accepts_login_zsh_command() {
        let parsed = ParsedShellCommand::extract(&[
            "/bin/zsh".to_string(),
            "-lc".to_string(),
            "echo hi".to_string(),
        ])
        .expect("parse zsh command");

        assert_eq!(parsed.script, "echo hi");
        assert!(parsed.login);
    }
}
