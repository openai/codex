//! Dispatches admitted hooks on the ready environments captured for a lifecycle event.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use anyhow::bail;
use codex_exec_server::ExecOutputStream;
use codex_exec_server::ExecParams;
use codex_exec_server::ExecProcess;
use codex_exec_server::ProcessId;
use codex_hooks::EnvironmentHookOutput;
use codex_hooks::EnvironmentHookSource;
use codex_hooks::HookEnvironmentExecutor;
use codex_hooks::Hooks;
use codex_network_proxy::NetworkProxy;
use codex_protocol::models::PermissionProfile;
use futures::FutureExt;
use futures::future::BoxFuture;

use crate::environment_selection::TurnEnvironmentSnapshot;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use crate::session::turn_context::TurnEnvironment;
use crate::shell::ShellType;
use crate::tools::network_approval::execution_network_proxy;
use crate::tools::network_approval::remote_network_proxy_launch;
use crate::tools::sandboxing::ToolError;
use crate::unified_exec::exec_env_policy_from_shell_policy;

const MAX_HOOK_OUTPUT_BYTES: usize = 1024 * 1024;

pub(crate) fn with_environment_hooks(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    environments: &TurnEnvironmentSnapshot,
) -> Hooks {
    with_tool_environment_hooks(
        session,
        turn,
        environments,
        /*target_environments*/ &[],
    )
}

pub(crate) fn with_tool_environment_hooks(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    environments: &TurnEnvironmentSnapshot,
    target_environments: &[&str],
) -> Hooks {
    session.hooks().with_environment_hooks(
        environment_hook_sources(environments, target_environments),
        Arc::new(EnvironmentCommandExecutor {
            environments: environments.clone(),
            session: Arc::clone(session),
            turn: Arc::clone(turn),
        }),
    )
}

pub(crate) fn environment_hook_sources(
    environments: &TurnEnvironmentSnapshot,
    target_environments: &[&str],
) -> Vec<EnvironmentHookSource> {
    environments
        .turn_environments()
        .filter(|environment| {
            !environment.config().environment_hooks.is_empty()
                && (target_environments.is_empty()
                    || target_environments.contains(&environment.selection.environment_id.as_str()))
        })
        .map(|environment| EnvironmentHookSource {
            environment_id: environment.selection.environment_id.clone(),
            cwd: environment.cwd().clone(),
            hooks: environment.config().environment_hooks.clone(),
        })
        .collect()
}

struct EnvironmentCommandExecutor {
    environments: TurnEnvironmentSnapshot,
    session: Arc<Session>,
    turn: Arc<TurnContext>,
}

impl EnvironmentCommandExecutor {
    async fn launch_params(
        &self,
        environment: &TurnEnvironment,
        argv: Vec<String>,
        env: HashMap<String, String>,
    ) -> Result<(ExecParams, Option<NetworkProxy>), ToolError> {
        let process_id = ProcessId::from(format!("hook-{}", uuid::Uuid::new_v4()));
        let network = execution_network_proxy(
            &self.session,
            &self.turn,
            Some(environment),
            self.turn.network.clone(),
            &environment.selection.environment_id,
            environment.permission_profile(),
            environment.config().network_policy.as_ref(),
            process_id.as_str(),
            /*fallback_policy_decider*/ None,
        )
        .await?;
        let network_proxy = if let Some(network) = &network {
            let (launch, _) = remote_network_proxy_launch(environment, network, &self.turn).await?;
            launch.proxy.enabled.then_some(launch)
        } else {
            None
        };
        let sandbox = network_proxy.as_ref().map(|_| {
            // Enforce managed network policy with the hook owner's filesystem access.
            let mut sandbox = environment.sandbox_context(/*additional_permissions*/ None);
            sandbox.permissions = PermissionProfile::Disabled;
            sandbox
        });
        let params = ExecParams {
            process_id,
            metadata: None,
            argv,
            cwd: environment.cwd().clone(),
            env_policy: Some(exec_env_policy_from_shell_policy(
                environment.shell_environment_policy(),
            )),
            shell_snapshot: None,
            env,
            tty: false,
            pipe_stdin: true,
            arg0: None,
            enforce_managed_network: sandbox.is_some(),
            sandbox,
            managed_network: None,
            network_proxy,
        };
        Ok((params, network))
    }
}

struct HookProcessGuard(Option<Arc<dyn ExecProcess>>);

