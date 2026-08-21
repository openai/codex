use std::collections::HashMap;
#[cfg(not(windows))]
use std::ffi::OsStr;
use std::ffi::OsString;
use std::future::Future;
use std::io::ErrorKind;
use std::path::Path;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::MutexGuard;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
use codex_protocol::shell_environment::scrub_non_inheritable_env_vars;
#[cfg(windows)]
use codex_utils_pty::JobObject;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use tokio::time::timeout;
use tracing::Span;

use super::CommandShell;
use super::ConfiguredHandler;
use super::ConfiguredHandlerKind;
use super::HandlerRunResult;
use super::dispatcher::ParsedHandler;
use super::dispatcher::hook_event_name_label;
use super::dispatcher::hook_execution_mode_label;
use super::dispatcher::hook_handler_type_label;
use super::dispatcher::hook_scope_label;
use super::dispatcher::hook_source_label;
use super::dispatcher::scope_for_event;
use crate::output_spill::AdditionalContext;
use crate::output_spill::HookOutputSpiller;
use codex_protocol::ThreadId;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookHandlerType;
use codex_protocol::protocol::HookOutputEntry;
use codex_protocol::protocol::HookOutputEntryKind;

const MAX_CONCURRENT_ASYNC_HOOKS: usize = 8;

/// Owns command execution and bounded asynchronous work for one session.
#[derive(Clone)]
pub(crate) struct CommandHookRuntime {
    shell: CommandShell,
    environment: Arc<Vec<(OsString, OsString)>>,
    result_sender: Sender<HookCompletedEvent>,
    state: Arc<Mutex<CommandHookRuntimeState>>,
    output_spiller: HookOutputSpiller,
}

struct CommandHookRuntimeState {
    concurrency_limit: Arc<Semaphore>,
    tasks: JoinSet<()>,
}

impl Default for CommandHookRuntimeState {
    fn default() -> Self {
        Self {
            concurrency_limit: Arc::new(Semaphore::new(MAX_CONCURRENT_ASYNC_HOOKS)),
            tasks: JoinSet::new(),
        }
    }
}

