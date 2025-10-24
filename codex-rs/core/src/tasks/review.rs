use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::TaskCompleteEvent;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::AgentEvent;
use crate::codex_delegate::run_codex_conversation;
// use crate::config::Config; // no longer needed directly; use session.base_config()
use crate::review_format::format_review_findings_block;
use crate::state::TaskKind;
use codex_protocol::user_input::UserInput;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy, Default)]
pub(crate) struct ReviewTask;

#[async_trait]
impl SessionTask for ReviewTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Review
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let receiver = match start_review_conversation(
            session.clone(),
            ctx.clone(),
            input,
            cancellation_token.clone(),
        )
        .await
        {
            Some(receiver) => receiver,
            None => return None,
        };

        let exit_emitted = process_review_events(session.clone(), ctx.clone(), receiver).await;

        if !exit_emitted && !cancellation_token.is_cancelled() {
            // Ensure the parent session leaves review mode even if the sub-agent
            // finished without emitting a TaskComplete event (for example, when
            // cancellation interrupts the stream before completion parsing).
            emit_review_exit_on_abort(session.clone_session(), ctx.clone()).await;
        }

        None
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        let _ = (session, ctx);
    }
}

async fn start_review_conversation(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    input: Vec<UserInput>,
    cancellation_token: CancellationToken,
) -> Option<async_channel::Receiver<AgentEvent>> {
    let config = ctx.client.get_config().await;
    let mut sub_agent_config = config.as_ref().clone();
    sub_agent_config.user_instructions = None;
    sub_agent_config.project_doc_max_bytes = 0;
    sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
    match run_codex_conversation(
        sub_agent_config,
        session.auth_manager(),
        input,
        cancellation_token,
    )
    .await
    {
        Ok(receiver) => Some(receiver),
        Err(_) => {
            exit_review_mode(session.clone_session(), None, ctx).await;
            None
        }
    }
}

async fn process_review_events(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    receiver: async_channel::Receiver<AgentEvent>,
) -> bool {
    let mut exit_emitted = false;
    while let Ok(agent_event) = receiver.recv().await {
        match handle_review_agent_event(&session, &ctx, agent_event).await {
            ReviewEventAction::Continue => {}
            ReviewEventAction::Finish {
                exit_already_emitted,
            } => {
                exit_emitted = exit_already_emitted;
                break;
            }
        }
    }
    exit_emitted
}

async fn handle_review_agent_event(
    session: &Arc<SessionTaskContext>,
    ctx: &Arc<TurnContext>,
    agent_event: AgentEvent,
) -> ReviewEventAction {
    match agent_event {
        AgentEvent::EventMsg(event) => {
            match event {
                EventMsg::AgentMessage(_) => {
                    // The structured review output is surfaced through ExitedReviewMode.
                    // Suppress the raw AgentMessage to avoid leaking implementation details.
                    ReviewEventAction::Continue
                }
                EventMsg::TaskComplete(task_complete) => {
                    finalize_review_completion(session, ctx, task_complete).await;
                    ReviewEventAction::Finish {
                        exit_already_emitted: true,
                    }
                }
                EventMsg::TurnAborted(_) => {
                    // The parent session emits the authoritative TurnAborted once
                    // review teardown completes, so drop the sub-agent's copy to
                    // preserve the expected event ordering.
                    ReviewEventAction::Finish {
                        exit_already_emitted: false,
                    }
                }
                EventMsg::AgentMessageDelta(_)
                | EventMsg::TokenCount(_)
                | EventMsg::Error(_)
                | EventMsg::TaskStarted(_)
                | EventMsg::UserMessage(_)
                | EventMsg::AgentReasoning(_)
                | EventMsg::AgentReasoningDelta(_)
                | EventMsg::AgentReasoningRawContent(_)
                | EventMsg::AgentReasoningRawContentDelta(_)
                | EventMsg::AgentReasoningSectionBreak(_)
                | EventMsg::SessionConfigured(_)
                | EventMsg::McpToolCallBegin(_)
                | EventMsg::McpToolCallEnd(_)
                | EventMsg::WebSearchBegin(_)
                | EventMsg::WebSearchEnd(_)
                | EventMsg::ExecCommandBegin(_)
                | EventMsg::ExecCommandOutputDelta(_)
                | EventMsg::ExecCommandEnd(_)
                | EventMsg::ViewImageToolCall(_)
                | EventMsg::ExecApprovalRequest(_)
                | EventMsg::ApplyPatchApprovalRequest(_)
                | EventMsg::BackgroundEvent(_)
                | EventMsg::StreamError(_)
                | EventMsg::PatchApplyBegin(_)
                | EventMsg::PatchApplyEnd(_)
                | EventMsg::TurnDiff(_)
                | EventMsg::GetHistoryEntryResponse(_)
                | EventMsg::McpListToolsResponse(_)
                | EventMsg::ListCustomPromptsResponse(_)
                | EventMsg::PlanUpdate(_)
                | EventMsg::ShutdownComplete
                | EventMsg::ConversationPath(_)
                | EventMsg::EnteredReviewMode(_)
                | EventMsg::ExitedReviewMode(_)
                | EventMsg::ItemStarted(_)
                | EventMsg::ItemCompleted(_) => {
                    session
                        .clone_session()
                        .send_event(ctx.as_ref(), event)
                        .await;
                    ReviewEventAction::Continue
                }
            }
        }
        AgentEvent::ExecApprovalRequest(event, tx) => {
            let decision = session
                .clone_session()
                .request_command_approval(
                    ctx.as_ref(),
                    event.call_id.clone(),
                    event.command.clone(),
                    event.cwd.clone(),
                    event.reason.clone(),
                )
                .await;
            let _ = tx.send(decision);
            ReviewEventAction::Continue
        }
        AgentEvent::PatchApprovalRequest(event, tx) => {
            session
                .clone_session()
                .send_event(ctx.as_ref(), EventMsg::ApplyPatchApprovalRequest(event))
                .await;
            let _ = tx.send(codex_protocol::protocol::ReviewDecision::Denied);
            ReviewEventAction::Continue
        }
    }
}

