use std::sync::Arc;

use async_trait::async_trait;
use codex_async_utils::OrCancelExt;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentMessageDeltaEvent;
use codex_protocol::protocol::ApplyPatchApprovalRequestEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecApprovalRequestEvent;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RequestUserInputEvent;
use codex_protocol::protocol::ReviewDecision;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::request_user_input::RequestUserInputArgs;
use codex_protocol::request_user_input::RequestUserInputResponse;
use tokio_util::sync::CancellationToken;

use crate::codex::SUBMISSION_CHANNEL_CAPACITY;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::review_format::format_review_findings_block;
use crate::review_format::render_review_output_text;
use crate::state::TaskKind;
use codex_protocol::user_input::UserInput;
use std::collections::HashMap;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone, Copy)]
pub(crate) struct ReviewTask;

impl ReviewTask {
    pub(crate) fn new() -> Self {
        Self
    }
}

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
        let _ = session
            .session
            .services
            .otel_manager
            .counter("codex.task.review", 1, &[]);

        // Start sub-codex conversation and get the receiver for events.
        let output = match start_review_conversation(
            session.clone(),
            ctx.clone(),
            input,
            cancellation_token.clone(),
        )
        .await
        {
            Some(receiver) => process_review_events(session.clone(), ctx.clone(), receiver).await,
            None => None,
        };
        if !cancellation_token.is_cancelled() {
            exit_review_mode(session.clone_session(), output.clone(), ctx.clone()).await;
        }
        None
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        exit_review_mode(session.clone_session(), None, ctx).await;
    }
}

async fn start_review_conversation(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    input: Vec<UserInput>,
    cancellation_token: CancellationToken,
) -> Option<async_channel::Receiver<Event>> {
    let config = ctx.client.config();
    let mut sub_agent_config = config.as_ref().clone();
    // Carry over review-only feature restrictions so the review agent cannot
    // re-enable blocked tools (web search, view image).
    sub_agent_config.web_search_mode = Some(WebSearchMode::Disabled);

    // Set explicit review rubric for the sub-agent
    sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());

    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| ctx.client.get_model());
    sub_agent_config.model = Some(model);
    let agent_control = session.session.services.agent_control.clone();
    let agent_id = match agent_control
        .spawn_agent_with_options(
            sub_agent_config,
            None,
            Some(SessionSource::SubAgent(SubAgentSource::Review)),
            false,
        )
        .await
    {
        Ok(agent_id) => agent_id,
        Err(_) => return None,
    };

    if agent_control.send_input(agent_id, input).await.is_err() {
        let _ = agent_control.shutdown_agent(agent_id).await;
        return None;
    }

    let (tx_sub, rx_sub) = async_channel::bounded(SUBMISSION_CHANNEL_CAPACITY);
    let parent_session = session.clone_session();
    let parent_ctx = ctx.clone();
    let cancel_token = cancellation_token.child_token();
    tokio::spawn(async move {
        forward_review_events(
            agent_control,
            agent_id,
            tx_sub,
            parent_session,
            parent_ctx,
            cancel_token,
        )
        .await;
    });

    Some(rx_sub)
}

async fn forward_review_events(
    agent_control: crate::agent::AgentControl,
    agent_id: ThreadId,
    tx_sub: async_channel::Sender<Event>,
    parent_session: Arc<Session>,
    parent_ctx: Arc<TurnContext>,
    cancellation_token: CancellationToken,
) {
    let cancelled = cancellation_token.cancelled();
    tokio::pin!(cancelled);

    loop {
        tokio::select! {
            _ = &mut cancelled => {
                shutdown_review_agent(&agent_control, agent_id).await;
                break;
            }
            event = agent_control.next_event(agent_id) => {
                let event = match event {
                    Ok(event) => event,
                    Err(_) => {
                        shutdown_review_agent(&agent_control, agent_id).await;
                        break;
                    }
                };

                match event.msg.clone() {
                    EventMsg::AgentMessageDelta(_)
                    | EventMsg::AgentReasoningDelta(_)
                    | EventMsg::TokenCount(_)
                    | EventMsg::SessionConfigured(_)
                    | EventMsg::ThreadNameUpdated(_) => {}
                    EventMsg::ExecApprovalRequest(request) => {
                        if handle_exec_approval(
                            &agent_control,
                            agent_id,
                            event.id,
                            &parent_session,
                            &parent_ctx,
                            request,
                            &cancellation_token,
                        )
                        .await
                        .is_err()
                        {
                            shutdown_review_agent(&agent_control, agent_id).await;
                            break;
                        }
                    }
                    EventMsg::ApplyPatchApprovalRequest(request) => {
                        if handle_patch_approval(
                            &agent_control,
                            agent_id,
                            event.id,
                            &parent_session,
                            &parent_ctx,
                            request,
                            &cancellation_token,
                        )
                        .await
                        .is_err()
                        {
                            shutdown_review_agent(&agent_control, agent_id).await;
                            break;
                        }
                    }
                    EventMsg::RequestUserInput(request) => {
                        if handle_request_user_input(
                            &agent_control,
                            agent_id,
                            event.id,
                            &parent_session,
                            &parent_ctx,
                            request,
                            &cancellation_token,
                        )
                        .await
                        .is_err()
                        {
                            shutdown_review_agent(&agent_control, agent_id).await;
                            break;
                        }
                    }
                    EventMsg::TurnComplete(_) | EventMsg::TurnAborted(_) => {
                        if !send_review_event(&tx_sub, event, &cancellation_token).await {
                            shutdown_review_agent(&agent_control, agent_id).await;
                            break;
                        }
                        shutdown_review_agent(&agent_control, agent_id).await;
                        break;
                    }
                    _ => {
                        if !send_review_event(&tx_sub, event, &cancellation_token).await {
                            shutdown_review_agent(&agent_control, agent_id).await;
                            break;
                        }
                    }
                }
            }
        }
    }
}

