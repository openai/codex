use std::sync::Arc;

use async_trait::async_trait;
use codex_protocol::config_types::ModeKind;
use codex_protocol::config_types::WebSearchMode;
use codex_protocol::items::TurnItem;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::permissions::FileSystemAccessMode;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::AgentMessageContentDeltaEvent;
use codex_protocol::protocol::AgentMessageDeltaEvent;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandStatus;
use codex_protocol::protocol::ExitedReviewModeEvent;
use codex_protocol::protocol::ItemCompletedEvent;
use codex_protocol::protocol::ReadOnlyAccess;
use codex_protocol::protocol::ReviewFinding;
use codex_protocol::protocol::ReviewOutputEvent;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_one_shot;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::types::WindowsSandboxModeToml;
use crate::features::Feature;
use crate::review_format::format_review_findings_block;
use crate::review_format::render_review_output_text;
use crate::review_prompts::ResolvedReviewRequest;
use crate::state::TaskKind;

use super::SessionTask;
use super::SessionTaskContext;

pub(crate) const REVIEW_FINDING_VALIDATOR_SUBAGENT: &str = "review_finding_validator";
const REVIEW_FINDING_VALIDATION_FAILURE_EXPLANATION: &str =
    "Review findings validation did not complete cleanly, so no findings were surfaced.";
const REVIEW_MALFORMED_OUTPUT_FAILURE_EXPLANATION: &str =
    "Reviewer did not return valid structured output, so the review did not complete cleanly.";
const REVIEW_UNEXPECTED_VALIDATED_FINDINGS_FAILURE_EXPLANATION: &str = "Review findings validation returned findings that were not present in the original review, so no findings were surfaced.";
const REVIEW_CORRECT_VERDICT: &str = "patch is correct";
const REVIEW_INCORRECT_VERDICT: &str = "patch is incorrect";
const REVIEW_FINDING_VALIDATION_PROMPT: &str = "You are validating an existing code review against the current codebase.\n\
Do not perform a fresh review.\n\
Do not introduce new findings.\n\
Only keep, discard, or clarify findings that already exist in the provided review output.\n\
Use repository inspection as needed to verify the existing findings.\n\
When you finish, return only strict JSON matching the provided schema.";
const REVIEW_SUBAGENT_REPO_GUIDANCE_PROMPT: &str = "Follow repository-specific guidance from inherited developer/user instructions when it affects review scope, diff selection, generated-file handling, or validation criteria.\n\
Ignore workflow instructions unrelated to producing the review result, including notifications, commits, PR creation, or file edits.";
const REVIEW_INTERRUPTED_MESSAGE: &str =
    "Review was interrupted. Please re-run /review and wait for it to complete.";

enum ReviewExit {
    Completed(ReviewOutputEvent),
    Failed(String),
    Interrupted,
}

#[derive(Debug, PartialEq)]
enum ParsedReviewOutput {
    Structured(ReviewOutputEvent),
    Fallback(ReviewOutputEvent),
}

#[derive(Clone)]
pub(crate) struct ReviewTask {
    validate_findings: bool,
    resolved_review_request: ResolvedReviewRequest,
}

impl ReviewTask {
    pub(crate) fn new(
        validate_findings: bool,
        resolved_review_request: ResolvedReviewRequest,
    ) -> Self {
        Self {
            validate_findings,
            resolved_review_request,
        }
    }
}

#[async_trait]
impl SessionTask for ReviewTask {
    fn kind(&self) -> TaskKind {
        TaskKind::Review
    }

    fn span_name(&self) -> &'static str {
        "session_task.review"
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
            .session_telemetry
            .counter("codex.task.review", 1, &[]);

        // Start sub-codex conversation and get the receiver for events.
        let review_exit = match start_review_conversation(
            session.clone(),
            ctx.clone(),
            input,
            self.validate_findings,
            cancellation_token.clone(),
        )
        .await
        {
            Some(receiver) => {
                maybe_validate_review_output(
                    session.clone(),
                    ctx.clone(),
                    process_review_events(
                        session.clone(),
                        ctx.clone(),
                        receiver,
                        !self.validate_findings,
                        !self.validate_findings,
                    )
                    .await,
                    self.validate_findings,
                    &self.resolved_review_request,
                    cancellation_token.clone(),
                )
                .await
            }
            None => ReviewExit::Interrupted,
        };
        if !cancellation_token.is_cancelled() {
            exit_review_mode(session.clone_session(), review_exit, ctx.clone()).await;
        }
        None
    }

    async fn abort(&self, session: Arc<SessionTaskContext>, ctx: Arc<TurnContext>) {
        exit_review_mode(session.clone_session(), ReviewExit::Interrupted, ctx).await;
    }
}

