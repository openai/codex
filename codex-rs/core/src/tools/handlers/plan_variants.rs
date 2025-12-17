use async_trait::async_trait;
use codex_protocol::plan_mode::PlanOutputEvent;
use codex_protocol::plan_tool::UpdatePlanArgs;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

use crate::codex_delegate::run_codex_conversation_one_shot;
use crate::config::Config;
use crate::features::Feature;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;

pub(crate) const PROPOSE_PLAN_VARIANTS_TOOL_NAME: &str = "propose_plan_variants";

pub struct PlanVariantsHandler;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct ProposePlanVariantsArgs {
    goal: String,
}

const PLAN_VARIANT_PROMPT: &str = r#"You are a planning subagent producing a single plan variant for the user's goal.

Requirements:
- Do not ask the user questions.
- Do not propose or perform edits. Do not call apply_patch.
- Do not call propose_plan_variants.
- Prefer exploring the codebase using read-only commands (ripgrep, cat, ls).
- Output ONLY valid JSON matching this shape:
  { "title": string, "summary": string, "plan": { "explanation": string|null, "plan": [ { "step": string, "status": "pending"|"in_progress"|"completed" } ] } }
"#;

fn build_plan_variant_developer_instructions(existing: &str) -> String {
    let existing = existing.trim();
    if existing.is_empty() {
        return PLAN_VARIANT_PROMPT.to_string();
    }
    format!("{PLAN_VARIANT_PROMPT}\n\n{existing}")
}

#[async_trait]
impl ToolHandler for PlanVariantsHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            payload,
            tool_name,
            ..
        } = invocation;

        let source = turn.client.get_session_source();
        if let SessionSource::SubAgent(SubAgentSource::Other(label)) = &source
            && label.starts_with("plan_variant")
        {
            return Err(FunctionCallError::RespondToModel(
                "propose_plan_variants is not supported in plan-variant subagents".to_string(),
            ));
        }

        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported payload for {tool_name}"
            )));
        };

        let args: ProposePlanVariantsArgs = serde_json::from_str(&arguments).map_err(|e| {
            FunctionCallError::RespondToModel(format!("failed to parse function arguments: {e:?}"))
        })?;

        let goal = args.goal.trim();
        if goal.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "goal must be non-empty".to_string(),
            ));
        }

        const TOTAL: usize = 3;

        let mut join_set = JoinSet::new();
        for idx in 1..=TOTAL {
            let label = format!("plan_variant_{idx}");
            let base_config = turn.client.config().as_ref().clone();
            let goal = goal.to_string();
            let session = Arc::clone(&session);
            let turn = Arc::clone(&turn);
            join_set.spawn(async move {
                let started_at = Instant::now();

                session
                    .notify_background_event(
                        turn.as_ref(),
                        format!("Plan variants: generating {idx}/{TOTAL}…"),
                    )
                    .await;

                session
                    .notify_background_event(
                        turn.as_ref(),
                        format!("Plan variant {idx}/{TOTAL}: starting"),
                    )
                    .await;

                let out = run_one_variant(
                    base_config,
                    goal,
                    idx,
                    TOTAL,
                    label,
                    Arc::clone(&session),
                    Arc::clone(&turn),
                )
                .await;

                let elapsed = started_at.elapsed();
                session
                    .notify_background_event(
                        turn.as_ref(),
                        format!(
                            "Plan variants: finished {idx}/{TOTAL} ({})",
                            fmt_variant_duration(elapsed)
                        ),
                    )
                    .await;

                (idx, out)
            });
        }

        let mut variants_by_idx = vec![None; TOTAL];
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok((idx, out)) => {
                    if idx > 0 && idx <= TOTAL {
                        variants_by_idx[idx - 1] = Some(out);
                    }
                }
                Err(err) => {
                    return Err(FunctionCallError::RespondToModel(format!(
                        "failed to join planning subagent task: {err:?}"
                    )));
                }
            }
        }

        let variants = variants_by_idx
            .into_iter()
            .enumerate()
            .map(|(idx, out)| {
                out.unwrap_or_else(|| PlanOutputEvent {
                    title: format!("Variant {}", idx + 1),
                    summary: "Variant task did not return output.".to_string(),
                    plan: UpdatePlanArgs {
                        explanation: None,
                        plan: Vec::new(),
                    },
                })
            })
            .collect::<Vec<_>>();

        Ok(ToolOutput::Function {
            content: json!({ "variants": variants }).to_string(),
            content_items: None,
            success: Some(true),
        })
    }
}