enum ReviewEventAction {
    Continue,
    Finish { exit_already_emitted: bool },
}

async fn finalize_review_completion(
    session: &Arc<SessionTaskContext>,
    ctx: &Arc<TurnContext>,
    task_complete: TaskCompleteEvent,
) {
    let review_output = task_complete
        .last_agent_message
        .as_deref()
        .map(parse_review_output_event);

    exit_review_mode(session.clone_session(), review_output, ctx.clone()).await;
}

/// Emits an ExitedReviewMode Event with optional ReviewOutput,
/// and records a developer message with the review output.
pub(crate) async fn exit_review_mode(
    session: Arc<Session>,
    review_output: Option<ReviewOutputEvent>,
    ctx: Arc<TurnContext>,
) {
    let mut user_message = String::new();
    if let Some(out) = review_output.clone() {
        let mut findings_str = String::new();
        let text = out.overall_explanation.trim();
        if !text.is_empty() {
            findings_str.push_str(text);
        }
        if !out.findings.is_empty() {
            let block = format_review_findings_block(&out.findings, None);
            findings_str.push_str(&format!("\n{block}"));
        }
        user_message.push_str(&format!(
            r#"<user_action>
  <context>User initiated a review task. Here's the full review output from reviewer model. User may select one or more comments to resolve.</context>
  <action>review</action>
  <results>
  {findings_str}
  </results>
</user_action>
"#));
    } else {
        user_message.push_str(r#"<user_action>
  <context>User initiated a review task, but was interrupted. If user asks about this, tell them to re-initiate a review with `/review` and wait for it to complete.</context>
  <action>review</action>
  <results>
  None.
  </results>
</user_action>
"#);
    }

    session
        .record_conversation_items(&[ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText { text: user_message }],
        }])
        .await;
    session
        .send_event(
            ctx.as_ref(),
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent { review_output }),
        )
        .await;
}

pub(crate) async fn emit_review_exit_on_abort(session: Arc<Session>, ctx: Arc<TurnContext>) {
    exit_review_mode(session, None, ctx).await;
}

/// Parse the review output; when not valid JSON, build a structured
/// fallback that carries the plain text as the overall explanation.
///
/// Returns: a ReviewOutputEvent parsed from JSON or a fallback populated from text.
fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
    // Try direct parse first
    if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
        return ev;
    }
    // If wrapped in markdown fences or extra prose, attempt to extract the first JSON object
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
    {
        return ev;
    }
    // Not JSON – return a structured ReviewOutputEvent that carries
    // the plain text as the overall explanation.
    ReviewOutputEvent {
        overall_explanation: text.to_string(),
        ..Default::default()
    }
}