async fn start_review_conversation(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    input: Vec<UserInput>,
    validate_findings: bool,
    cancellation_token: CancellationToken,
) -> Option<async_channel::Receiver<Event>> {
    let config = ctx.config.clone();
    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| ctx.model_info.slug.clone());
    let (sub_agent_config, _) = build_review_sub_agent_config(
        config.as_ref(),
        &model,
        crate::REVIEW_PROMPT,
        false,
        validate_findings,
    );
    let final_output_json_schema = if validate_findings {
        Some(review_output_schema())
    } else {
        None
    };
    (run_codex_thread_one_shot(
        sub_agent_config,
        session.auth_manager(),
        session.models_manager(),
        input,
        session.clone_session(),
        ctx.clone(),
        cancellation_token,
        SubAgentSource::Review,
        final_output_json_schema,
        None,
    )
    .await)
        .ok()
        .map(|io| io.rx_event)
}

fn build_review_sub_agent_config(
    parent_config: &Config,
    model: &str,
    base_instructions: &str,
    default_mode_request_user_input: bool,
    force_read_only_sandbox: bool,
) -> (Config, bool) {
    let mut sub_agent_config = parent_config.clone();
    // Carry over review-only feature restrictions so the delegate cannot
    // re-enable blocked tools (web search, collab tools, view image).
    if let Err(err) = sub_agent_config
        .web_search_mode
        .set(WebSearchMode::Disabled)
    {
        panic!("by construction Constrained<WebSearchMode> must always support Disabled: {err}");
    }
    let _ = sub_agent_config.features.disable(Feature::Collab);
    let request_user_input_available = if default_mode_request_user_input {
        let _ = sub_agent_config
            .features
            .enable(Feature::DefaultModeRequestUserInput);
        ModeKind::Default.allows_request_user_input()
            || sub_agent_config
                .features
                .enabled(Feature::DefaultModeRequestUserInput)
    } else {
        false
    };
    if force_read_only_sandbox {
        let parent_sandbox_policy = parent_config.permissions.sandbox_policy.get();
        let read_only_access = if parent_sandbox_policy.has_full_disk_read_access() {
            ReadOnlyAccess::FullAccess
        } else {
            ReadOnlyAccess::Restricted {
                include_platform_defaults: parent_sandbox_policy.include_platform_defaults(),
                readable_roots: parent_sandbox_policy
                    .get_readable_roots_with_cwd(parent_config.cwd.as_path()),
            }
        };
        let read_only_policy = SandboxPolicy::ReadOnly {
            access: read_only_access,
            network_access: false,
        };
        sub_agent_config.permissions.sandbox_policy =
            Constrained::allow_only(read_only_policy.clone());
        sub_agent_config.permissions.file_system_sandbox_policy =
            match parent_config.permissions.file_system_sandbox_policy.kind {
                FileSystemSandboxKind::Restricted => FileSystemSandboxPolicy::restricted(
                    parent_config
                        .permissions
                        .file_system_sandbox_policy
                        .entries
                        .iter()
                        .cloned()
                        .map(|mut entry| {
                            entry.access = if entry.access.can_read() {
                                FileSystemAccessMode::Read
                            } else {
                                FileSystemAccessMode::None
                            };
                            entry
                        })
                        .collect(),
                ),
                FileSystemSandboxKind::Unrestricted | FileSystemSandboxKind::ExternalSandbox => {
                    FileSystemSandboxPolicy::from(&read_only_policy)
                }
            };
        sub_agent_config.permissions.network_sandbox_policy =
            NetworkSandboxPolicy::from(&read_only_policy);
        sub_agent_config.permissions.network = None;
        sub_agent_config.permissions.windows_sandbox_mode =
            Some(WindowsSandboxModeToml::Unelevated);
    }
    sub_agent_config.base_instructions = Some(format!(
        "{base_instructions}\n\n{REVIEW_SUBAGENT_REPO_GUIDANCE_PROMPT}"
    ));
    sub_agent_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    sub_agent_config.model = Some(model.to_string());
    (sub_agent_config, request_user_input_available)
}

fn should_forward_hidden_review_diagnostic(msg: &EventMsg) -> bool {
    match msg {
        EventMsg::Error(_) | EventMsg::Warning(_) | EventMsg::StreamError(_) => true,
        EventMsg::ExecCommandEnd(event) => !matches!(event.status, ExecCommandStatus::Completed),
        EventMsg::PatchApplyEnd(event) => !event.success,
        EventMsg::McpToolCallEnd(event) => !event.is_success(),
        EventMsg::DynamicToolCallResponse(event) => !event.success,
        _ => false,
    }
}

fn remap_hidden_review_diagnostic_to_parent_turn(msg: EventMsg, parent_turn_id: &str) -> EventMsg {
    match msg {
        EventMsg::ExecCommandEnd(mut event) => {
            event.turn_id = parent_turn_id.to_string();
            EventMsg::ExecCommandEnd(event)
        }
        EventMsg::PatchApplyEnd(mut event) => {
            event.turn_id = parent_turn_id.to_string();
            EventMsg::PatchApplyEnd(event)
        }
        EventMsg::DynamicToolCallResponse(mut event) => {
            event.turn_id = parent_turn_id.to_string();
            EventMsg::DynamicToolCallResponse(event)
        }
        other => other,
    }
}

