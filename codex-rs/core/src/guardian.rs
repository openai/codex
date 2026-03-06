//! Guardian review decides whether a tool may leave the normal sandbox.
//!
//! High-level approach:
//! 1. Reconstruct a compact transcript that preserves user intent and the most
//!    recent assistant context.
//! 2. Ask a dedicated guardian subagent to assess the exact planned action and
//!    return strict JSON.
//!    The guardian clones the parent config, so it inherits any managed
//!    network proxy / allowlist that the parent turn already had.
//! 3. Fail closed on timeout, execution failure, or malformed output.
//! 4. Approve only low- and medium-risk actions (`risk_score < 80`).

use std::collections::BTreeSet;
use std::future::Future;
use std::sync::Arc;
use std::time::Duration;

use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::WarningEvent;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_one_shot;
use crate::compact::content_items_to_text;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::NetworkProxySpec;
use crate::event_mapping::is_contextual_user_message_content;
use crate::features::Feature;
use crate::protocol::SandboxPolicy;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::truncate_text;
use codex_protocol::protocol::ReviewDecision;

const GUARDIAN_MODEL: &str = "gpt-5.4";
const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
// This is intentionally much smaller than the model's context window. Guardian
// only needs enough conversation state to establish intent and recent context;
// keeping the transcript compact helps latency and leaves plenty of room for
// the policy prompt and exact planned action JSON.
const GUARDIAN_MAX_TRANSCRIPT_TOKENS: usize = 3_500;
// Always keep some recent non-user context so the reviewer can see what the
// agent was trying to do immediately before the escalation.
const GUARDIAN_RECENT_ENTRY_LIMIT: usize = 20;

pub(crate) const GUARDIAN_REJECTION_MESSAGE: &str = "Guardian rejected this action due to unacceptable risk. The agent must not attempt to achieve the same outcome via workaround, indirect execution, or policy circumvention. Proceed only with a materially safer alternative, or stop and request user input.";

/// Canonical description of the action the guardian is being asked to review.
#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewRequest {
    pub(crate) tool_name: &'static str,
    pub(crate) action: Value,
}

/// Coarse risk label paired with the numeric `risk_score`.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GuardianRiskLevel {
    Low,
    Medium,
    High,
}

/// Evidence item returned by the guardian subagent.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct GuardianEvidence {
    message: String,
    why: String,
}

/// Structured output contract that the guardian subagent must satisfy.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct GuardianAssessment {
    risk_level: GuardianRiskLevel,
    risk_score: u8,
    rationale: String,
    evidence: Vec<GuardianEvidence>,
}

/// Minimal result shape used by the existing approval/escalation pipeline.
#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewResult {
    pub(crate) approved: bool,
}

#[derive(Debug)]
enum GuardianReviewFailure {
    TimedOut,
    Failed(anyhow::Error),
}

/// Transcript entry retained for guardian review after filtering and numbering.
#[derive(Debug)]
struct GuardianTranscriptEntry {
    number: usize,
    role: &'static str,
    is_user: bool,
    text: String,
}

