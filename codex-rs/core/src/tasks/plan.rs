use async_trait::async_trait;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::plan_tool::StepStatus;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentMessageDeltaEvent;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExitedPlanModeEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::PlanOutputEvent;
use codex_protocol::protocol::PlanRequest;
use codex_protocol::protocol::SubAgentSource;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_conversation_one_shot;
use crate::state::TaskKind;
use codex_protocol::user_input::UserInput;
use std::sync::Arc;

use super::SessionTask;
use super::SessionTaskContext;

#[derive(Clone)]
pub(crate) struct PlanTask {
    request: PlanRequest,
}

impl PlanTask {
    pub(crate) fn new(request: PlanRequest) -> Self {
        Self { request }
    }
}

const PLAN_MODE_PROMPT: &str = r#"You are Codex in Plan Mode.

Goal: produce a clear, actionable plan for the user's request without making code changes.

Rules:
- You may explore the repo with read-only commands.
- Do not attempt to edit files or run mutating commands.
- You may ask the user clarifying questions via AskUserQuestion.
- Use `propose_plan_variants` to generate 3 alternative plans as input if helpful.
- When you have a final plan, call `approve_plan` with a concise title, summary, and step list.
- If the user requests revisions, incorporate feedback and propose a revised plan (you may call `propose_plan_variants` again).
- If the user rejects, stop.

When the plan is approved, your final assistant message MUST be ONLY valid JSON matching:
{ "title": string, "summary": string, "plan": { "explanation": string|null, "plan": [ { "step": string, "status": "pending"|"in_progress"|"completed" } ] } }
"#;

const PLAN_MODE_DEVELOPER_INSTRUCTIONS: &str = r#"## Plan Mode
You are planning only. Do not call `apply_patch` or execute mutating commands.

- To generate diverse approaches, call `propose_plan_variants` once you understand the goal.
- Present the final plan via `approve_plan`.
- After an `approve_plan` result:
  - Approved: output the final plan JSON as your only assistant message.
  - Revised: update the plan and call `approve_plan` again.
  - Rejected: stop; do not proceed.
"#;

#[async_trait]
impl SessionTask for PlanTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Plan
    }

    async fn run(
        self: Arc<Self>,
        session: Arc<SessionTaskContext>,
        ctx: Arc<TurnContext>,
        _input: Vec<UserInput>,
        cancellation_token: CancellationToken,
    ) -> Option<String> {
        let output = match start_plan_conversation(
            session.clone(),
            ctx.clone(),
            self.request.clone(),
            cancellation_token.clone(),
        )
        .await
        {
            Some(receiver) => process_plan_events(session.clone(), ctx.clone(), receiver).await,
            None => None,
        };

        if !cancellation_token.is_cancelled() {
            exit_plan_mode(session.clone_session(), output.clone(), ctx.clone()).await;
        }
        None
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        exit_plan_mode(session.clone_session(), None, ctx).await;
    }
}

async fn start_plan_conversation(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    request: PlanRequest,
    cancellation_token: CancellationToken,
) -> Option<async_channel::Receiver<Event>> {
    let config = ctx.client.config();
    let mut sub_agent_config = config.as_ref().clone();

    sub_agent_config.base_instructions = Some(PLAN_MODE_PROMPT.to_string());

    let ask = crate::tools::spec::prepend_ask_user_question_developer_instructions(None)
        .unwrap_or_default();
    sub_agent_config.developer_instructions =
        Some(format!("{PLAN_MODE_DEVELOPER_INSTRUCTIONS}\n{ask}"));

    sub_agent_config
        .features
        .disable(crate::features::Feature::ApplyPatchFreeform)
        .disable(crate::features::Feature::WebSearchRequest)
        .disable(crate::features::Feature::ViewImageTool);

    sub_agent_config.approval_policy = codex_protocol::protocol::AskForApproval::Never;
    sub_agent_config.sandbox_policy = codex_protocol::protocol::SandboxPolicy::ReadOnly;

    let input: Vec<UserInput> = vec![UserInput::Text {
        text: format!("User goal: {}", request.goal.trim()),
    }];

    run_codex_conversation_one_shot(
        sub_agent_config,
        session.auth_manager(),
        session.models_manager(),
        input,
        session.clone_session(),
        ctx,
        cancellation_token,
        None,
        SubAgentSource::Other("plan_mode".to_string()),
    )
    .await
    .ok()
    .map(|io| io.rx_event)
}