async fn send_review_event(
    tx_sub: &async_channel::Sender<Event>,
    event: Event,
    cancellation_token: &CancellationToken,
) -> bool {
    matches!(
        tx_sub.send(event).or_cancel(cancellation_token).await,
        Ok(Ok(()))
    )
}

async fn shutdown_review_agent(agent_control: &crate::agent::AgentControl, agent_id: ThreadId) {
    let _ = agent_control.shutdown_agent(agent_id).await;
}

async fn handle_exec_approval(
    agent_control: &crate::agent::AgentControl,
    agent_id: ThreadId,
    id: String,
    parent_session: &Session,
    parent_ctx: &TurnContext,
    event: ExecApprovalRequestEvent,
    cancellation_token: &CancellationToken,
) -> crate::error::Result<()> {
    let approval_fut = parent_session.request_command_approval(
        parent_ctx,
        parent_ctx.sub_id.clone(),
        event.command,
        event.cwd,
        event.reason,
        event.proposed_execpolicy_amendment,
    );
    let decision = await_approval_with_cancel(
        approval_fut,
        parent_session,
        &parent_ctx.sub_id,
        cancellation_token,
    )
    .await;
    agent_control
        .send_op(agent_id, Op::ExecApproval { id, decision })
        .await
        .map(|_| ())
}

async fn handle_patch_approval(
    agent_control: &crate::agent::AgentControl,
    agent_id: ThreadId,
    id: String,
    parent_session: &Session,
    parent_ctx: &TurnContext,
    event: ApplyPatchApprovalRequestEvent,
    cancellation_token: &CancellationToken,
) -> crate::error::Result<()> {
    let decision_rx = parent_session
        .request_patch_approval(
            parent_ctx,
            parent_ctx.sub_id.clone(),
            event.changes,
            event.reason,
            event.grant_root,
        )
        .await;
    let decision = await_approval_with_cancel(
        async move { decision_rx.await.unwrap_or_default() },
        parent_session,
        &parent_ctx.sub_id,
        cancellation_token,
    )
    .await;
    agent_control
        .send_op(agent_id, Op::PatchApproval { id, decision })
        .await
        .map(|_| ())
}

async fn handle_request_user_input(
    agent_control: &crate::agent::AgentControl,
    agent_id: ThreadId,
    id: String,
    parent_session: &Session,
    parent_ctx: &TurnContext,
    event: RequestUserInputEvent,
    cancellation_token: &CancellationToken,
) -> crate::error::Result<()> {
    let args = RequestUserInputArgs {
        questions: event.questions,
    };
    let response_fut =
        parent_session.request_user_input(parent_ctx, parent_ctx.sub_id.clone(), args);
    let response = await_user_input_with_cancel(
        response_fut,
        parent_session,
        &parent_ctx.sub_id,
        cancellation_token,
    )
    .await;
    agent_control
        .send_op(agent_id, Op::UserInputAnswer { id, response })
        .await
        .map(|_| ())
}

async fn await_user_input_with_cancel<F>(
    fut: F,
    parent_session: &Session,
    sub_id: &str,
    cancellation_token: &CancellationToken,
) -> RequestUserInputResponse
where
    F: core::future::Future<Output = Option<RequestUserInputResponse>>,
{
    tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => {
            let empty = RequestUserInputResponse {
                answers: HashMap::new(),
            };
            parent_session
                .notify_user_input_response(sub_id, empty.clone())
                .await;
            empty
        }
        response = fut => response.unwrap_or_else(|| RequestUserInputResponse {
            answers: HashMap::new(),
        }),
    }
}