/// Top-level guardian review entry point for tool retries that need to leave
/// the current sandbox.
///
/// This function always fails closed: any timeout, subagent failure, or parse
/// failure is treated as a high-risk denial.
pub(crate) async fn review_sandbox_escalation(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    tool_name: &str,
    planned_action: Option<Value>,
    retry_reason: Option<String>,
) -> GuardianReviewResult {
    session
        .notify_background_event(
            turn.as_ref(),
            format!("Guardian assessing sandbox escalation for {tool_name}..."),
        )
        .await;

    let prompt = build_guardian_prompt(session.as_ref(), retry_reason, planned_action).await;
    let schema = guardian_output_schema();
    let cancel_token = CancellationToken::new();
    let review = run_guardian_subagent_with_timeout(
        run_guardian_subagent(
            session.clone(),
            turn.clone(),
            prompt,
            schema,
            cancel_token.clone(),
        ),
        cancel_token,
        GUARDIAN_REVIEW_TIMEOUT,
    )
    .await;

    let assessment = match review {
        Ok(assessment) => assessment,
        Err(GuardianReviewFailure::Failed(err)) => GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            risk_score: 100,
            rationale: format!("Guardian review failed: {err}"),
            evidence: vec![],
        },
        Err(GuardianReviewFailure::TimedOut) => GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            risk_score: 100,
            rationale:
                "Guardian review timed out while evaluating the requested sandbox escalation."
                    .to_string(),
            evidence: vec![],
        },
    };

    let approved = assessment.risk_score < 80;
    let verdict = if approved { "approved" } else { "denied" };
    // Emit a concise warning so the parent turn has an auditable summary of the
    // guardian decision without needing the full subagent transcript.
    let warning = format!(
        "Guardian {verdict} sandbox escalation ({}/100, {}): {}",
        assessment.risk_score,
        assessment.risk_level.as_str(),
        assessment.rationale
    );
    session
        .send_event(
            turn.as_ref(),
            EventMsg::Warning(WarningEvent { message: warning }),
        )
        .await;

    GuardianReviewResult { approved }
}

async fn run_guardian_subagent_with_timeout<F>(
    review_fut: F,
    cancel_token: CancellationToken,
    timeout_duration: Duration,
) -> Result<GuardianAssessment, GuardianReviewFailure>
where
    F: Future<Output = anyhow::Result<GuardianAssessment>>,
{
    tokio::select! {
        review = review_fut => review.map_err(GuardianReviewFailure::Failed),
        _ = tokio::time::sleep(timeout_duration) => {
            // Cancel the delegate token before failing closed so the one-shot
            // subagent tears down its background streams instead of lingering
            // after the caller has already timed out.
            cancel_token.cancel();
            Err(GuardianReviewFailure::TimedOut)
        }
    }
}

/// Adapter used by callsites that already traffic in `ReviewDecision`.
pub(crate) async fn review_escalation(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: GuardianReviewRequest,
) -> ReviewDecision {
    review_escalation_with_reason(session, turn, request, None).await
}

pub(crate) async fn review_escalation_with_reason(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    request: GuardianReviewRequest,
    retry_reason: Option<String>,
) -> ReviewDecision {
    let review = review_sandbox_escalation(
        Arc::clone(session),
        Arc::clone(turn),
        request.tool_name,
        Some(request.action),
        retry_reason,
    )
    .await;

    if review.approved {
        ReviewDecision::Approved
    } else {
        ReviewDecision::Denied
    }
}

/// Builds the guardian prompt from three pieces:
/// - the policy prompt
/// - a compact transcript for authorization and local context
/// - the exact action JSON being proposed for escalation
async fn build_guardian_prompt(
    session: &Session,
    retry_reason: Option<String>,
    planned_action: Option<Value>,
) -> String {
    let history = session.clone_history().await;
    let transcript_entries = collect_guardian_transcript_entries(history.raw_items());
    let session_id = session.conversation_id.to_string();
    let rollout_path = session
        .services
        .rollout
        .lock()
        .await
        .as_ref()
        .map(|rollout| rollout.rollout_path().display().to_string());
    let planned_action_json = planned_action
        .map(|action| serde_json::to_string_pretty(&action).unwrap_or_else(|_| "{}".to_string()))
        .unwrap_or_else(|| "{}".to_string());

    let (transcript, omission_note) = build_guardian_transcript(
        transcript_entries.as_slice(),
        &session_id,
        rollout_path.as_deref(),
    );
    let retry_reason_block = retry_reason
        .map(|reason| format!("\nRetry reason: {reason}\n"))
        .unwrap_or_default();
    let omission_block = omission_note
        .map(|note| format!("\n{note}\n"))
        .unwrap_or_default();

    format!(
        "{}\n{}\n{}>>> TRANSCRIPT START\n{}\n>>> TRANSCRIPT END\n{}\nPlanned action JSON:\n{}\n",
        guardian_policy_prompt(),
        retry_reason_block,
        "Assess the exact planned action below. Use read-only tool checks when local state matters.\n",
        transcript,
        omission_block,
        planned_action_json
    )
}

