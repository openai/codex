//! Acquires host execution capacity without blocking the submission dispatcher.
//! Input stays with the registered task until admission succeeds, so abort owns cleanup.

use std::sync::Arc;

use codex_async_utils::OrCancelExt;
use codex_protocol::error::CodexErr;
use codex_protocol::error::CodexErrorDetails;
use codex_protocol::error::Result;
use tokio_util::sync::CancellationToken;

use crate::agent::host::AgentTurnAdmission;
use crate::agent::host::AgentTurnAdmissionRequest;
use crate::hook_environment_executor::with_environment_hooks;
use crate::hook_runtime::record_pending_input;
use crate::session::TurnInput;
use crate::session::session::Session;
use crate::session::turn::recordable_inputs;
use crate::session::turn::run_hooks_and_record_inputs;
use crate::session::turn_context::TurnContext;
use crate::state::TaskKind;
use crate::state::TurnState;
use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::TurnAbortReason;
use codex_thread_store::PersistContext;
use tokio::sync::Mutex;

/// The host admission path already emitted the logical turn's start event.
pub(crate) struct HostTurnStarted;

pub(super) async fn admit(
    session: &Arc<Session>,
    context: &Arc<TurnContext>,
    task_kind: TaskKind,
    provider: &dyn AgentTurnAdmission,
    cancellation: &CancellationToken,
) -> Result<Vec<TurnInput>> {
    // Review forwards the delegate's start event. Other tasks announce the attempt
    // here so an unresolved admission remains visible and interruptible.
    if task_kind != TaskKind::Review {
        session.emit_turn_started(context, task_kind).await;
        context.extension_data.insert(HostTurnStarted);
    }
    let guard = provider
        .admit_turn(AgentTurnAdmissionRequest {
            thread_id: session.thread_id,
            turn_id: context.sub_id.clone(),
        })
        .or_cancel(cancellation)
        .await
        .map_err(|_| CodexErr::new(CodexErrorDetails::TurnAborted))??;

    // Acquiring this lock waits for turn registration to finish before attaching the permit.
    // A cancelled/replaced task must never install its permit into a newer task,
    // even if recovery reused the turn ID.
    let mut active = session.active_turn.lock().await;
    let task = active
        .as_mut()
        .and_then(|turn| turn.task.as_mut())
        .filter(|task| Arc::ptr_eq(&task.turn_context, context))
        .filter(|_| !cancellation.is_cancelled())
        .ok_or_else(|| CodexErr::new(CodexErrorDetails::TurnAborted))?;
    task._agent_execution_guard = Some(guard);
    task.pending_initial_input.take().ok_or_else(|| {
        CodexErr::Fatal("initial input missing from the task awaiting host admission".into())
    })
}

impl Session {
    pub(crate) fn host_turn_admission(&self) -> Option<Arc<dyn AgentTurnAdmission>> {
        match &self.agent_runtime {
            crate::agent::host::AgentRuntimeServices::Controller => None,
            crate::agent::host::AgentRuntimeServices::Host(services) => {
                Some(Arc::clone(&services.turn_admission))
            }
        }
    }

    /// Interruption and suspension stop the task before it can persist its initial input.
    pub(crate) async fn record_pending_initial_input(
        self: &Arc<Self>,
        context: &Arc<TurnContext>,
        turn_state: &Mutex<TurnState>,
        input: Option<Vec<TurnInput>>,
        abort_reason: Option<&TurnAbortReason>,
    ) -> Result<()> {
        let Some(mut input) = input else {
            return Ok(());
        };
        input.extend(
            self.input_queue
                .take_pending_input_for_turn_state(turn_state)
                .await,
        );
        let model_info = context.capture_current_model_info();
        if abort_reason != Some(&TurnAbortReason::Interrupted) {
            run_hooks_and_record_inputs(
                self,
                context,
                &model_info,
                &input,
                PersistContext::Standard,
            )
            .await?;
            return Ok(());
        }

        for item in recordable_inputs(self, &model_info, &input) {
            let item = item?;
            // The user must resubmit a prompt that still needs a submit hook.
            if matches!(item.as_ref(), TurnInput::UserInput { .. })
                && with_environment_hooks(
                    self,
                    context,
                    &context.initial_environments.refresh_readiness(),
                )
                .execution_mode_for_event(HookEventName::UserPromptSubmit)
                .is_some()
            {
                continue;
            }
            record_pending_input(
                self,
                context,
                &model_info,
                item.into_owned(),
                Vec::new(),
                PersistContext::Standard,
            )
            .await;
        }
        Ok(())
    }
}