async fn process_review_events(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    receiver: async_channel::Receiver<Event>,
    forward_intermediate_agent_messages: bool,
    allow_wrapped_json: bool,
) -> Option<ParsedReviewOutput> {
    let mut prev_agent_message: Option<Event> = None;
    while let Ok(event) = receiver.recv().await {
        match event.clone().msg {
            EventMsg::AgentMessage(_) => {
                if forward_intermediate_agent_messages && let Some(prev) = prev_agent_message.take()
                {
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
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::Reasoning(_),
                ..
            })
            | EventMsg::AgentReasoning(_)
            | EventMsg::AgentReasoningDelta(_)
            | EventMsg::AgentReasoningRawContent(_)
            | EventMsg::AgentReasoningRawContentDelta(_)
            | EventMsg::AgentReasoningSectionBreak(_)
            | EventMsg::RawResponseItem(_)
            | EventMsg::ReasoningContentDelta(_)
            | EventMsg::ReasoningRawContentDelta(_)
                if !forward_intermediate_agent_messages => {}
            EventMsg::TurnComplete(task_complete) => {
                // Parse review output from the last agent message (if present).
                let out = task_complete
                    .last_agent_message
                    .as_deref()
                    .map(|text| parse_review_output_event(text, allow_wrapped_json));
                return out;
            }
            EventMsg::TurnAborted(_) => {
                // Cancellation or abort: consumer will finalize with None.
                return None;
            }
            _ if !forward_intermediate_agent_messages
                && should_forward_hidden_review_diagnostic(&event.msg) =>
            {
                session
                    .clone_session()
                    .send_event(
                        ctx.as_ref(),
                        remap_hidden_review_diagnostic_to_parent_turn(event.msg, &ctx.sub_id),
                    )
                    .await;
            }
            _ if !forward_intermediate_agent_messages => {}
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

async fn maybe_validate_review_output(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    review_output: Option<ParsedReviewOutput>,
    validate_findings: bool,
    resolved_review_request: &ResolvedReviewRequest,
    cancellation_token: CancellationToken,
) -> ReviewExit {
    let Some(review_output) = review_output else {
        return ReviewExit::Interrupted;
    };
    let review_output = match review_output {
        ParsedReviewOutput::Structured(review_output) => review_output,
        ParsedReviewOutput::Fallback(review_output) => {
            if validate_findings {
                return ReviewExit::Failed(REVIEW_MALFORMED_OUTPUT_FAILURE_EXPLANATION.to_string());
            }
            review_output
        }
    };
    if !validate_findings {
        return ReviewExit::Completed(review_output);
    }
    if review_output.findings.is_empty() {
        return ReviewExit::Completed(normalize_validated_review_output(review_output));
    }

    match run_review_validation_subagent(
        session,
        ctx,
        review_output.clone(),
        resolved_review_request,
        cancellation_token.clone(),
    )
    .await
    {
        Ok(Some(validated_output)) => match ensure_validated_findings_match_original(
            &review_output.findings,
            validated_output,
        ) {
            Ok(validated_output) => {
                ReviewExit::Completed(normalize_validated_review_output(validated_output))
            }
            Err(message) => ReviewExit::Failed(message),
        },
        Ok(None) if cancellation_token.is_cancelled() => ReviewExit::Interrupted,
        Ok(None) => ReviewExit::Failed(REVIEW_FINDING_VALIDATION_FAILURE_EXPLANATION.to_string()),
        Err(message) => ReviewExit::Failed(message),
    }
}

async fn run_review_validation_subagent(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    review_output: ReviewOutputEvent,
    resolved_review_request: &ResolvedReviewRequest,
    cancellation_token: CancellationToken,
) -> Result<Option<ReviewOutputEvent>, String> {
    let config = ctx.config.clone();
    let model = config
        .review_model
        .clone()
        .unwrap_or_else(|| ctx.model_info.slug.clone());
    let (sub_agent_config, request_user_input_available) = build_review_sub_agent_config(
        config.as_ref(),
        &model,
        REVIEW_FINDING_VALIDATION_PROMPT,
        true,
        true,
    );
    let prompt = review_validation_prompt(
        resolved_review_request,
        &review_output,
        request_user_input_available,
    );
    let receiver = (run_codex_thread_one_shot(
        sub_agent_config,
        session.auth_manager(),
        session.models_manager(),
        vec![UserInput::Text {
            text: prompt,
            text_elements: Vec::new(),
        }],
        session.clone_session(),
        ctx.clone(),
        cancellation_token,
        SubAgentSource::Other(REVIEW_FINDING_VALIDATOR_SUBAGENT.to_string()),
        Some(review_output_schema()),
        None,
    )
    .await)
        .ok()
        .map(|io| io.rx_event)
        .ok_or_else(|| REVIEW_FINDING_VALIDATION_FAILURE_EXPLANATION.to_string())?;
    Ok(process_review_validation_events(session, ctx, receiver).await)
}

async fn process_review_validation_events(
    session: Arc<SessionTaskContext>,
    ctx: Arc<TurnContext>,
    receiver: async_channel::Receiver<Event>,
) -> Option<ReviewOutputEvent> {
    while let Ok(event) = receiver.recv().await {
        match event.clone().msg {
            EventMsg::TurnComplete(task_complete) => {
                return task_complete
                    .last_agent_message
                    .as_deref()
                    .and_then(|text| serde_json::from_str::<ReviewOutputEvent>(text).ok());
            }
            EventMsg::TurnAborted(_) => return None,
            EventMsg::ItemCompleted(ItemCompletedEvent {
                item: TurnItem::AgentMessage(_),
                ..
            })
            | EventMsg::AgentMessage(_)
            | EventMsg::AgentMessageDelta(AgentMessageDeltaEvent { .. })
            | EventMsg::AgentMessageContentDelta(AgentMessageContentDeltaEvent { .. }) => {}
            _ if should_forward_hidden_review_diagnostic(&event.msg) => {
                session
                    .clone_session()
                    .send_event(
                        ctx.as_ref(),
                        remap_hidden_review_diagnostic_to_parent_turn(event.msg, &ctx.sub_id),
                    )
                    .await;
            }
            _ => {}
        }
    }
    None
}

/// Parse a ReviewOutputEvent from a text blob returned by the reviewer model.
/// If the text is valid JSON matching ReviewOutputEvent, deserialize it.
/// If allowed, attempt to extract the first JSON object substring and parse it.
/// If parsing still fails, return a structured fallback carrying the plain text
/// in `overall_explanation`.
fn parse_review_output_event(text: &str, allow_wrapped_json: bool) -> ParsedReviewOutput {
    if let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(text) {
        return ParsedReviewOutput::Structured(ev);
    }
    if allow_wrapped_json
        && let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
        && let Ok(ev) = serde_json::from_str::<ReviewOutputEvent>(slice)
    {
        return ParsedReviewOutput::Structured(ev);
    }
    ParsedReviewOutput::Fallback(ReviewOutputEvent {
        overall_explanation: text.to_string(),
        ..Default::default()
    })
}

fn review_validation_prompt(
    resolved_review_request: &ResolvedReviewRequest,
    review_output: &ReviewOutputEvent,
    request_user_input_available: bool,
) -> String {
    let review_context_json = serde_json::json!({
        "original_review_target_summary": resolved_review_request.user_facing_hint.trim(),
        "original_review_scope": resolved_review_request.prompt.trim(),
        "original_review_output": review_output,
    });
    let review_context_json = serde_json::to_string_pretty(&review_context_json)
        .or_else(|_| serde_json::to_string(&review_context_json))
        .unwrap_or_else(|_| render_review_output_text(review_output));
    let clarification_instruction = if request_user_input_available {
        "If you are unsure whether a finding is valid or need clarification from the user before deciding, call `request_user_input`, wait for the answer, and then continue.\n\
Do not keep uncertain findings unless you validated them or resolved the uncertainty with the user.\n\n"
    } else {
        "If you are unsure whether a finding is valid and user clarification would be required, do not guess.\n\
Discard the finding and explain briefly in `overall_explanation` that it could not be confirmed from repository inspection alone.\n\n"
    };
    format!(
        "Validate the review findings below against the current codebase before they are surfaced to the user.\n\n\
Use repository inspection to verify each finding.\n\
Discard any finding that is incorrect, unsupported by the code, already fixed, or otherwise not actionable.\n\
{clarification_instruction}\
When you finish, return strict JSON matching the review output schema:\n\
- `findings` must contain only the findings that remain valid after verification.\n\
- Do not introduce any new finding that was not already present in the original review output.\n\
- Each surviving finding must match one original finding exactly by `title`, `priority`, and `code_location`.\n\
- You may adjust `body`, `overall_explanation`, and confidence scores when needed for accuracy.\n\
- If no findings remain, return `findings: []`, set `overall_correctness` to `{REVIEW_CORRECT_VERDICT}`, and explain briefly what was discarded.\n\
- If any findings remain, set `overall_correctness` to `{REVIEW_INCORRECT_VERDICT}`.\n\
- `overall_explanation` should summarize what you kept and what you discarded.\n\
- Do not include markdown fences or any text outside the JSON object.\n\n\
Original review context JSON:\n\
{review_context_json}"
    )
}

fn review_output_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "findings": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "title": { "type": "string" },
                        "body": { "type": "string" },
                        "confidence_score": { "type": "number" },
                        "priority": { "type": "integer" },
                        "code_location": {
                            "type": "object",
                            "additionalProperties": false,
                            "properties": {
                                "absolute_file_path": { "type": "string" },
                                "line_range": {
                                    "type": "object",
                                    "additionalProperties": false,
                                    "properties": {
                                        "start": { "type": "integer", "minimum": 0 },
                                        "end": { "type": "integer", "minimum": 0 }
                                    },
                                    "required": ["start", "end"]
                                }
                            },
                            "required": ["absolute_file_path", "line_range"]
                        }
                    },
                    "required": [
                        "title",
                        "body",
                        "confidence_score",
                        "priority",
                        "code_location"
                    ]
                }
            },
            "overall_correctness": { "type": "string" },
            "overall_explanation": { "type": "string" },
            "overall_confidence_score": { "type": "number" }
        },
        "required": [
            "findings",
            "overall_correctness",
            "overall_explanation",
            "overall_confidence_score"
        ]
    })
}

