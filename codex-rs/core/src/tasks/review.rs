use crate::agent::AgentStatus;
use crate::agent::status::is_final;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::review_format::format_review_findings_block;
use crate::review_format::render_review_output_text;
use crate::state::TaskKind;
use async_trait::async_trait;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

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

        // Start sub-codex conversation and await its final status.
        let output = match start_review_conversation(session.clone(), ctx.clone(), input).await {
            Some(sub_agent) => {
                wait_for_review_output(sub_agent.status_rx, cancellation_token.clone()).await
            }
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
) -> Option<ReviewSubAgent> {
    let config = ctx.client.config();
    let mut sub_agent_config = config.as_ref().clone();
    // Carry over review-only feature restrictions so the sub-agent cannot
    // re-enable blocked tools (web search, view image).
    sub_agent_config.web_search_mode = Some(WebSearchMode::Disabled);

    // Set explicit review rubric for the sub-agent
    sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());

    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| ctx.client.get_model());
    sub_agent_config.model = Some(model);

    let agent_control = session.agent_control();
    let thread_id = agent_control
        .spawn_agent(
            sub_agent_config,
            None,
            Some(SessionSource::SubAgent(SubAgentSource::Review)),
        )
        .await
        .ok()?;

    agent_control.send_input(thread_id, input).await.ok()?;

    let status_rx = agent_control.subscribe_status(thread_id).await.ok()?;
    Some(ReviewSubAgent { status_rx })
}

struct ReviewSubAgent {
    status_rx: watch::Receiver<AgentStatus>,
}

async fn wait_for_review_output(
    mut status_rx: watch::Receiver<AgentStatus>,
    cancellation_token: CancellationToken,
) -> Option<ReviewOutputEvent> {
    let mut status = status_rx.borrow().clone();
    while !is_final(&status) {
        tokio::select! {
            _ = cancellation_token.cancelled() => return None,
            changed = status_rx.changed() => {
                if changed.is_err() {
                    break;
                }
                status = status_rx.borrow().clone();
            }
        }
    }
    review_output_from_status(status)
}

fn review_output_from_status(status: AgentStatus) -> Option<ReviewOutputEvent> {
    match status {
        AgentStatus::Completed(Some(message)) => Some(parse_review_output_event(&message)),
        _ => None,
    }
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
    const REVIEW_USER_MESSAGE_ID: &str = "review:rollout:user";
    const REVIEW_ASSISTANT_MESSAGE_ID: &str = "review:rollout:assistant";
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