fn fmt_variant_duration(elapsed: Duration) -> String {
    let secs = elapsed.as_secs_f64();
    if secs < 60.0 {
        return format!("{secs:.1}s");
    }

    let whole_secs = elapsed.as_secs();
    let minutes = whole_secs / 60;
    let seconds = whole_secs % 60;
    format!("{minutes}m {seconds:02}s")
}

fn activity_for_event(msg: &EventMsg) -> Option<String> {
    match msg {
        EventMsg::TaskStarted(_) => Some("starting".to_string()),
        EventMsg::UserMessage(_) => Some("sending prompt".to_string()),
        EventMsg::AgentReasoning(_)
        | EventMsg::AgentReasoningDelta(_)
        | EventMsg::AgentReasoningRawContent(_)
        | EventMsg::AgentReasoningRawContentDelta(_)
        | EventMsg::AgentReasoningSectionBreak(_) => Some("thinking".to_string()),
        EventMsg::AgentMessage(_) | EventMsg::AgentMessageDelta(_) => Some("writing".to_string()),
        EventMsg::ExecCommandBegin(ev) => Some(format!("shell {}", ev.command.join(" "))),
        EventMsg::McpToolCallBegin(ev) => Some(format!(
            "mcp {}/{}",
            ev.invocation.server.trim(),
            ev.invocation.tool.trim()
        )),
        EventMsg::WebSearchBegin(_) => Some("web_search".to_string()),
        _ => None,
    }
}

fn fmt_variant_tokens(tokens: i64) -> Option<String> {
    if tokens <= 0 {
        return None;
    }

    let tokens_f = tokens as f64;
    if tokens < 1_000 {
        return Some(format!("{tokens}"));
    }
    if tokens < 100_000 {
        return Some(format!("{:.1}k", tokens_f / 1_000.0));
    }
    if tokens < 1_000_000 {
        return Some(format!("{}k", tokens / 1_000));
    }
    if tokens < 100_000_000 {
        return Some(format!("{:.1}M", tokens_f / 1_000_000.0));
    }

    Some(format!("{}M", tokens / 1_000_000))
}