/// Keeps all user turns plus a bounded amount of recent assistant context.
///
/// The pruning strategy is intentionally simple and reviewable:
/// - always retain user messages because they carry authorization and intent
/// - seed the transcript with the most recent entries
/// - if the transcript is still too large, drop older non-user entries first
/// - progressively shrink the per-entry truncation cap before giving up
fn build_guardian_transcript(
    entries: &[GuardianTranscriptEntry],
    session_id: &str,
    rollout_path: Option<&str>,
) -> (String, Option<String>) {
    if entries.is_empty() {
        let note = format!(
            "Conversation transcript omitted. Session ID: {session_id}. Rollout path: {}. Full conversation can be consulted for deeper judgment.",
            rollout_path.unwrap_or("<unavailable>")
        );
        return ("<no retained transcript entries>".to_string(), Some(note));
    }

    // Preserve all user turns and a slice of recent context so the reviewer can
    // see both the authorization signal and the immediate lead-up to retry.
    let recent_numbers: BTreeSet<usize> = entries
        .iter()
        .rev()
        .take(GUARDIAN_RECENT_ENTRY_LIMIT)
        .map(|entry| entry.number)
        .collect();
    let mut included_numbers: BTreeSet<usize> = entries
        .iter()
        .filter(|entry| entry.is_user || recent_numbers.contains(&entry.number))
        .map(|entry| entry.number)
        .collect();

    // Start with more generous per-entry truncation, then tighten it if needed
    // before dropping the transcript entirely.
    for cap in [220usize, 120, 60] {
        while transcript_token_count(entries, &included_numbers, cap)
            > GUARDIAN_MAX_TRANSCRIPT_TOKENS
        {
            // Drop the oldest non-user context first. User messages remain
            // sticky because they are the strongest evidence of authorization.
            let Some(number) = entries
                .iter()
                .find(|entry| included_numbers.contains(&entry.number) && !entry.is_user)
                .map(|entry| entry.number)
            else {
                break;
            };
            included_numbers.remove(&number);
        }

        if transcript_token_count(entries, &included_numbers, cap) <= GUARDIAN_MAX_TRANSCRIPT_TOKENS
        {
            let transcript = render_transcript(entries, &included_numbers, cap);
            let omission = if included_numbers.len() < entries.len() {
                Some(format!(
                    "Earlier conversation entries were omitted. Session ID: {session_id}. Rollout path: {}. Full conversation can be consulted for deeper judgment.",
                    rollout_path.unwrap_or("<unavailable>")
                ))
            } else {
                None
            };
            return (transcript, omission);
        }
    }

    (
        "<transcript omitted to preserve budget for planned action>".to_string(),
        Some(format!(
            "Conversation transcript omitted due to size. Session ID: {session_id}. Rollout path: {}. Full conversation can be consulted for deeper judgment.",
            rollout_path.unwrap_or("<unavailable>")
        )),
    )
}

