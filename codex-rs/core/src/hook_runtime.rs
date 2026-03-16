use std::future::Future;
use std::sync::Arc;

use codex_hooks::SessionStartOutcome;
use codex_hooks::UserPromptSubmitOutcome;
use codex_hooks::UserPromptSubmitRequest;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookRunSummary;

use crate::codex::Session;
use crate::codex::TurnContext;

pub(crate) struct HookRuntimeOutcome {
    pub should_stop: bool,
    pub additional_context: Option<String>,
}

struct ContextInjectingHookOutcome {
    hook_events: Vec<HookCompletedEvent>,
    outcome: HookRuntimeOutcome,
}

impl From<SessionStartOutcome> for ContextInjectingHookOutcome {
    fn from(value: SessionStartOutcome) -> Self {
        let SessionStartOutcome {
            hook_events,
            should_stop,
            stop_reason: _,
            additional_context,
        } = value;
        Self {
            hook_events,
            outcome: HookRuntimeOutcome {
                should_stop,
                additional_context,
            },
        }
    }
}

impl From<UserPromptSubmitOutcome> for ContextInjectingHookOutcome {
    fn from(value: UserPromptSubmitOutcome) -> Self {
        let UserPromptSubmitOutcome {
            hook_events,
            should_stop,
            stop_reason: _,
            additional_context,
        } = value;
        Self {
            hook_events,
            outcome: HookRuntimeOutcome {
                should_stop,
                additional_context,
            },
        }
    }
}

pub(crate) async fn run_pending_session_start_hooks(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
) -> bool {
    let Some(session_start_source) = sess.take_pending_session_start_source().await else {
        return false;
    };

    let request = codex_hooks::SessionStartRequest {
        session_id: sess.conversation_id,
        cwd: turn_context.cwd.clone(),
        transcript_path: sess.current_rollout_path().await,
        model: turn_context.model_info.slug.clone(),
        permission_mode: hook_permission_mode(turn_context),
        source: session_start_source,
    };
    let preview_runs = sess.hooks().preview_session_start(&request);
    run_context_injecting_hook(
        sess,
        turn_context,
        preview_runs,
        sess.hooks()
            .run_session_start(request, Some(turn_context.sub_id.clone())),
    )
    .await
    .record_additional_context(sess, turn_context)
    .await
}

pub(crate) async fn run_user_prompt_submit_hooks(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    prompt: String,
) -> HookRuntimeOutcome {
    let request = UserPromptSubmitRequest {
        session_id: sess.conversation_id,
        turn_id: turn_context.sub_id.clone(),
        cwd: turn_context.cwd.clone(),
        transcript_path: sess.current_rollout_path().await,
        model: turn_context.model_info.slug.clone(),
        permission_mode: hook_permission_mode(turn_context),
        prompt,
    };
    let preview_runs = sess.hooks().preview_user_prompt_submit(&request);
    run_context_injecting_hook(
        sess,
        turn_context,
        preview_runs,
        sess.hooks().run_user_prompt_submit(request),
    )
    .await
}

async fn run_context_injecting_hook<Fut, Outcome>(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    preview_runs: Vec<HookRunSummary>,
    outcome_future: Fut,
) -> HookRuntimeOutcome
where
    Fut: Future<Output = Outcome>,
    Outcome: Into<ContextInjectingHookOutcome>,
{
    emit_hook_started_events(sess, turn_context, preview_runs).await;

    let outcome = outcome_future.await.into();
    emit_hook_completed_events(sess, turn_context, outcome.hook_events).await;
    HookRuntimeOutcome {
        should_stop: outcome.outcome.should_stop,
        additional_context: if outcome.outcome.should_stop {
            None
        } else {
            outcome.outcome.additional_context
        },
    }
}

impl HookRuntimeOutcome {
    async fn record_additional_context(
        self,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
    ) -> bool {
        if let Some(additional_context) = self.additional_context {
            record_additional_context(sess, turn_context, additional_context).await;
        }

        self.should_stop
    }
}

pub(crate) async fn record_additional_context(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    additional_context: String,
) {
    let developer_message: ResponseItem = DeveloperInstructions::new(additional_context).into();
    sess.record_conversation_items(turn_context, std::slice::from_ref(&developer_message))
        .await;
}

async fn emit_hook_started_events(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    preview_runs: Vec<HookRunSummary>,
) {
    for run in preview_runs {
        sess.send_event(
            turn_context,
            EventMsg::HookStarted(crate::protocol::HookStartedEvent {
                turn_id: Some(turn_context.sub_id.clone()),
                run,
            }),
        )
        .await;
    }
}

async fn emit_hook_completed_events(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    completed_events: Vec<HookCompletedEvent>,
) {
    for completed in completed_events {
        sess.send_event(turn_context, EventMsg::HookCompleted(completed))
            .await;
    }
}

fn hook_permission_mode(turn_context: &TurnContext) -> String {
    match turn_context.approval_policy.value() {
        AskForApproval::Never => "bypassPermissions",
        AskForApproval::UnlessTrusted
        | AskForApproval::OnFailure
        | AskForApproval::OnRequest
        | AskForApproval::Granular(_) => "default",
    }
    .to_string()
}