fn normalize_validated_review_output(mut review_output: ReviewOutputEvent) -> ReviewOutputEvent {
    review_output.overall_correctness = if review_output.findings.is_empty() {
        REVIEW_CORRECT_VERDICT.to_string()
    } else {
        REVIEW_INCORRECT_VERDICT.to_string()
    };
    if review_output.findings.is_empty() && review_output.overall_explanation.trim().is_empty() {
        review_output.overall_explanation = "No findings.".to_string();
    }
    review_output
}

fn ensure_validated_findings_match_original(
    original_findings: &[ReviewFinding],
    validated_output: ReviewOutputEvent,
) -> Result<ReviewOutputEvent, String> {
    let mut matched_original_findings = vec![false; original_findings.len()];
    for validated_finding in &validated_output.findings {
        let Some((matched_idx, _)) =
            original_findings
                .iter()
                .enumerate()
                .find(|(idx, original_finding)| {
                    !matched_original_findings[*idx]
                        && validated_finding.title == original_finding.title
                        && validated_finding.priority == original_finding.priority
                        && validated_finding.code_location == original_finding.code_location
                })
        else {
            return Err(REVIEW_UNEXPECTED_VALIDATED_FINDINGS_FAILURE_EXPLANATION.to_string());
        };
        matched_original_findings[matched_idx] = true;
    }
    Ok(validated_output)
}