async fn process_plan_events(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    receiver: async_channel::Receiver<Event>,
) -> Option<PlanOutputEvent> {
    while let Ok(event) = receiver.recv().await {
        match event.clone().msg {
            // Suppress assistant text; plan mode surfaces via tool UIs and final output.
            EventMsg::AgentMessage(_)
            | EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::AgentMessage(_),
                ..
            })
            | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
            | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
            EventMsg::TaskComplete(task_complete) => {
                let out = task_complete
                    .last_agent_message
                    .as_deref()
                    .and_then(parse_plan_output_event);
                return out;
            }
            EventMsg::TurnAborted(_) => return None,
            other => {
                session
                    .clone_session()
                    .send_event(ctx.as_ref(), other)
                    .await;
            }
        }
    }
    None
}

fn parse_plan_output_event(text: &str) -> Option<PlanOutputEvent> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return None;
    }
    if let Ok(ev) = serde_json::from_str::<PlanOutputEvent>(trimmed) {
        return Some(ev);
    }
    if let (Some(start), Some(end)) = (trimmed.find('{'), trimmed.rfind('}'))
        && start < end
        && let Some(slice) = trimmed.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<PlanOutputEvent>(slice)
    {
        return Some(ev);
    }
    None
}

pub(crate) async fn exit_plan_mode(
    session: Arc<Session>,
    plan_output: Option<PlanOutputEvent>,
    ctx: Arc<TurnContext>,
) {
    const PLAN_USER_MESSAGE_ID: &str = "plan:rollout:user";
    const PLAN_ASSISTANT_MESSAGE_ID: &str = "plan:rollout:assistant";

    let (user_message, assistant_message) = if let Some(out) = plan_output.clone() {
        let mut body = String::new();
        let title = out.title.trim();
        body.push_str(&format!("Title: {title}\n"));
        let summary = out.summary.trim();
        if !summary.is_empty() {
            body.push_str(&format!("Summary: {summary}\n"));
        }
        body.push_str("Steps:\n");
        if out.plan.plan.is_empty() {
            body.push_str("- (no steps provided)\n");
        } else {
            for item in &out.plan.plan {
                let status = step_status_label(&item.status);
                let step = item.step.trim();
                body.push_str(&format!("- [{status}] {step}\n"));
            }
        }
        (
            "Plan approved.".to_string(),
            format!("Approved plan:\n{body}"),
        )
    } else {
        (
            "Plan ended without an approved plan.".to_string(),
            "Plan was rejected or interrupted.".to_string(),
        )
    };

    session
        .record_conversation_items(
            &ctx,
            &[ResponseItem::Message {
                id: Some(PLAN_USER_MESSAGE_ID.to_string()),
                role: "user".to_string(),
                content: vec![ContentItem::InputText { text: user_message }],
            }],
        )
        .await;
    session
        .send_event(
            ctx.as_ref(),
            EventMsg::ExitedPlanMode(ExitedPlanModeEvent { plan_output }),
        )
        .await;
    session
        .record_response_item_and_emit_turn_item(
            ctx.as_ref(),
            ResponseItem::Message {
                id: Some(PLAN_ASSISTANT_MESSAGE_ID.to_string()),
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: assistant_message,
                }],
            },
        )
        .await;
}

fn step_status_label(status: &StepStatus) -> &'static str {
    match status {
        StepStatus::Pending => "pending",
        StepStatus::InProgress => "in_progress",
        StepStatus::Completed => "completed",
    }
}