impl Drop for HookProcessGuard {
    fn drop(&mut self) {
        // Don't let waiting for termination delay a hook timeout or error.
        if let Some(process) = self.0.take() {
            tokio::spawn(async move {
                let _ = process.terminate().await;
            });
        }
    }
}

impl HookEnvironmentExecutor for EnvironmentCommandExecutor {
    fn execute(
        &self,
        environment_id: &str,
        command: &str,
        env: &HashMap<String, String>,
        input_json: &str,
        timeout: Duration,
    ) -> BoxFuture<'_, anyhow::Result<EnvironmentHookOutput>> {
        let environment_id = environment_id.to_owned();
        let command = command.to_owned();
        let env = env.clone();
        let input_json = input_json.to_owned();
        async move {
            let environment = self
                .environments
                .turn_environments()
                .find(|environment| environment.selection.environment_id == environment_id)
                .context("hook environment was not ready for this event")?;
            let shell = environment
                .shell
                .as_ref()
                .context("hook environment did not report a shell")?;

            // Exec-server keeps pipe stdin open and older executors have no close-stdin RPC.
            // JSON is one line: read precisely that line, then give the hook a pipe that reaches EOF.
            let input_json = format!("{input_json}\n").into_bytes();
            let script = match shell.shell_type {
                // Start the hook on a new line so a leading `(` doesn't become Bash's arithmetic `((`.
                ShellType::Sh | ShellType::Bash | ShellType::Zsh => format!(
                    "IFS= read -r codex_hook_input; printf '%s' \"$codex_hook_input\" | (\n{command}\n)"
                ),
                // A script block can reset `$?`. Run the hook at the top level and preserve its
                // exit code before PowerShell's -Command reduces it to 0 or 1.
                ShellType::PowerShell => format!(
                    "$global:LASTEXITCODE = 0\n\
                     [Console]::In.ReadLine() | {command}\n\
                     if ($?) {{ exit 0 }}\n\
                     if ($LASTEXITCODE) {{ exit $LASTEXITCODE }}\n\
                     exit 1"
                ),
                ShellType::Cmd => bail!("environment command hooks require PowerShell on Windows"),
            };
            let argv = shell.derive_exec_args(&script, environment.config().allow_login_shell);
            // Keep network attribution registered until the hook and its output are complete.
            let (params, _execution_network) = self
                .launch_params(environment, argv, env)
                .await
                .map_err(|error| match error {
                    ToolError::Rejected(message) => anyhow::anyhow!("{message}"),
                    ToolError::Codex(error) => anyhow::Error::new(error),
                })?;
            let deadline = tokio::time::Instant::now() + timeout;
            let backend = environment.environment.get_exec_backend();
            let process = tokio::time::timeout_at(deadline, backend.start(params))
                .await
                .context("environment command hook timed out during launch")??
                .process;
            let mut guard = HookProcessGuard(Some(Arc::clone(&process)));
            let output = tokio::time::timeout_at(deadline, async {
                let written = process.write(input_json).await?;
                if written.status != codex_exec_server::WriteStatus::Accepted {
                    bail!("hook process did not accept its input: {:?}", written.status);
                }
                let mut after_seq = None;
                let mut stdout = Vec::new();
                let mut stderr = Vec::new();
                loop {
                    let result = process.read(after_seq, Some(64 * 1024), Some(1000)).await?;
                    if let Some(failure) = result.failure {
                        bail!("{failure}");
                    }
                    // Closed describes the process, not this bounded page. Keep
                    // reading retained output until we receive an empty final page.
                    let drained = result.closed && result.chunks.is_empty();
                    for chunk in result.chunks {
                        let target = match chunk.stream {
                            ExecOutputStream::Stdout | ExecOutputStream::Pty => &mut stdout,
                            ExecOutputStream::Stderr => &mut stderr,
                        };
                        let remaining = MAX_HOOK_OUTPUT_BYTES.saturating_sub(target.len());
                        target.extend_from_slice(&chunk.chunk.0[..remaining.min(chunk.chunk.0.len())]);
                    }
                    after_seq = result.next_seq.checked_sub(1).or(after_seq);
                    if drained {
                        return Ok(EnvironmentHookOutput {
                            exit_code: result.exit_code.context("hook process closed without exit status")?,
                            stdout: String::from_utf8_lossy(&stdout).into_owned(),
                            stderr: String::from_utf8_lossy(&stderr).into_owned(),
                        });
                    }
                }
            })
            .await
            .context("environment command hook timed out")??;
            guard.0 = None;
            Ok(output)
        }
        .boxed()
    }
}