/// Emits an ExitedReviewMode Event with either review output or an explicit failure message,
/// and records a developer message with the review output.
async fn exit_review_mode(session: Arc<Session>, review_exit: ReviewExit, ctx: Arc<TurnContext>) {
    const REVIEW_USER_MESSAGE_ID: &str = "review_rollout_user";
    const REVIEW_ASSISTANT_MESSAGE_ID: &str = "review_rollout_assistant";
    let (review_output, failure_message, user_message, assistant_message) = match review_exit {
        ReviewExit::Completed(out) => {
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
            (Some(out), None, rendered, assistant_message)
        }
        ReviewExit::Failed(message) => {
            let assistant_message = if message.trim().is_empty() {
                REVIEW_INTERRUPTED_MESSAGE.to_string()
            } else {
                message
            };
            (
                None,
                Some(assistant_message.clone()),
                crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL.to_string(),
                assistant_message,
            )
        }
        ReviewExit::Interrupted => (
            None,
            Some(REVIEW_INTERRUPTED_MESSAGE.to_string()),
            crate::client_common::REVIEW_EXIT_INTERRUPTED_TMPL.to_string(),
            REVIEW_INTERRUPTED_MESSAGE.to_string(),
        ),
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
            EventMsg::ExitedReviewMode(ExitedReviewModeEvent {
                review_output,
                failure_message,
            }),
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

    // Review turns can run before any regular user turn, so explicitly
    // materialize rollout persistence. Do this after emitting review output so
    // file creation + git metadata collection cannot delay client-facing items.
    session.ensure_rollout_materialized().await;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::codex::make_session_and_context_with_dynamic_tools_and_rx;
    use crate::config::ManagedFeatures;
    use crate::config::test_config;
    use crate::config_loader::FeatureRequirementsToml;
    use crate::config_loader::RequirementSource;
    use crate::config_loader::Sourced;
    use codex_protocol::permissions::FileSystemPath;
    use codex_protocol::permissions::FileSystemSandboxEntry;
    use codex_protocol::permissions::FileSystemSpecialPath;
    use codex_protocol::protocol::Event;
    use codex_protocol::protocol::ExecCommandEndEvent;
    use codex_protocol::protocol::ExecCommandSource;
    use codex_protocol::protocol::ReviewTarget;
    use codex_protocol::protocol::TurnCompleteEvent;
    use codex_utils_absolute_path::AbsolutePathBuf;
    use pretty_assertions::assert_eq;
    use std::collections::BTreeMap;
    use std::path::PathBuf;
    use std::time::Duration;
    use tempfile::tempdir;
    use tokio::time::timeout;

    fn failed_exec_end_event(call_id: &str) -> EventMsg {
        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: call_id.to_string(),
            process_id: None,
            turn_id: "hidden-review-turn".to_string(),
            command: vec!["git".to_string(), "diff".to_string()],
            cwd: PathBuf::from("/tmp"),
            parsed_cmd: Vec::new(),
            source: ExecCommandSource::Agent,
            interaction_input: None,
            stdout: String::new(),
            stderr: "fatal: bad revision".to_string(),
            aggregated_output: "fatal: bad revision".to_string(),
            exit_code: 128,
            duration: Duration::ZERO,
            formatted_output: "fatal: bad revision".to_string(),
            status: ExecCommandStatus::Failed,
        })
    }

    async fn recv_forwarded_exec_failure(
        rx_event: &async_channel::Receiver<Event>,
        call_id: &str,
    ) -> ExecCommandEndEvent {
        timeout(Duration::from_secs(2), async {
            loop {
                let event = rx_event.recv().await.expect("forwarded event");
                if let EventMsg::ExecCommandEnd(end) = event.msg
                    && end.call_id == call_id
                {
                    return end;
                }
            }
        })
        .await
        .expect("timed out waiting for forwarded exec failure")
    }

    #[test]
    fn validation_subagent_config_forces_read_only_sandbox() {
        let mut parent_config = test_config();
        parent_config.user_instructions = Some("user".to_string());
        parent_config.developer_instructions = Some("developer".to_string());
        parent_config.permissions.sandbox_policy =
            Constrained::allow_only(SandboxPolicy::DangerFullAccess);
        parent_config.permissions.file_system_sandbox_policy =
            FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess);
        parent_config.permissions.network_sandbox_policy = NetworkSandboxPolicy::Enabled;
        parent_config.permissions.windows_sandbox_mode = None;

        let (validator_config, request_user_input_available) = build_review_sub_agent_config(
            &parent_config,
            "gpt-5",
            REVIEW_FINDING_VALIDATION_PROMPT,
            true,
            true,
        );

        assert_eq!(
            validator_config.permissions.sandbox_policy.get(),
            &SandboxPolicy::new_read_only_policy()
        );
        assert_eq!(
            validator_config.permissions.file_system_sandbox_policy,
            FileSystemSandboxPolicy::from(&SandboxPolicy::new_read_only_policy())
        );
        assert_eq!(
            validator_config.permissions.network_sandbox_policy,
            NetworkSandboxPolicy::Restricted
        );
        assert_eq!(validator_config.permissions.network, None);
        assert_eq!(
            validator_config.permissions.windows_sandbox_mode,
            Some(WindowsSandboxModeToml::Unelevated)
        );
        assert_eq!(validator_config.user_instructions, Some("user".to_string()));
        assert_eq!(
            validator_config.developer_instructions,
            Some("developer".to_string())
        );
        assert!(
            validator_config
                .base_instructions
                .as_deref()
                .expect("validator base instructions")
                .contains(REVIEW_SUBAGENT_REPO_GUIDANCE_PROMPT)
        );
        assert!(request_user_input_available);
    }

    #[test]
    fn validation_subagent_config_preserves_parent_read_restrictions() {
        let temp = tempdir().expect("temp dir");
        let cwd = temp.path().join("repo");
        let readable_root = temp.path().join("readable");
        let writable_root = temp.path().join("scratch");
        let denied_root = writable_root.join("secret");
        std::fs::create_dir_all(&cwd).expect("cwd dir");
        std::fs::create_dir_all(&readable_root).expect("readable dir");
        std::fs::create_dir_all(&writable_root).expect("writable dir");
        std::fs::create_dir_all(&denied_root).expect("denied dir");

        let cwd = AbsolutePathBuf::from_absolute_path(cwd).expect("absolute cwd");
        let readable_root =
            AbsolutePathBuf::from_absolute_path(readable_root).expect("absolute readable root");
        let writable_root =
            AbsolutePathBuf::from_absolute_path(writable_root).expect("absolute writable root");
        let denied_root =
            AbsolutePathBuf::from_absolute_path(denied_root).expect("absolute denied root");

        let mut parent_config = test_config();
        parent_config.cwd = cwd.to_path_buf();
        parent_config.permissions.sandbox_policy =
            Constrained::allow_only(SandboxPolicy::WorkspaceWrite {
                writable_roots: vec![writable_root.clone()],
                read_only_access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![readable_root.clone()],
                },
                network_access: true,
                exclude_tmpdir_env_var: true,
                exclude_slash_tmp: true,
            });
        parent_config.permissions.file_system_sandbox_policy =
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::CurrentWorkingDirectory,
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: readable_root.clone(),
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: writable_root.clone(),
                    },
                    access: FileSystemAccessMode::Write,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: denied_root.clone(),
                    },
                    access: FileSystemAccessMode::None,
                },
            ]);
        parent_config.permissions.network_sandbox_policy = NetworkSandboxPolicy::Enabled;

        let (validator_config, _) = build_review_sub_agent_config(
            &parent_config,
            "gpt-5",
            REVIEW_FINDING_VALIDATION_PROMPT,
            true,
            true,
        );

        assert_eq!(
            validator_config.permissions.sandbox_policy.get(),
            &SandboxPolicy::ReadOnly {
                access: ReadOnlyAccess::Restricted {
                    include_platform_defaults: false,
                    readable_roots: vec![readable_root.clone(), cwd.clone(), writable_root.clone(),],
                },
                network_access: false,
            }
        );
        assert_eq!(
            validator_config.permissions.file_system_sandbox_policy,
            FileSystemSandboxPolicy::restricted(vec![
                FileSystemSandboxEntry {
                    path: FileSystemPath::Special {
                        value: FileSystemSpecialPath::CurrentWorkingDirectory,
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: readable_root.clone(),
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path {
                        path: writable_root.clone(),
                    },
                    access: FileSystemAccessMode::Read,
                },
                FileSystemSandboxEntry {
                    path: FileSystemPath::Path { path: denied_root },
                    access: FileSystemAccessMode::None,
                },
            ])
        );
        assert_eq!(
            validator_config.permissions.network_sandbox_policy,
            NetworkSandboxPolicy::Restricted
        );
    }

    #[test]
    fn review_subagent_config_preserves_parent_sandbox() {
        let mut parent_config = test_config();
        parent_config.user_instructions = Some("user".to_string());
        parent_config.developer_instructions = Some("developer".to_string());
        parent_config.permissions.sandbox_policy =
            Constrained::allow_only(SandboxPolicy::DangerFullAccess);
        parent_config.permissions.file_system_sandbox_policy =
            FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess);
        parent_config.permissions.network_sandbox_policy = NetworkSandboxPolicy::Enabled;
        parent_config.permissions.windows_sandbox_mode = None;

        let (reviewer_config, request_user_input_available) = build_review_sub_agent_config(
            &parent_config,
            "gpt-5",
            crate::REVIEW_PROMPT,
            false,
            false,
        );

        assert_eq!(
            reviewer_config.permissions.sandbox_policy.get(),
            &SandboxPolicy::DangerFullAccess
        );
        assert_eq!(
            reviewer_config.permissions.file_system_sandbox_policy,
            FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess)
        );
        assert_eq!(
            reviewer_config.permissions.network_sandbox_policy,
            NetworkSandboxPolicy::Enabled
        );
        assert_eq!(reviewer_config.permissions.windows_sandbox_mode, None);
        assert_eq!(reviewer_config.user_instructions, Some("user".to_string()));
        assert_eq!(
            reviewer_config.developer_instructions,
            Some("developer".to_string())
        );
        assert!(
            reviewer_config
                .base_instructions
                .as_deref()
                .expect("reviewer base instructions")
                .contains(REVIEW_SUBAGENT_REPO_GUIDANCE_PROMPT)
        );
        assert!(!request_user_input_available);
    }

    #[test]
    fn review_subagent_config_forces_read_only_sandbox_when_requested() {
        let mut parent_config = test_config();
        parent_config.permissions.sandbox_policy =
            Constrained::allow_only(SandboxPolicy::DangerFullAccess);
        parent_config.permissions.file_system_sandbox_policy =
            FileSystemSandboxPolicy::from(&SandboxPolicy::DangerFullAccess);
        parent_config.permissions.network_sandbox_policy = NetworkSandboxPolicy::Enabled;
        parent_config.permissions.windows_sandbox_mode = None;

        let (reviewer_config, request_user_input_available) = build_review_sub_agent_config(
            &parent_config,
            "gpt-5",
            crate::REVIEW_PROMPT,
            false,
            true,
        );

        assert_eq!(
            reviewer_config.permissions.sandbox_policy.get(),
            &SandboxPolicy::new_read_only_policy()
        );
        assert_eq!(
            reviewer_config.permissions.file_system_sandbox_policy,
            FileSystemSandboxPolicy::from(&SandboxPolicy::new_read_only_policy())
        );
        assert_eq!(
            reviewer_config.permissions.network_sandbox_policy,
            NetworkSandboxPolicy::Restricted
        );
        assert_eq!(reviewer_config.permissions.network, None);
        assert_eq!(
            reviewer_config.permissions.windows_sandbox_mode,
            Some(WindowsSandboxModeToml::Unelevated)
        );
        assert!(!request_user_input_available);
    }

    #[test]
    fn validation_subagent_config_allows_pinned_off_request_user_input_feature() {
        let mut parent_config = test_config();
        parent_config.features = ManagedFeatures::from_configured(
            parent_config.features.get().clone(),
            Some(Sourced {
                value: FeatureRequirementsToml {
                    entries: BTreeMap::from([(
                        "default_mode_request_user_input".to_string(),
                        false,
                    )]),
                },
                source: RequirementSource::Unknown,
            }),
        )
        .expect("managed features");

        let (validator_config, request_user_input_available) = build_review_sub_agent_config(
            &parent_config,
            "gpt-5",
            REVIEW_FINDING_VALIDATION_PROMPT,
            true,
            true,
        );

        assert!(
            !request_user_input_available,
            "request_user_input should remain unavailable when the feature is pinned off"
        );
        assert!(
            !validator_config
                .features
                .enabled(Feature::DefaultModeRequestUserInput)
        );
    }

    #[test]
    fn review_validation_prompt_omits_request_user_input_when_unavailable() {
        let prompt = review_validation_prompt(
            &ResolvedReviewRequest {
                target: ReviewTarget::UncommittedChanges,
                prompt: "Review the current changes".to_string(),
                user_facing_hint: "Uncommitted changes".to_string(),
            },
            &ReviewOutputEvent {
                overall_explanation: "Initial review output".to_string(),
                ..Default::default()
            },
            false,
        );

        assert!(!prompt.contains("request_user_input"));
        assert!(prompt.contains("Discard the finding"));
    }

    #[tokio::test]
    async fn review_event_pump_forwards_hidden_failed_exec_diagnostics() {
        let (session, turn_context, rx_event) =
            make_session_and_context_with_dynamic_tools_and_rx(Vec::new()).await;
        let expected_turn_id = turn_context.sub_id.clone();
        let session = Arc::new(SessionTaskContext::new(session));
        let (tx_child, rx_child) = async_channel::unbounded();
        let call_id = "first-pass-exec-failure";
        let review_output = serde_json::json!({
            "findings": [],
            "overall_correctness": "patch is correct",
            "overall_explanation": "No findings.",
            "overall_confidence_score": 0.8
        })
        .to_string();

        let review_events = tokio::spawn(process_review_events(
            session,
            turn_context,
            rx_child,
            false,
            false,
        ));

        tx_child
            .send(Event {
                id: "child-1".to_string(),
                msg: failed_exec_end_event(call_id),
            })
            .await
            .expect("send exec failure");
        tx_child
            .send(Event {
                id: "child-2".to_string(),
                msg: EventMsg::TurnComplete(TurnCompleteEvent {
                    turn_id: "child-turn".to_string(),
                    last_agent_message: Some(review_output),
                }),
            })
            .await
            .expect("send completion");
        drop(tx_child);

        let exec_failure = recv_forwarded_exec_failure(&rx_event, call_id).await;
        assert_eq!(exec_failure.call_id, call_id);
        assert_eq!(exec_failure.turn_id, expected_turn_id);
        assert_eq!(exec_failure.status, ExecCommandStatus::Failed);
        assert_eq!(exec_failure.exit_code, 128);
        assert_eq!(exec_failure.formatted_output, "fatal: bad revision");

        let parsed = review_events.await.expect("review event pump");
        assert_eq!(
            parsed,
            Some(ParsedReviewOutput::Structured(ReviewOutputEvent {
                findings: Vec::new(),
                overall_correctness: "patch is correct".to_string(),
                overall_explanation: "No findings.".to_string(),
                overall_confidence_score: 0.8,
            }))
        );
    }

    #[tokio::test]
    async fn review_validation_event_pump_forwards_hidden_failed_exec_diagnostics() {
        let (session, turn_context, rx_event) =
            make_session_and_context_with_dynamic_tools_and_rx(Vec::new()).await;
        let expected_turn_id = turn_context.sub_id.clone();
        let session = Arc::new(SessionTaskContext::new(session));
        let (tx_child, rx_child) = async_channel::unbounded();
        let call_id = "validator-exec-failure";
        let validated_output = serde_json::json!({
            "findings": [],
            "overall_correctness": "patch is correct",
            "overall_explanation": "Validated output.",
            "overall_confidence_score": 0.9
        })
        .to_string();

        let validation_events = tokio::spawn(process_review_validation_events(
            session,
            turn_context,
            rx_child,
        ));

        tx_child
            .send(Event {
                id: "child-1".to_string(),
                msg: failed_exec_end_event(call_id),
            })
            .await
            .expect("send exec failure");
        tx_child
            .send(Event {
                id: "child-2".to_string(),
                msg: EventMsg::TurnComplete(TurnCompleteEvent {
                    turn_id: "child-turn".to_string(),
                    last_agent_message: Some(validated_output),
                }),
            })
            .await
            .expect("send completion");
        drop(tx_child);

        let exec_failure = recv_forwarded_exec_failure(&rx_event, call_id).await;
        assert_eq!(exec_failure.call_id, call_id);
        assert_eq!(exec_failure.turn_id, expected_turn_id);
        assert_eq!(exec_failure.status, ExecCommandStatus::Failed);
        assert_eq!(exec_failure.exit_code, 128);
        assert_eq!(exec_failure.formatted_output, "fatal: bad revision");

        let parsed = validation_events.await.expect("validation event pump");
        assert_eq!(
            parsed,
            Some(ReviewOutputEvent {
                findings: Vec::new(),
                overall_correctness: "patch is correct".to_string(),
                overall_explanation: "Validated output.".to_string(),
                overall_confidence_score: 0.9,
            })
        );
    }
}
