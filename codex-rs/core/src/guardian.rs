use std::collections::BTreeSet;
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
use tokio::time::timeout;
use tokio_util::sync::CancellationToken;

use crate::codex::Session;
use crate::codex::TurnContext;
use crate::codex_delegate::run_codex_thread_one_shot;
use crate::compact::content_items_to_text;
use crate::config::Config;
use crate::config::Constrained;
use crate::event_mapping::is_contextual_user_message_content;
use crate::features::Feature;
use crate::protocol::SandboxPolicy;
use crate::truncate::TruncationPolicy;
use crate::truncate::approx_token_count;
use crate::truncate::truncate_text;
use codex_protocol::protocol::ReviewDecision;

const GUARDIAN_MODEL: &str = "gpt-5.3-codex";
const GUARDIAN_REVIEW_TIMEOUT: Duration = Duration::from_secs(90);
const GUARDIAN_MAX_TRANSCRIPT_TOKENS: usize = 3_500;
const GUARDIAN_RECENT_ENTRY_LIMIT: usize = 20;

pub(crate) const GUARDIAN_REJECTION_MESSAGE: &str = "Guardian rejected this action due to unacceptable risk. The agent must not attempt to achieve the same outcome via workaround, indirect execution, or policy circumvention. Proceed only with a materially safer alternative, or stop and request user input.";

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct GuardianEvidence {
    message: String,
    why: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct GuardianAssessment {
    risk_level: GuardianRiskLevel,
    risk_score: u8,
    rationale: String,
    evidence: Vec<GuardianEvidence>,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GuardianRiskLevel {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewResult {
    pub(crate) approved: bool,
    pub(crate) assessment: GuardianAssessment,
}

#[derive(Debug, Clone)]
pub(crate) struct GuardianReviewRequest {
    pub(crate) tool_name: &'static str,
    pub(crate) action: Value,
}

#[derive(Debug)]
struct GuardianTranscriptEntry {
    number: usize,
    role: &'static str,
    is_user: bool,
    text: String,
}

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
    let review = timeout(
        GUARDIAN_REVIEW_TIMEOUT,
        run_guardian_subagent(session.clone(), turn.clone(), prompt, schema),
    )
    .await;

    let assessment = match review {
        Ok(Ok(assessment)) => assessment,
        Ok(Err(err)) => GuardianAssessment {
            risk_level: GuardianRiskLevel::High,
            risk_score: 100,
            rationale: format!("Guardian review failed: {err}"),
            evidence: vec![],
        },
        Err(_) => GuardianAssessment {
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

    GuardianReviewResult {
        approved,
        assessment,
    }
}

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

    for cap in [220usize, 120, 60] {
        while transcript_token_count(entries, &included_numbers, cap)
            > GUARDIAN_MAX_TRANSCRIPT_TOKENS
        {
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

async fn run_guardian_subagent(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    prompt: String,
    schema: Value,
) -> anyhow::Result<GuardianAssessment> {
    let mut guardian_config: Config = turn.config.as_ref().clone();
    guardian_config.model = Some(GUARDIAN_MODEL.to_string());
    guardian_config.model_reasoning_effort =
        Some(codex_protocol::openai_models::ReasoningEffort::Low);
    guardian_config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    guardian_config.permissions.sandbox_policy =
        Constrained::allow_only(SandboxPolicy::new_read_only_policy());
    let _ = guardian_config.features.disable(Feature::Collab);
    let _ = guardian_config.features.disable(Feature::WebSearchRequest);
    let _ = guardian_config.features.disable(Feature::WebSearchCached);

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
        CancellationToken::new(),
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
    use codex_protocol::models::ContentItem;

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
}