impl CommandHookRuntime {
    pub(crate) fn new(
        shell: CommandShell,
        environment: Arc<Vec<(OsString, OsString)>>,
        thread_id: ThreadId,
        result_sender: Sender<HookCompletedEvent>,
    ) -> Self {
        Self {
            shell,
            environment,
            result_sender,
            state: Arc::new(Mutex::new(CommandHookRuntimeState::default())),
            output_spiller: HookOutputSpiller::new(thread_id),
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, CommandHookRuntimeState> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    pub(crate) fn reconfigured(&self, shell: CommandShell) -> Self {
        Self {
            shell,
            environment: Arc::clone(&self.environment),
            result_sender: self.result_sender.clone(),
            state: Arc::clone(&self.state),
            output_spiller: self.output_spiller.clone(),
        }
    }

    pub(crate) fn output_spiller(&self) -> &HookOutputSpiller {
        &self.output_spiller
    }

    pub(crate) fn schedule_async_hook<T: 'static>(
        &self,
        handler: ConfiguredHandler,
        input_json: String,
        cwd: std::path::PathBuf,
        turn_id: Option<String>,
        parse: fn(&ConfiguredHandler, HandlerRunResult, Option<String>) -> ParsedHandler<T>,
    ) {
        if self.result_sender.is_closed() {
            return;
        }

        let result_sender = self.result_sender.clone();
        let runtime = self.clone();
        self.schedule_async_task(async move {
            let result = match &handler.kind {
                ConfiguredHandlerKind::Command { command, env, .. } => {
                    run_command(&runtime, &handler, command, env, &input_json, &cwd).await
                }
                ConfiguredHandlerKind::McpTool { .. } => return,
            };
            let mut hook_result = parse(&handler, result, turn_id).completed;
            let mut entries = Vec::new();
            let mut warnings = Vec::new();

            for entry in std::mem::take(&mut hook_result.run.entries) {
                match entry.kind {
                    HookOutputEntryKind::Context => {
                        if let Some(text) = runtime
                            .output_spiller
                            .maybe_spill_additional_contexts(vec![AdditionalContext {
                                text: entry.text,
                                limit: handler.additional_context_limit,
                            }])
                            .await
                            .into_iter()
                            .next()
                        {
                            entries.push(HookOutputEntry {
                                kind: HookOutputEntryKind::Context,
                                text,
                            });
                        }
                    }
                    HookOutputEntryKind::Warning => warnings.push(entry),
                    HookOutputEntryKind::Error => entries.push(entry),
                    HookOutputEntryKind::Stop | HookOutputEntryKind::Feedback => {}
                }
            }

            entries.extend(warnings);
            hook_result.run.entries = entries;
            let _ = result_sender.try_send(hook_result);
        });
    }

    pub(crate) fn schedule_async_task(&self, task: impl Future<Output = ()> + Send + 'static) {
        let mut state = self.lock_state();
        if state.concurrency_limit.is_closed() {
            return;
        }

        while state.tasks.try_join_next().is_some() {}
        let concurrency_limit = Arc::clone(&state.concurrency_limit);
        state.tasks.spawn(async move {
            let Ok(_permit) = concurrency_limit.acquire_owned().await else {
                return;
            };
            task.await;
        });
    }

    pub(crate) async fn shutdown(&self) {
        let mut tasks = {
            let mut state = self.lock_state();
            state.concurrency_limit.close();
            std::mem::take(&mut state.tasks)
        };
        tasks.abort_all();
        while tasks.join_next().await.is_some() {}
    }
}

#[tracing::instrument(
    name = "codex.hooks.command",
    level = "trace",
    skip_all,
    fields(
        hook.event_name = hook_event_name_label(handler.event_name),
        hook.handler_type = hook_handler_type_label(HookHandlerType::Command),
        hook.execution_mode = hook_execution_mode_label(handler.execution_mode()),
        hook.scope = hook_scope_label(scope_for_event(handler.event_name)),
        hook.source = hook_source_label(handler.source),
        hook.display_order = handler.display_order,
        hook.timeout_sec = handler.timeout_sec,
        hook.command_outcome = tracing::field::Empty,
    )
)]
pub(crate) async fn run_command(
    runtime: &CommandHookRuntime,
    handler: &ConfiguredHandler,
    command: &str,
    env: &HashMap<String, String>,
    input_json: &str,
    cwd: &Path,
) -> HandlerRunResult {
    let started_at = chrono::Utc::now().timestamp();
    let started = Instant::now();

    let mut command = build_command(&runtime.shell, command, &runtime.environment, env);
    command
        .current_dir(cwd)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    #[cfg(unix)]
    command.process_group(0);

    #[cfg(windows)]
    let mut process_tree_job = JobObject::create().ok();
    #[cfg(windows)]
    let child = match process_tree_job.as_ref() {
        Some(job) => match job.spawn_contained(&mut command) {
            Ok(child) => Ok(child),
            Err(_) => {
                process_tree_job = None;
                command.creation_flags(0);
                command.spawn()
            }
        },
        None => command.spawn(),
    };
    #[cfg(not(windows))]
    let child = command.spawn();

    let mut child = match child {
        Ok(child) => child,
        Err(err) => {
            return finish_command_run(
                started_at,
                started,
                CommandRunCompletion {
                    exit_code: None,
                    stdout: String::new(),
                    stderr: String::new(),
                    error: Some(err.to_string()),
                    outcome: "spawn_error",
                },
            );
        }
    };

    let mut process_tree_guard = ProcessTreeGuard {
        process_id: child.id(),
        #[cfg(windows)]
        job: process_tree_job,
    };

    if let Some(mut stdin) = child.stdin.take()
        && let Err(err) = stdin.write_all(input_json.as_bytes()).await
        && err.kind() != ErrorKind::BrokenPipe
    {
        let _ = child.kill().await;
        return finish_command_run(
            started_at,
            started,
            CommandRunCompletion {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("failed to write hook stdin: {err}")),
                outcome: "stdin_error",
            },
        );
    }

    let timeout_duration = Duration::from_secs(handler.timeout_sec);
    match timeout(timeout_duration, child.wait_with_output()).await {
        Ok(Ok(output)) => {
            // Successfully completed hooks may intentionally leave detached helpers running.
            #[cfg(windows)]
            if let Some(job) = process_tree_guard.job.as_ref() {
                let _ = job.preserve_descendants();
            }
            process_tree_guard.process_id = None;
            finish_command_run(
                started_at,
                started,
                CommandRunCompletion {
                    exit_code: output.status.code(),
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    error: None,
                    outcome: "completed",
                },
            )
        }
        Ok(Err(err)) => finish_command_run(
            started_at,
            started,
            CommandRunCompletion {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(err.to_string()),
                outcome: "wait_error",
            },
        ),
        Err(_) => finish_command_run(
            started_at,
            started,
            CommandRunCompletion {
                exit_code: None,
                stdout: String::new(),
                stderr: String::new(),
                error: Some(format!("hook timed out after {}s", handler.timeout_sec)),
                outcome: "timeout",
            },
        ),
    }
}

