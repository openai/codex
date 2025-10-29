use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ReviewOutputEvent;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_conversation_one_shot;
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
    // Run with only reviewer rubric — drop outer user_instructions
    sub_agent_config.user_instructions = None;
    // Avoid loading project docs; reviewer only needs findings
    sub_agent_config.project_doc_max_bytes = 0;
    // Carry over review-only feature restrictions so the delegate cannot
    // re-enable blocked tools (web search, view image, streamable shell).
    sub_agent_config
        .features
        .disable(crate::features::Feature::WebSearchRequest)
        .disable(crate::features::Feature::ViewImageTool)
        .disable(crate::features::Feature::StreamableShell);
    // Set explicit review rubric for the sub-agent
    sub_agent_config.base_instructions = Some(crate::REVIEW_PROMPT.to_string());
    (run_codex_conversation_one_shot(
        sub_agent_config,
        session.auth_manager(),
        input,
        session.clone_session(),
        ctx.clone(),
        cancellation_token,
    )
    .await)
        .ok()
        .map(|io| io.rx_event)
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
            EventMsg::TaskComplete(task_complete) => {
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
    // Channel closed without TaskComplete: treat as interrupted.
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
    let user_message = if let Some(out) = review_output.clone() {
        let mut findings_str = String::new();
        let text = out.overall_explanation.trim();
        if !text.is_empty() {
            findings_str.push_str(text);
        }
        if !out.findings.is_empty() {
            let block = format_review_findings_block(&out.findings, None);
            findings_str.push_str(&format!("\n{block}"));
        }
        crate::client_common::REVIEW_EXIT_SUCCESS_TMPL.replace("{results}", &findings_str)
    } else {
        crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL.to_string()
    };

    session
        .record_conversation_items(
            &ctx,
            &[ResponseItem::Message {
                id: None,
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
}