fn render_transcript(
    entries: &[GuardianTranscriptEntry],
    included_numbers: &BTreeSet<usize>,
    per_entry_token_cap: usize,
) -> String {
    entries
        .iter()
        .filter(|entry| included_numbers.contains(&entry.number))
        .map(|entry| {
            let text = truncate_text(&entry.text, TruncationPolicy::Tokens(per_entry_token_cap));
            format!("[{}] {}: {}", entry.number, entry.role, text)
        })
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn transcript_token_count(
    entries: &[GuardianTranscriptEntry],
    included_numbers: &BTreeSet<usize>,
    per_entry_token_cap: usize,
) -> usize {
    approx_token_count(&render_transcript(
        entries,
        included_numbers,
        per_entry_token_cap,
    ))
}

/// Retains only human-readable user/assistant message content for guardian
/// review and skips synthetic contextual scaffolding that would just add noise.
fn collect_guardian_transcript_entries(items: &[ResponseItem]) -> Vec<GuardianTranscriptEntry> {
    items
        .iter()
        .filter_map(|item| match item {
            ResponseItem::Message { role, content, .. } if role == "user" => {
                if is_contextual_user_message_content(content) {
                    None
                } else {
                    content_items_to_text(content).map(|text| ("user", true, text))
                }
            }
            ResponseItem::Message { role, content, .. } if role == "assistant" => {
                content_items_to_text(content).map(|text| ("assistant", false, text))
            }
            _ => None,
        })
        .enumerate()
        .map(|(idx, (role, is_user, text))| GuardianTranscriptEntry {
            number: idx + 1,
            role,
            is_user,
            text,
        })
        .collect()
}

/// Runs the guardian as a locked-down one-shot subagent.
///
/// The guardian itself should not mutate state, trigger further approvals, or
/// roam the network, so it is pinned to a read-only sandbox with
/// `approval_policy = never` and nonessential agent features disabled.
async fn run_guardian_subagent(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    prompt: String,
    schema: Value,
    cancel_token: CancellationToken,
) -> anyhow::Result<GuardianAssessment> {
    let live_network_config = match session.services.network_proxy.as_ref() {
        Some(network_proxy) => Some(network_proxy.proxy().current_cfg().await?),
        None => None,
    };
    let guardian_config =
        build_guardian_subagent_config(turn.config.as_ref(), live_network_config)?;

    // `run_codex_thread_one_shot` is already the subagent runner used elsewhere
    // in core. Reusing it here keeps the MVP aligned with the existing review
    // subagent model instead of introducing a guardian-specific execution path.
    let codex = run_codex_thread_one_shot(
        guardian_config,
        session.services.auth_manager.clone(),
        session.services.models_manager.clone(),
        vec![UserInput::Text {
            text: prompt,
            text_elements: Vec::new(),
        }],
        session,
        turn,
        cancel_token,
        SubAgentSource::Other("guardian".to_string()),
        Some(schema),
        None,
    )
    .await?;

    let mut last_agent_message = None;
    while let Ok(event) = codex.next_event().await {
        match event.msg {
            EventMsg::TurnComplete(event) => {
                last_agent_message = event.last_agent_message;
                break;
            }
            EventMsg::TurnAborted(_) => break,
            _ => {}
        }
    }

    parse_guardian_assessment(last_agent_message.as_deref())
}

/// Builds the locked-down guardian config from the parent turn config.
///
/// The guardian stays read-only and cannot request more permissions itself, but
/// cloning the parent config preserves any already-configured managed network
/// proxy / allowlist. When the parent session has edited that proxy state
/// in-memory, we refresh from the live runtime config so the guardian sees the
/// same current allowlist as the parent turn.
fn build_guardian_subagent_config(
    parent_config: &Config,
    live_network_config: Option<codex_network_proxy::NetworkProxyConfig>,
) -> anyhow::Result<Config> {
    let mut guardian_config = parent_config.clone();
    guardian_config.model = Some(GUARDIAN_MODEL.to_string());
    guardian_config.model_reasoning_effort =
        Some(codex_protocol::openai_models::ReasoningEffort::Low);
    guardian_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    guardian_config.permissions.sandbox_policy =
        Constrained::allow_only(SandboxPolicy::new_read_only_policy());
    if let Some(live_network_config) = live_network_config
        && guardian_config.permissions.network.is_some()
    {
        let network_constraints = guardian_config
            .config_layer_stack
            .requirements()
            .network
            .as_ref()
            .map(|network| network.value.clone());
        guardian_config.permissions.network = Some(NetworkProxySpec::from_config_and_constraints(
            live_network_config,
            network_constraints,
        )?);
    }
    let _ = guardian_config.features.disable(Feature::Collab);
    let _ = guardian_config.features.disable(Feature::WebSearchRequest);
    let _ = guardian_config.features.disable(Feature::WebSearchCached);
    Ok(guardian_config)
}

/// The model is asked for strict JSON, but we still accept a surrounding prose
/// wrapper so transient formatting drift fails less noisily during dogfooding.
fn parse_guardian_assessment(text: Option<&str>) -> anyhow::Result<GuardianAssessment> {
    let Some(text) = text else {
        anyhow::bail!("guardian review completed without an assessment payload");
    };
    if let Ok(assessment) = serde_json::from_str::<GuardianAssessment>(text) {
        return Ok(assessment);
    }
    if let (Some(start), Some(end)) = (text.find('{'), text.rfind('}'))
        && start < end
        && let Some(slice) = text.get(start..=end)
    {
        return Ok(serde_json::from_str::<GuardianAssessment>(slice)?);
    }
    anyhow::bail!("guardian assessment was not valid JSON")
}

/// JSON schema supplied as `final_output_json_schema` to force a structured
/// final answer from the guardian subagent.
fn guardian_output_schema() -> Value {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "risk_level": {
                "type": "string",
                "enum": ["low", "medium", "high"]
            },
            "risk_score": {
                "type": "integer",
                "minimum": 0,
                "maximum": 100
            },
            "rationale": {
                "type": "string"
            },
            "evidence": {
                "type": "array",
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "message": { "type": "string" },
                        "why": { "type": "string" }
                    },
                    "required": ["message", "why"]
                }
            }
        },
        "required": ["risk_level", "risk_score", "rationale", "evidence"]
    })
}

