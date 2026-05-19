//! Model-backed watchdog review for active thread goals.
//!
//! The watchdog runs as a read-only one-shot subagent over a forked copy of the
//! current transcript. It does not mutate goal state; its assessment is fed back
//! into the next main-agent continuation turn as hidden goal context.

use crate::codex_delegate::run_codex_thread_one_shot;
use crate::config::Config;
use crate::config::Constrained;
use crate::config::PermissionProfileState;
use crate::session::session::Session;
use crate::session::turn_context::TurnContext;
use anyhow::Context;
use codex_features::Feature;
use codex_protocol::models::PermissionProfile;
use codex_protocol::openai_models::ReasoningEffort;
use codex_protocol::protocol::AskForApproval;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::InitialHistory;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SandboxPolicy;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::protocol::ThreadGoal;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::warn;

const GOAL_WATCHDOG_NAME: &str = "goal_watchdog";
const GOAL_WATCHDOG_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum GoalWatchdogVerdict {
    Continue,
    Complete,
    Blocked,
}

impl GoalWatchdogVerdict {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Continue => "continue",
            Self::Complete => "complete",
            Self::Blocked => "blocked",
        }
    }
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct GoalWatchdogAssessment {
    verdict: GoalWatchdogVerdict,
    rationale: String,
    next_action: String,
    completion_evidence_missing: Vec<String>,
}

impl GoalWatchdogAssessment {
    fn render(&self) -> String {
        let mut lines = vec![
            "Goal watchdog model assessment:".to_string(),
            format!("- Verdict: {}", self.verdict.as_str()),
            format!("- Rationale: {}", self.rationale.trim()),
            format!("- Suggested next action: {}", self.next_action.trim()),
        ];
        if !self.completion_evidence_missing.is_empty() {
            lines.push("- Missing completion evidence:".to_string());
            for evidence in &self.completion_evidence_missing {
                lines.push(format!("  - {}", evidence.trim()));
            }
        }
        lines.join("\n")
    }
}

pub(crate) async fn goal_watchdog_report(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    goal: &ThreadGoal,
) -> String {
    match run_goal_watchdog_report(session, turn_context, goal).await {
        Ok(report) => report,
        Err(err) => {
            warn!("goal watchdog review failed: {err}");
            format!(
                "Goal watchdog model assessment unavailable: {err}. Continue from direct evidence and do not treat this watchdog failure as proof of completion."
            )
        }
    }
}

async fn run_goal_watchdog_report(
    session: Arc<Session>,
    turn_context: Arc<TurnContext>,
    goal: &ThreadGoal,
) -> anyhow::Result<String> {
    let initial_history = goal_watchdog_initial_history(session.as_ref()).await?;
    let config = goal_watchdog_config(turn_context.as_ref())?;
    let model = turn_context.model_info.slug.clone();
    let effort = goal_watchdog_reasoning_effort(turn_context.as_ref());
    let prompt = goal_watchdog_prompt(goal);
    let cancel_token = CancellationToken::new();
    let codex = run_codex_thread_one_shot(
        config,
        session.services.auth_manager.clone(),
        session.services.models_manager.clone(),
        vec![UserInput::Text {
            text: prompt,
            text_elements: Vec::new(),
        }],
        Arc::clone(&session),
        turn_context,
        cancel_token.clone(),
        SubAgentSource::Other(GOAL_WATCHDOG_NAME.to_string()),
        Some(goal_watchdog_output_schema()),
        Some(InitialHistory::Forked(initial_history)),
    )
    .await
    .context("failed to start goal watchdog model")?;

    let last_agent_message = wait_for_goal_watchdog(codex, cancel_token)
        .await
        .context("goal watchdog model did not complete")?;
    parse_goal_watchdog_assessment(&last_agent_message)
        .map(|assessment| {
            format!(
                "{}\n- Watchdog model: {model}\n- Watchdog reasoning effort: {}",
                assessment.render(),
                effort
                    .map(|effort| effort.to_string())
                    .unwrap_or_else(|| "default".to_string())
            )
        })
        .context("goal watchdog model returned invalid assessment")
}

async fn goal_watchdog_initial_history(session: &Session) -> anyhow::Result<Vec<RolloutItem>> {
    session
        .try_ensure_rollout_materialized()
        .await
        .context("failed to materialize rollout before goal watchdog review")?;
    session
        .flush_rollout()
        .await
        .context("failed to flush rollout before goal watchdog review")?;
    let live_thread = session.live_thread_for_persistence("fork goal watchdog")?;
    let history = live_thread.load_history(/*include_archived*/ true).await?;
    Ok(history.items)
}

fn goal_watchdog_config(turn_context: &TurnContext) -> anyhow::Result<Config> {
    let mut config = turn_context.config.as_ref().clone();
    config.model = Some(turn_context.model_info.slug.clone());
    config.model_reasoning_effort = goal_watchdog_reasoning_effort(turn_context);
    config.include_skill_instructions = false;
    config.developer_instructions = None;
    config.include_apps_instructions = false;
    config.permissions.approval_policy = Constrained::allow_only(AskForApproval::Never);
    let sandbox_policy = SandboxPolicy::new_read_only_policy();
    let permission_profile = PermissionProfile::from_legacy_sandbox_policy(&sandbox_policy);
    let permission_profile_state = PermissionProfileState::from_constrained_legacy(
        Constrained::allow_only(permission_profile),
    )
    .map_err(|err| anyhow::anyhow!("goal watchdog could not set permission profile: {err}"))?;
    config
        .permissions
        .set_permission_profile_state(permission_profile_state);
    config
        .permissions
        .set_legacy_sandbox_policy(sandbox_policy, config.cwd.as_path())
        .map_err(|err| anyhow::anyhow!("goal watchdog could not set sandbox policy: {err}"))?;
    for feature in [
        Feature::Goals,
        Feature::SpawnCsv,
        Feature::Collab,
        Feature::MultiAgentV2,
        Feature::CodexHooks,
        Feature::Apps,
        Feature::Plugins,
        Feature::WebSearchRequest,
        Feature::WebSearchCached,
    ] {
        config.features.disable(feature).map_err(|err| {
            anyhow::anyhow!(
                "goal watchdog could not disable `features.{}`: {err}",
                feature.key()
            )
        })?;
        if config.features.enabled(feature) {
            warn!(
                "goal watchdog could not disable `features.{}`; continuing with the feature enabled",
                feature.key()
            );
        }
    }
    Ok(config)
}