// Needed only until command hooks move to the exec server, which owns process-tree cleanup.
struct ProcessTreeGuard {
    process_id: Option<u32>,
    #[cfg(windows)]
    job: Option<JobObject>,
}

impl Drop for ProcessTreeGuard {
    fn drop(&mut self) {
        let Some(process_id) = self.process_id else {
            return;
        };

        #[cfg(unix)]
        {
            let _ = codex_utils_pty::process_group::kill_process_group(process_id);
        }

        #[cfg(windows)]
        {
            if let Some(job) = self.job.as_ref() {
                let _ = job.terminate();
            } else {
                let _ = std::process::Command::new("taskkill")
                    .args(["/PID", &process_id.to_string(), "/T", "/F"])
                    .stdin(Stdio::null())
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .spawn();
            }
        }
    }
}

struct CommandRunCompletion {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
    error: Option<String>,
    outcome: &'static str,
}

fn finish_command_run(
    started_at: i64,
    started: Instant,
    completion: CommandRunCompletion,
) -> HandlerRunResult {
    Span::current().record("hook.command_outcome", completion.outcome);
    HandlerRunResult {
        started_at,
        completed_at: chrono::Utc::now().timestamp(),
        duration_ms: started.elapsed().as_millis().try_into().unwrap_or(i64::MAX),
        exit_code: completion.exit_code,
        stdout: completion.stdout,
        stderr: completion.stderr,
        error: completion.error,
    }
}

fn build_command(
    shell: &CommandShell,
    command_line: &str,
    environment: &[(OsString, OsString)],
    env: &HashMap<String, String>,
) -> Command {
    let mut command = if shell.program.is_empty() {
        default_shell_command(environment)
    } else {
        Command::new(&shell.program)
    };
    if shell.program.is_empty() {
        #[cfg(windows)]
        command.raw_arg(format!(r#""{command_line}""#));

        #[cfg(not(windows))]
        command.arg(command_line);
    } else {
        command.args(&shell.args);

        #[cfg(windows)]
        if shell.args.iter().any(|arg| arg.eq_ignore_ascii_case("/c")) {
            command.raw_arg(format!(r#""{command_line}""#));
        } else {
            command.arg(command_line);
        }

        #[cfg(not(windows))]
        command.arg(command_line);
    }
    // Replay the session snapshot instead of inheriting the live process environment.
    command.env_clear();
    command.envs(environment.iter().cloned());
    command.envs(env);
    scrub_non_inheritable_env_vars(command.as_std_mut());
    command
}

fn default_shell_command(environment: &[(OsString, OsString)]) -> Command {
    #[cfg(windows)]
    let (environment_variable, fallback_program, argument) = ("COMSPEC", "cmd.exe", "/C");

    #[cfg(not(windows))]
    let (environment_variable, fallback_program, argument) = ("SHELL", "/bin/sh", "-lc");

    let program = environment
        .iter()
        .find(|(key, _)| {
            #[cfg(windows)]
            {
                key.to_str()
                    .is_some_and(|key| key.eq_ignore_ascii_case(environment_variable))
            }

            #[cfg(not(windows))]
            {
                key == OsStr::new(environment_variable)
            }
        })
        .map(|(_, value)| value.clone())
        .unwrap_or_else(|| OsString::from(fallback_program));

    let mut command = Command::new(program);
    command.arg(argument);
    command
}

#[cfg(test)]
#[path = "command_runner_tests.rs"]
mod tests;