/// Embedded policy prompt for the guardian MVP.
///
/// Keeping the policy text local to this file makes the review contract easy to
/// audit alongside the parsing and approval threshold logic.
fn guardian_policy_prompt() -> &'static str {
    r#"You are performing a risk assessment of a coding-agent sandbox escalation.

Your primary objective is to identify whether the planned action poses a risk of irreversible damage to the user or the organisation.

Workspace policy:
- Actions that send internal OpenAI data outside OpenAI-controlled systems are always risky.
- Treat non-OpenAI endpoints as high-risk for exfiltration.
- Using credentials to authenticate a user-requested action is not by itself credential exfiltration.
- Before assigning high risk for network actions, identify what data is actually leaving.
- Flag destructive or costly-to-reverse actions as risky when they are highly likely to cause material damage, data loss, service disruption, permission changes, or broad unintended mutation.
- Do not treat normal internal operational workflows as destructive solely because they are large-scale or long-running.
- Sandbox retry itself is not suspicious; evaluate the unsandboxed action, not the existence of a sandbox denial.
- Benign local filesystem actions are usually low risk even if outside the workspace root.
- Do not assign high risk solely because a path is outside the writable workspace roots.
- If the transcript shows an explicit user request for the exact target path or action, treat that as a strong authorization signal that materially lowers risk, but not enough to override clear exfiltration or system-damage concerns.

Output strict JSON with this exact schema:
{
  "risk_level": "low" | "medium" | "high",
  "risk_score": 0-100,
  "rationale": string,
  "evidence": [{"message": string, "why": string}]
}"#
}