async fn await_approval_with_cancel<F>(
    fut: F,
    parent_session: &Session,
    sub_id: &str,
    cancellation_token: &CancellationToken,
) -> ReviewDecision
where
    F: core::future::Future<Output = ReviewDecision>,
{
    tokio::select! {
        biased;
        _ = cancellation_token.cancelled() => {
            parent_session.notify_approval(sub_id, ReviewDecision::Abort).await;
            ReviewDecision::Abort
        }
        decision = fut => {
            decision
        }
    }
}

async fn process_review_events(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    receiver: async_channel::Receiver<Event>,
) -> Option<ReviewOutputEvent> {
    let mut prev_agent_message: Option<Event> = None;
    while let Ok(event) = receiver.recv().await {
        match event.clone().msg {
            EventMsg::AgentMessage(_) => {
                if let Some(prev) = prev_agent_message.take() {
                    session
                        .clone_session()
                        .send_event(ctx.as_ref(), prev.msg)
                        .await;
                }
                prev_agent_message = Some(event);
            }
            // Suppress ItemCompleted only for assistant messages: forwarding it
            // would trigger legacy AgentMessage via as_legacy_events(), which this
            // review flow intentionally hides in favor of structured output.
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::AgentMessage(_),
                ..
            })
            | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
            | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
            EventMsg::TurnComplete(task_complete) => {
                // Parse review output from the last agent message (if present).
                let out = task_complete
                    .last_agent_message
                    .as_deref()
                    .map(parse_review_output_event);
                return out;
            }
            EventMsg::TurnAborted(_) => {
                // Cancellation or abort: consumer will finalize with None.
                return None;
            }
            other => {
                session
                    .clone_session()
                    .send_event(ctx.as_ref(), other)
                    .await;
            }
        }
    }
    // Channel closed without TurnComplete: treat as interrupted.
    None
}

/// Parse a ReviewOutputEvent from a text blob returned by the reviewer model.
/// If the text is valid JSON matching ReviewOutputEvent, deserialize it.
/// Otherwise, attempt to extract the first JSON object substring and parse it.
/// If parsing still fails, return a structured fallback carrying the plain text
/// in `overall_explanation`.
fn parse_review_output_event(text: &str) -> ReviewOutputEvent {
    if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
        return ev;
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
    {
        return ev;
    }
    ReviewOutputEvent {
        overall_explanation: text.to_string(),
        ..Default::default()
    }
}

/// Emits an ExitedReviewMode Event with optional ReviewOutput,
/// and records a developer message with the review output.
pub(crate) async fn exit_review_mode(
    session: Arc<Session>,
    review_output: Option<ReviewOutputEvent>,
    ctx: Arc<TurnContext>,
) {
    const REVIEW_USER_MESSAGE_ID: &str = "review_rollout_user";
    const REVIEW_ASSISTANT_MESSAGE_ID: &str = "review_rollout_assistant";
    let (user_message, assistant_message) = if let Some(out) = review_output.clone() {
        let mut findings_str = String::new();
        let text = out.overall_explanation.trim();
        if !text.is_empty() {
            findings_str.push_str(text);
        }
        if !out.findings.is_empty() {
            let block = format_review_findings_block(&out.findings, None);
            findings_str.push_str(&format!("\n{block}"));
        }
        let rendered =
            crate::client_common::REVIEW_EXIT_SUCCESS_TMPL.replace("{results}", &findings_str);
        let assistant_message = render_review_output_text(&out);
        (rendered, assistant_message)
    } else {
        let rendered = crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL.to_string();
        let assistant_message =
            "Review was interrupted. Please re-run /review and wait for it to complete."
                .to_string();
        (rendered, assistant_message)
    };

    session
        .record_conversation_items(
            &ctx,
            &[ResponseItem::Message {
                id: Some(REVIEW_USER_MESSAGE_ID.to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: user_message }],
                end_turn: None,
                phase: None,
            }],
        )
        .await;
    session
        .send_event(
            ctx.as_ref(),
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent { review_output }),
        )
        .await;
    session
        .record_response_item_and_emit_turn_item(
            ctx.as_ref(),
            ResponseItem::Message {
                id: Some(REVIEW_ASSISTANT_MESSAGE_ID.to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: assistant_message,
                }],
                end_turn: None,
                phase: None,
            },
        )
        .await;
}
