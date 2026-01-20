use crate::agent::AgentStatus;
use crate::agent::status::is_final;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::review_format::format_review_findings_block;
use crate::review_format::render_review_output_text;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EnteredReviewModeEvent;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::ReviewRequest;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use std::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone)]
/// Task driver for review-mode turns.
pub(crate) struct ReviewTask {
    review_request: ReviewRequest,
    sub_agent_thread_id: Arc<Mutex<Option<ThreadId>>>,
}

impl ReviewTask {
    /// Creates a new review task with the already-resolved request details.
    pub(crate) fn new(review_request: ReviewRequest) -> Self {
        Self {
            review_request,
            sub_agent_thread_id: Arc::new(Mutex::new(None)),
        }
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

        // Build review agent config.
        let sub_agent_config = {
            let config = ctx.client.config();
            let mut sub_agent_config = config.as_ref().clone();
            sub_agent_config.web_search_mode = Some(WebSearchMode::Disabled);
            sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
            let model = config
                .review_model
                .clone()
                .unwrap_or_else(|| ctx.client.get_model());
            sub_agent_config.model = Some(model);
            sub_agent_config
        };

        // Spawn the review agent.
        let agent_control = session.agent_control();
        let thread_id = agent_control
            .spawn_agent(
                sub_agent_config,
                None,
                Some(SessionSource::SubAgent(SubAgentSource::Review)),
            )
            .await
            .ok()?;

        if let Ok(mut guard) = self.sub_agent_thread_id.lock() {
            *guard = Some(thread_id);
        }

        let mut status_rx = agent_control.subscribe_status(thread_id).await.ok()?;

        session
            .session
            .send_event(
                ctx.as_ref(),
                EventMsg::EnteredReviewMode(EnteredReviewModeEvent {
                    review_request: self.review_request.clone(),
                    review_thread_id: thread_id,
                }),
            )
            .await;

        // Send input to the agent.
        let output = match session.agent_control().send_input(thread_id, input).await {
            Ok(_) => {
                let mut status = status_rx.borrow().clone();
                let parse_output = |message: String| {
                    if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(&message) {
                        return ev;
                    }
                    if let (Some(start), Some(end)) = (message.find('{'), message.rfind('}'))
                        && start < end
                        && let Some(slice) = message.get(start..=end)
                        && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
                    {
                        return ev;
                    }
                    ReviewOutputEvent {
                        overall_explanation: message,
                        ..Default::default()
                    }
                };
                loop {
                    if is_final(&status) {
                        break match status {
                            AgentStatus::Completed(Some(message)) => Some(parse_output(message)),
                            _ => None,
                        };
                    }
                    tokio::select! {
                        _ = cancellation_token.cancelled() => break None,
                        changed = status_rx.changed() => {
                            if changed.is_err() {
                                break None;
                            }
                            status = status_rx.borrow().clone();
                        }
                    }
                }
            }
            Err(err) => {
                tracing::error!("Error while sending input to review agent {err:?}");
                None
            }
        };

        if !cancellation_token.is_cancelled() {
            exit_review_mode(
                session.clone_session(),
                output.clone(),
                ctx.clone(),
                Some(thread_id),
            )
            .await;
        }
        None
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        let thread_id = self
            .sub_agent_thread_id
            .lock()
            .ok()
            .and_then(|mut guard| guard.take());
        if let Some(thread_id) = thread_id {
            let _ = session.agent_control().interrupt_agent(thread_id).await;
        }
        exit_review_mode(session.clone_session(), None, ctx, thread_id).await;
    }
}

/// Emits an ExitedReviewMode Event with optional ReviewOutput, close the review
/// agent and records a developer message with the review output.
pub(crate) async fn exit_review_mode(
    session: Arc<Session>,
    review_output: Option<ReviewOutputEvent>,
    ctx: Arc<TurnContext>,
    review_thread_id: Option<ThreadId>,
) {
    // Close and drop the agent
    if let Some(thread_id) = review_thread_id
        && let Err(e) = session
            .services
            .agent_control
            .shutdown_agent(thread_id)
            .await
    {
        tracing::error!("Error while shutting down review agent: {e:?}");
    }

    // Build the message to add in the parent thread.
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
            },
        )
        .await;
}