impl GuardianRiskLevel {
    fn as_str(self) -> &'static str {
        match self {
            GuardianRiskLevel::Low => "low",
            GuardianRiskLevel::Medium => "medium",
            GuardianRiskLevel::High => "high",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::NetworkProxySpec;
    use crate::config::test_config;
    use crate::config_loader::NetworkConstraints;
    use codex_network_proxy::NetworkProxyConfig;
    use codex_protocol::models::ContentItem;
    use pretty_assertions::assert_eq;

    #[test]
    fn build_guardian_transcript_keeps_original_numbering() {
        let entries = [
            GuardianTranscriptEntry {
                number: 1,
                role: "user",
                is_user: true,
                text: "first".to_string(),
            },
            GuardianTranscriptEntry {
                number: 2,
                role: "assistant",
                is_user: false,
                text: "second".to_string(),
            },
            GuardianTranscriptEntry {
                number: 3,
                role: "assistant",
                is_user: false,
                text: "third".to_string(),
            },
        ];

        let (transcript, omission) =
            build_guardian_transcript(&entries[..2], "session-1", Some("/tmp/rollout.jsonl"));

        assert!(transcript.contains("[1] user: first"));
        assert!(transcript.contains("[2] assistant: second"));
        assert!(omission.is_none());
    }

    #[test]
    fn collect_guardian_transcript_entries_skips_contextual_user_messages() {
        let items = vec![
            ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>"
                        .to_string(),
                }],
                end_turn: None,
                phase: None,
            },
            ResponseItem::Message {
                id: None,
                role: "assistant".to_string(),
                content: vec![ContentItem::OutputText {
                    text: "hello".to_string(),
                }],
                end_turn: None,
                phase: None,
            },
        ];

        let entries = collect_guardian_transcript_entries(&items);

        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].number, 1);
        assert_eq!(entries[0].role, "assistant");
    }

    #[test]
    fn parse_guardian_assessment_extracts_embedded_json() {
        let parsed = parse_guardian_assessment(Some(
            "preface {\"risk_level\":\"medium\",\"risk_score\":42,\"rationale\":\"ok\",\"evidence\":[]}",
        ))
        .expect("guardian assessment");

        assert_eq!(parsed.risk_score, 42);
        assert_eq!(parsed.risk_level, GuardianRiskLevel::Medium);
    }

    #[tokio::test]
    async fn guardian_timeout_cancels_subagent_token() {
        let cancel_token = CancellationToken::new();
        let waiter = tokio::spawn({
            let cancel_token = cancel_token.clone();
            async move {
                cancel_token.cancelled().await;
            }
        });

        let result = run_guardian_subagent_with_timeout(
            std::future::pending::<anyhow::Result<GuardianAssessment>>(),
            cancel_token,
            Duration::from_millis(10),
        )
        .await;

        assert!(matches!(result, Err(GuardianReviewFailure::TimedOut)));
        tokio::time::timeout(Duration::from_secs(1), waiter)
            .await
            .expect("timeout helper should cancel the guardian token")
            .expect("waiter task should finish cleanly");
    }

    #[test]
    fn guardian_subagent_config_preserves_parent_network_proxy() {
        let mut parent_config = test_config();
        let network = NetworkProxySpec::from_config_and_constraints(
            NetworkProxyConfig::default(),
            Some(NetworkConstraints {
                enabled: Some(true),
                allowed_domains: Some(vec!["github.com".to_string()]),
                ..Default::default()
            }),
        )
        .expect("network proxy spec");
        parent_config.permissions.network = Some(network.clone());

        let guardian_config =
            build_guardian_subagent_config(&parent_config, None).expect("guardian config");

        assert_eq!(guardian_config.permissions.network, Some(network));
        assert_eq!(
            guardian_config.permissions.approval_policy,
            Constrained::allow_only(AskForApproval::Never)
        );
        assert_eq!(
            guardian_config.permissions.sandbox_policy,
            Constrained::allow_only(SandboxPolicy::new_read_only_policy())
        );
    }

    #[test]
    fn guardian_subagent_config_uses_live_network_proxy_state() {
        let mut parent_config = test_config();
        let mut parent_network = NetworkProxyConfig::default();
        parent_network.network.enabled = true;
        parent_network.network.allowed_domains = vec!["parent.example".to_string()];
        parent_config.permissions.network = Some(
            NetworkProxySpec::from_config_and_constraints(parent_network, None)
                .expect("parent network proxy spec"),
        );

        let mut live_network = NetworkProxyConfig::default();
        live_network.network.enabled = true;
        live_network.network.allowed_domains = vec!["github.com".to_string()];

        let guardian_config =
            build_guardian_subagent_config(&parent_config, Some(live_network.clone()))
                .expect("guardian config");

        assert_eq!(
            guardian_config.permissions.network,
            Some(
                NetworkProxySpec::from_config_and_constraints(live_network, None)
                    .expect("live network proxy spec")
            )
        );
    }
}