async fn run_one_variant(
    base_config: Config,
    goal: String,
    idx: usize,
    total: usize,
    label: String,
    parent_session: Arc<crate::codex::Session>,
    parent_ctx: Arc<crate::codex::TurnContext>,
) -> PlanOutputEvent {
    let mut cfg = base_config.clone();

    // Do not override the base/system prompt; some environments restrict it to whitelisted prompts.
    // Put plan-variant guidance in developer instructions instead.
    //
    // Also avoid inheriting large caller developer instructions (e.g. plan mode's own instructions)
    // into each variant, which can significantly increase token usage. Plan variants use a focused
    // prompt and return JSON only.
    cfg.developer_instructions = Some(build_plan_variant_developer_instructions(""));

    // Keep plan variants on the same model + reasoning settings as the parent turn.
    cfg.model = Some(parent_ctx.client.get_model());
    cfg.model_reasoning_effort = parent_ctx.client.get_reasoning_effort();
    cfg.model_reasoning_summary = parent_ctx.client.get_reasoning_summary();

    let mut features = cfg.features.clone();
    features
        .disable(Feature::ApplyPatchFreeform)
        .disable(Feature::WebSearchRequest)
        .disable(Feature::ViewImageTool);
    cfg.features = features;
    cfg.approval_policy = codex_protocol::protocol::AskForApproval::Never;
    cfg.sandbox_policy = codex_protocol::protocol::SandboxPolicy::ReadOnly;

    let input = vec![UserInput::Text {
        text: format!("Goal: {goal}\n\nReturn plan variant #{idx}."),
    }];

    let cancel = CancellationToken::new();
    let session_for_events = Arc::clone(&parent_session);
    let io = match run_codex_conversation_one_shot(
        cfg,
        Arc::clone(&parent_session.services.auth_manager),
        Arc::clone(&parent_session.services.models_manager),
        input,
        parent_session,
        Arc::clone(&parent_ctx),
        cancel,
        None,
        SubAgentSource::Other(label),
    )
    .await
    {
        Ok(io) => io,
        Err(err) => {
            return PlanOutputEvent {
                title: format!("Variant {idx}"),
                summary: format!("Failed to start subagent: {err}"),
                plan: UpdatePlanArgs {
                    explanation: None,
                    plan: Vec::new(),
                },
            };
        }
    };

    let mut last_agent_message: Option<String> = None;
    let mut last_activity: Option<String> = None;
    let mut last_reported_tokens: Option<i64> = None;
    let mut last_token_update_at: Option<Instant> = None;
    while let Ok(Event { msg, .. }) = io.rx_event.recv().await {
        if let EventMsg::TokenCount(ev) = &msg
            && let Some(info) = &ev.info
        {
            let tokens = info.total_token_usage.blended_total();
            let now = Instant::now();
            let should_report = match (last_reported_tokens, last_token_update_at) {
                (Some(prev), Some(prev_at)) => {
                    tokens > prev
                        && (tokens - prev >= 250 || now.duration_since(prev_at).as_secs() >= 2)
                }
                (Some(prev), None) => tokens > prev,
                (None, _) => tokens > 0,
            };

            if should_report && let Some(formatted) = fmt_variant_tokens(tokens) {
                session_for_events
                    .notify_background_event(
                        parent_ctx.as_ref(),
                        format!("Plan variant {idx}/{total}: tokens {formatted}"),
                    )
                    .await;
                last_reported_tokens = Some(tokens);
                last_token_update_at = Some(now);
            }
        }

        if let Some(activity) = activity_for_event(&msg)
            && last_activity.as_deref() != Some(activity.as_str())
        {
            session_for_events
                .notify_background_event(
                    parent_ctx.as_ref(),
                    format!("Plan variant {idx}/{total}: {activity}"),
                )
                .await;
            last_activity = Some(activity);
        }

        match msg {
            EventMsg::TaskComplete(ev) => {
                last_agent_message = ev.last_agent_message;
                break;
            }
            EventMsg::TurnAborted(_) => break,
            _ => {}
        }
    }

    let text = last_agent_message.unwrap_or_default();
    parse_plan_output_event(idx, text.as_str())
}

fn parse_plan_output_event(idx: usize, text: &str) -> PlanOutputEvent {
    if let Ok(ev) = serde_json::from_str::<PlanOutputEvent>(text) {
        return ev;
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<PlanOutputEvent>(slice)
    {
        return ev;
    }
    PlanOutputEvent {
        title: format!("Variant {idx}"),
        summary: "Subagent did not return valid JSON.".to_string(),
        plan: UpdatePlanArgs {
            explanation: Some(text.to_string()),
            plan: Vec::new(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plan_variants_do_not_override_base_instructions() {
        let codex_home = tempfile::TempDir::new().expect("tmp dir");
        let overrides = {
            #[cfg(target_os = "linux")]
            {
                use assert_cmd::cargo::cargo_bin;
                let mut overrides = crate::config::ConfigOverrides::default();
                overrides.codex_linux_sandbox_exe = Some(cargo_bin("codex-linux-sandbox"));
                overrides
            }
            #[cfg(not(target_os = "linux"))]
            {
                crate::config::ConfigOverrides::default()
            }
        };
        let mut cfg = crate::config::Config::load_from_base_config_with_overrides(
            crate::config::ConfigToml::default(),
            overrides,
            codex_home.path().to_path_buf(),
        )
        .expect("load test config");

        cfg.base_instructions = None;
        cfg.developer_instructions = Some("existing developer instructions".to_string());

        let existing_base = cfg.base_instructions.clone();
        let existing = cfg.developer_instructions.clone().unwrap_or_default();
        cfg.developer_instructions =
            Some(build_plan_variant_developer_instructions(existing.as_str()));

        assert_eq!(cfg.base_instructions, existing_base);
        assert!(
            cfg.developer_instructions
                .as_deref()
                .unwrap_or_default()
                .starts_with("You are a planning subagent")
        );
        assert!(
            cfg.developer_instructions
                .as_deref()
                .unwrap_or_default()
                .contains("existing developer instructions")
        );
    }
}
