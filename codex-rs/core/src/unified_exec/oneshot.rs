//! One-shot execution over the shared unified-exec lifecycle.
//!
//! The caller never receives a resumable process. Timeout and cancellation
//! terminate the exact process handle published by the normal startup path.

use std::sync::Arc;
use std::sync::OnceLock;

use tokio::time::Duration;
use tokio::time::Instant;

use super::ExecCommandRequest;
use super::UnifiedExecContext;
use super::UnifiedExecError;
use super::UnifiedExecProcess;
use super::UnifiedExecProcessManager;
use crate::tools::context::ExecCommandToolOutput;

pub(super) struct Completion<'a> {
    pub timeout: Duration,
    pub timed_out: bool,
    pub process: &'a OnceLock<Arc<UnifiedExecProcess>>,
}

impl UnifiedExecProcessManager {
    pub(crate) async fn exec_command_to_completion(
        request: ExecCommandRequest,
        context: &UnifiedExecContext,
        timeout: Duration,
    ) -> Result<ExecCommandToolOutput, UnifiedExecError> {
        let context = UnifiedExecContext::new(
            Arc::clone(&context.session),
            Arc::clone(&context.step_context),
            context.cancellation_token.child_token(),
            context.call_id.clone(),
        );
        let _cancel_on_drop = context.cancellation_token.clone().drop_guard();
        tokio::spawn(async move {
            let manager = &context.session.services.unified_exec_manager;
            let process_id = request.process_id;
            if Instant::now().checked_add(timeout).is_none() {
                manager.release_process_id(process_id).await;
                return Err(UnifiedExecError::process_failed(
                    "timeout_ms is too large".into(),
                ));
            }

            let process = OnceLock::new();
            let mut completion = Completion {
                timeout,
                timed_out: false,
                process: &process,
            };
            let result = {
                let mut execution =
                    Box::pin(manager.exec_command_inner(request, &context, Some(&mut completion)));
                tokio::select! {
                    biased;
                    _ = context.cancellation_token.cancelled() => {
                        if let Some(process) = process.get() {
                            if !process.has_exited()
                                && let Err(err) = process.terminate_confirmed().await
                            {
                                process.fail_and_terminate(err.to_string());
                            }
                            let _ = execution.await;
                        } else {
                            drop(execution);
                            manager.release_process_id(process_id).await;
                        }
                        Err(UnifiedExecError::process_failed("command cancelled".into()))
                    }
                    result = &mut execution => result,
                }
            };

            result.map(|mut output| {
                if completion.timed_out {
                    output.process_id = None;
                }
                output
            })
        })
        .await
        .map_err(|err| UnifiedExecError::process_failed(err.to_string()))?
    }
}
