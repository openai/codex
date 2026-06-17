//! Runs remote shell commands through the existing unified-exec process lifecycle.
//!
//! This adapter intentionally keeps unified exec's combined output and process lifecycle
//! semantics. It only waits for completion and converts the result for `shell_command`.

use std::io;
use std::sync::Arc;

use codex_exec_server::Environment;
use codex_protocol::error::CodexErr;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use tokio::sync::Mutex;
use tokio::time::Instant;

use super::NoopSpawnLifecycle;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use super::UnifiedExecProcessManager;
use super::async_watcher::start_streaming_output;
use super::head_tail_buffer::HeadTailBuffer;
use crate::exec::ExecExpirationOutcome;
use crate::exec::is_likely_sandbox_denied;
use crate::sandboxing::ExecRequest;

impl UnifiedExecProcessManager {
    pub(crate) async fn execute_remote_shell(
        &self,
        mut request: ExecRequest,
        environment: &Environment,
        context: &UnifiedExecContext,
    ) -> Result<ExecToolCallOutput, CodexErr> {
        if let Some(network) = request.network.as_ref() {
            network.apply_to_env(&mut request.env);
        }

        let expiration = request.expiration.clone();
        let sandbox = request.sandbox;
        let process_id = self.allocate_process_id().await;
        let started_at = Instant::now();
        let process = match self
            .open_session_with_exec_env(
                process_id,
                &request,
                /*tty*/ false,
                Box::new(NoopSpawnLifecycle),
                environment,
            )
            .await
        {
            Ok(process) => process,
            Err(error) => {
                self.release_process_id(process_id).await;
                return Err(match error {
                    UnifiedExecError::SandboxDenied { output, .. } => {
                        CodexErr::Sandbox(SandboxErr::Denied {
                            output: Box::new(output),
                            network_policy_decision: None,
                        })
                    }
                    error => CodexErr::Io(io::Error::other(error.to_string())),
                });
            }
        };

        start_streaming_output(
            &process,
            context,
            Arc::new(Mutex::new(HeadTailBuffer::default())),
        );
        let output_handles = process.output_handles();
        let output_drained = process.output_drained_notify();
        // Match unified exec by establishing the remote process before waiting for completion.
        let expiration_wait = expiration.wait_with_outcome();
        tokio::pin!(expiration_wait);
        let expiration_outcome = tokio::select! {
            _ = output_handles.cancellation_token.cancelled() => None,
            outcome = &mut expiration_wait => Some(outcome),
        };
        // Keep unified exec's existing termination behavior rather than adding a second drain path.
        if expiration_outcome.is_some() && process.terminate_confirmed().await.is_err() {
            process.terminate();
        }
        output_drained.notified().await;

        let bytes = output_handles.output_buffer.lock().await.to_bytes();
        // Unified exec exposes remote process output as one combined stream.
        let stdout = StreamOutput {
            text: bytes,
            truncated_after_lines: None,
        }
        .from_utf8_lossy();
        let timed_out = expiration_outcome == Some(ExecExpirationOutcome::TimedOut);
        let exit_code = match expiration_outcome {
            Some(ExecExpirationOutcome::TimedOut) => 124,
            Some(ExecExpirationOutcome::Cancelled) => 1,
            None => process.exit_code().unwrap_or(-1),
        };
        let output = ExecToolCallOutput {
            exit_code,
            stdout: stdout.clone(),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: stdout,
            duration: started_at.elapsed(),
            timed_out,
        };
        let failure_message = process.failure_message();
        self.release_process_id(process_id).await;

        if timed_out {
            return Err(CodexErr::Sandbox(SandboxErr::Timeout {
                output: Box::new(output),
            }));
        }
        if let Some(message) = failure_message {
            return Err(CodexErr::Io(io::Error::other(message)));
        }
        if is_likely_sandbox_denied(sandbox, &output) {
            return Err(CodexErr::Sandbox(SandboxErr::Denied {
                output: Box::new(output),
                network_policy_decision: None,
            }));
        }

        Ok(output)
    }
}
