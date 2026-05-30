use std::io::ErrorKind;
use std::path::Path;

use anyhow::Context;
use anyhow::Result;
use futures::StreamExt;
use futures::stream::FuturesUnordered;
use tempfile::TempPath;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::engine::CommandShell;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;
use crate::engine::command_runner::run_command;
use crate::engine::dispatcher;

/// Runs `SessionStart` handlers in parallel without sharing a writable env file.
///
/// Each handler receives its own scratch `CODEX_ENV_FILE` / `CLAUDE_ENV_FILE`
/// path. After all handlers finish, their scratch-file contents are merged back
/// into the canonical env file in configured handler order. Core can then source
/// and capture that one canonical file exactly as it does for any other
/// `SessionStart` run.
pub(super) async fn execute_handlers<T>(
    shell: &CommandShell,
    handlers: Vec<ConfiguredHandler>,
    input_json: String,
    cwd: &Path,
    turn_id: Option<String>,
    parse: fn(&ConfiguredHandler, CommandRunResult, Option<String>) -> dispatcher::ParsedHandler<T>,
) -> Vec<dispatcher::ParsedHandler<T>> {
    // No env file means there is nothing special to isolate; use the standard
    // parallel dispatcher.
    let Some(env_file_path) = shell.session_start_env_file.as_deref() else {
        return dispatcher::execute_handlers(shell, handlers, input_json, cwd, turn_id, parse)
            .await;
    };
    let env_file_path = Path::new(env_file_path);
    let scratch_dir = env_file_path.parent().unwrap_or_else(|| Path::new("."));

    let mut pending = FuturesUnordered::new();
    for (configured_order, handler) in handlers.into_iter().enumerate() {
        // Each handler writes to an isolated env file, but still sees both
        // compatibility aliases when the original shell had them.
        let scratch_result = tempfile::NamedTempFile::new_in(scratch_dir)
            .context("failed to create SessionStart scratch env file")
            .map(|scratch_file| {
                let scratch_path = scratch_file.into_temp_path();

                let mut scratch_shell = shell.clone();
                scratch_shell.session_start_env_file =
                    Some(scratch_path.to_string_lossy().to_string());

                (scratch_path, scratch_shell)
            });

        let input_json = input_json.clone();
        let turn_id = turn_id.clone();
        pending.push(async move {
            let (scratch_path, scratch_shell) = match scratch_result {
                Ok(scratch) => scratch,
                Err(error) => {
                    return HandlerExecution {
                        configured_order,
                        completion_order: 0,
                        handler,
                        run_result: error_result(error),
                        scratch_path: None,
                        turn_id,
                    };
                }
            };
            let run_result = run_command(&scratch_shell, &handler, &input_json, cwd).await;
            HandlerExecution {
                configured_order,
                completion_order: 0,
                handler,
                run_result,
                scratch_path: Some(scratch_path),
                turn_id,
            }
        });
    }

    let mut completed = Vec::new();
    let mut completion_order = 0;
    while let Some(mut execution) = pending.next().await {
        execution.completion_order = completion_order;
        completion_order += 1;
        completed.push(execution);
    }
    completed.sort_by_key(|execution| execution.configured_order);

    let mut env_file_ends_with_newline = match fs::read(env_file_path).await {
        Ok(contents) => contents.last().is_none_or(|byte| *byte == b'\n'),
        Err(error) if error.kind() == ErrorKind::NotFound => true,
        Err(error) => {
            tracing::warn!(
                "failed to read SessionStart env file {} before merge: {error}",
                env_file_path.display()
            );
            true
        }
    };

    // Merge each successful handler's isolated file in configured order. The
    // core session layer will source this single canonical file after hooks
    // finish.
    for execution in &mut completed {
        let Some(scratch_path) = execution.scratch_path.as_ref() else {
            continue;
        };
        let result: Result<bool> = async {
            if execution.run_result.error.is_some() || execution.run_result.exit_code != Some(0) {
                return Ok(env_file_ends_with_newline);
            }

            let contents = fs::read(scratch_path).await.with_context(|| {
                format!(
                    "failed to read SessionStart scratch env file {}",
                    scratch_path.display()
                )
            })?;
            if contents.is_empty() {
                return Ok(env_file_ends_with_newline);
            }

            let mut file = fs::OpenOptions::new()
                .append(true)
                .create(true)
                .open(env_file_path)
                .await
                .with_context(|| format!("failed to open {}", env_file_path.display()))?;
            if !env_file_ends_with_newline && contents.first() != Some(&b'\n') {
                file.write_all(b"\n")
                    .await
                    .with_context(|| format!("failed to append to {}", env_file_path.display()))?;
            }
            file.write_all(&contents)
                .await
                .with_context(|| format!("failed to append to {}", env_file_path.display()))?;
            file.flush()
                .await
                .with_context(|| format!("failed to flush {}", env_file_path.display()))?;
            Ok(contents.last() == Some(&b'\n'))
        }
        .await;

        match result {
            Ok(ends_with_newline) => {
                env_file_ends_with_newline = ends_with_newline;
            }
            Err(error) => {
                execution.run_result = error_result(error);
            }
        }
    }

    completed
        .into_iter()
        .map(|execution| {
            let mut parsed = parse(&execution.handler, execution.run_result, execution.turn_id);
            parsed.completion_order = execution.completion_order;
            parsed
        })
        .collect()
}

struct HandlerExecution {
    configured_order: usize,
    completion_order: usize,
    handler: ConfiguredHandler,
    run_result: CommandRunResult,
    scratch_path: Option<TempPath>,
    turn_id: Option<String>,
}

fn error_result(error: anyhow::Error) -> CommandRunResult {
    let timestamp = chrono::Utc::now().timestamp();
    CommandRunResult {
        started_at: timestamp,
        completed_at: timestamp,
        duration_ms: 0,
        exit_code: None,
        stdout: String::new(),
        stderr: String::new(),
        error: Some(format!("{error:#}")),
    }
}
