use std::future::Future;
use std::sync::Arc;

use codex_hooks::SessionStartOutcome;
use codex_hooks::UserPromptSubmitOutcome;
use codex_hooks::UserPromptSubmitRequest;
use codex_protocol::items::TurnItem;
use codex_protocol::models::DeveloperInstructions;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::models::ResponseItemMetadata;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::HookCompletedEvent;
use codex_protocol::protocol::HookRunSummary;
use codex_protocol::user_input::UserInput;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::event_mapping::parse_turn_item;

pub(crate) struct HookRuntimeOutcome {
    pub should_stop: bool,
    pub additional_contexts: Vec<String>,
}

pub(crate) enum PendingInputHookDisposition {
    Accepted(Box<PendingInputRecord>),
    Blocked { additional_contexts: Vec<String> },
}

pub(crate) enum PendingInputRecord {
    UserMessage {
        content: Vec<UserInput>,
        response_item: ResponseItem,
        additional_contexts: Vec<String>,
    },
    ConversationItem {
        response_item: ResponseItem,
    },
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
            additional_contexts,
        } = value;
        Self {
            hook_events,
            outcome: HookRuntimeOutcome {
                should_stop,
                additional_contexts,
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
            additional_contexts,
        } = value;
        Self {
            hook_events,
            outcome: HookRuntimeOutcome {
                should_stop,
                additional_contexts,
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
        transcript_path: sess.hook_transcript_path().await,
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
    .record_additional_contexts(sess, turn_context)
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
        transcript_path: sess.hook_transcript_path().await,
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

pub(crate) async fn inspect_pending_input(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    pending_input_item: ResponseInputItem,
) -> PendingInputHookDisposition {
    let response_item = ResponseItem::from(pending_input_item);
    if let Some(TurnItem::UserMessage(user_message)) = parse_turn_item(&response_item) {
        let user_prompt_submit_outcome =
            run_user_prompt_submit_hooks(sess, turn_context, user_message.message()).await;
        if user_prompt_submit_outcome.should_stop {
            PendingInputHookDisposition::Blocked {
                additional_contexts: user_prompt_submit_outcome.additional_contexts,
            }
        } else {
            PendingInputHookDisposition::Accepted(Box::new(PendingInputRecord::UserMessage {
                content: user_message.content,
                response_item,
                additional_contexts: user_prompt_submit_outcome.additional_contexts,
            }))
        }
    } else {
        PendingInputHookDisposition::Accepted(Box::new(PendingInputRecord::ConversationItem {
            response_item,
        }))
    }
}

pub(crate) async fn record_pending_input(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    pending_input: PendingInputRecord,
    message_metadata: Option<ResponseItemMetadata>,
) {
    match pending_input {
        PendingInputRecord::UserMessage {
            content,
            response_item,
            additional_contexts,
        } => {
            sess.record_user_prompt_and_emit_turn_item(
                turn_context.as_ref(),
                content.as_slice(),
                response_item,
                message_metadata,
            )
            .await;
            record_additional_contexts(sess, turn_context, additional_contexts).await;
        }
        PendingInputRecord::ConversationItem { response_item } => {
            sess.record_conversation_items(turn_context, std::slice::from_ref(&response_item))
                .await;
        }
    }
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
    outcome.outcome
}

impl HookRuntimeOutcome {
    async fn record_additional_contexts(
        self,
        sess: &Arc<Session>,
        turn_context: &Arc<TurnContext>,
    ) -> bool {
        record_additional_contexts(sess, turn_context, self.additional_contexts).await;

        self.should_stop
    }
}

pub(crate) async fn record_additional_contexts(
    sess: &Arc<Session>,
    turn_context: &Arc<TurnContext>,
    additional_contexts: Vec<String>,
) {
    let developer_messages = additional_context_messages(additional_contexts);
    if developer_messages.is_empty() {
        return;
    }

    sess.record_conversation_items(turn_context, developer_messages.as_slice())
        .await;
}

fn additional_context_messages(additional_contexts: Vec<String>) -> Vec<ResponseItem> {
    additional_contexts
        .into_iter()
        .map(|additional_context| DeveloperInstructions::new(additional_context).into())
        .collect()
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

#[cfg(test)]
mod tests {
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    use super::additional_context_messages;

    #[test]
    fn additional_context_messages_stay_separate_and_ordered() {
        let messages = additional_context_messages(vec![
            "first tide note".to_string(),
            "second tide note".to_string(),
        ]);

        assert_eq!(messages.len(), 2);
        assert_eq!(
            messages
                .iter()
                .map(|message| match message {
                    codex_protocol::models::ResponseItem::Message { role, content, .. } => {
                        let text = content
                            .iter()
                            .map(|item| match item {
                                ContentItem::InputText { text } => text.as_str(),
                                ContentItem::InputImage { .. } | ContentItem::OutputText { .. } => {
                                    panic!("expected input text content, got {item:?}")
                                }
                            })
                            .collect::<String>();
                        (role.as_str(), text)
                    }
                    other => panic!("expected developer message, got {other:?}"),
                })
                .collect::<Vec<_>>(),
            vec![
                ("developer", "first tide note".to_string()),
                ("developer", "second tide note".to_string()),
            ],
        );
    }
}