fn goal_watchdog_reasoning_effort(turn_context: &TurnContext) -> Option<ReasoningEffort> {
    if turn_context
        .model_info
        .supported_reasoning_levels
        .iter()
        .any(|preset| preset.effort == ReasoningEffort::Low)
    {
        Some(ReasoningEffort::Low)
    } else {
        turn_context
            .reasoning_effort
            .or(turn_context.model_info.default_reasoning_level)
    }
}

async fn wait_for_goal_watchdog(
    codex: crate::session::Codex,
    cancel_token: CancellationToken,
) -> anyhow::Result<String> {
    let mut last_error: Option<String> = None;
    let result = tokio::time::timeout(GOAL_WATCHDOG_TIMEOUT, async {
        loop {
            let event = codex.next_event().await?;
            match event.msg {
                EventMsg::TurnComplete(turn_complete) => {
                    return turn_complete.last_agent_message.ok_or_else(|| {
                        anyhow::anyhow!(
                            last_error
                                .unwrap_or_else(|| "watchdog completed without output".to_string())
                        )
                    });
                }
                EventMsg::TurnAborted(turn_aborted) => {
                    anyhow::bail!("watchdog turn aborted: {:?}", turn_aborted.reason);
                }
                EventMsg::Error(error) => {
                    last_error = Some(error.message);
                }
                _ => {}
            }
        }
    })
    .await;

    match result {
        Ok(result) => result,
        Err(_) => {
            cancel_token.cancel();
            let _ = codex.submit(Op::Interrupt).await;
            anyhow::bail!(
                "watchdog timed out after {} seconds",
                GOAL_WATCHDOG_TIMEOUT.as_secs()
            );
        }
    }
}

fn goal_watchdog_prompt(goal: &ThreadGoal) -> String {
    format!(
        r#"You are a goal watchdog model monitoring the main Codex agent.

Review the forked transcript and the active thread goal below. Do not perform the user's work, edit files, call tools, or mark the goal complete. Your job is to independently assess whether the main agent has enough evidence to continue, ask the user, or complete the goal.

Active goal:
<objective>
{}
</objective>

Return only JSON matching this contract:
- verdict: "continue" when the main agent should keep working, "complete" when the transcript already proves the whole goal is done, or "blocked" when the main agent should ask the user before continuing.
- rationale: one concise evidence-based sentence.
- next_action: the single highest-value next action for the main agent.
- completion_evidence_missing: a list of concrete missing evidence items; use an empty list only when verdict is "complete".
"#,
        escape_watchdog_objective(&goal.objective)
    )
}

fn goal_watchdog_output_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "additionalProperties": false,
        "required": [
            "verdict",
            "rationale",
            "next_action",
            "completion_evidence_missing"
        ],
        "properties": {
            "verdict": {
                "type": "string",
                "enum": ["continue", "complete", "blocked"]
            },
            "rationale": {
                "type": "string"
            },
            "next_action": {
                "type": "string"
            },
            "completion_evidence_missing": {
                "type": "array",
                "items": {
                    "type": "string"
                }
            }
        }
    })
}

fn parse_goal_watchdog_assessment(input: &str) -> anyhow::Result<GoalWatchdogAssessment> {
    let assessment: GoalWatchdogAssessment = serde_json::from_str(input)?;
    if assessment.rationale.trim().is_empty() {
        anyhow::bail!("watchdog rationale must not be empty");
    }
    if assessment.next_action.trim().is_empty() {
        anyhow::bail!("watchdog next_action must not be empty");
    }
    Ok(assessment)
}

fn escape_watchdog_objective(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn parses_and_renders_watchdog_assessment() {
        let assessment = parse_goal_watchdog_assessment(
            r#"{"verdict":"continue","rationale":"Tests have not run yet.","next_action":"Run the focused tests.","completion_evidence_missing":["targeted test output","fmt output"]}"#,
        )
        .expect("assessment should parse");

        assert_eq!(
            assessment.render(),
            "Goal watchdog model assessment:\n- Verdict: continue\n- Rationale: Tests have not run yet.\n- Suggested next action: Run the focused tests.\n- Missing completion evidence:\n  - targeted test output\n  - fmt output"
        );
    }

    #[test]
    fn watchdog_prompt_escapes_objective_delimiters() {
        let goal = ThreadGoal {
            thread_id: codex_protocol::ThreadId::new(),
            objective: "Finish <phase> & audit > guesswork".to_string(),
            status: codex_protocol::protocol::ThreadGoalStatus::Active,
            token_budget: None,
            tokens_used: 0,
            time_used_seconds: 0,
            created_at: 1,
            updated_at: 1,
        };

        let prompt = goal_watchdog_prompt(&goal);

        assert!(prompt.contains("Finish &lt;phase&gt; &amp; audit &gt; guesswork"));
        assert!(!prompt.contains("Finish <phase> & audit > guesswork"));
    }
}
