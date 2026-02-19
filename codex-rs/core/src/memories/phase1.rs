use crate::Prompt;
use crate::RolloutRecorder;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::types::MemoriesConfig;
use crate::error::CodexErr;
use crate::memories::metrics;
use crate::memories::phase_one;
use crate::memories::prompts::build_stage_one_input_message;
use crate::rollout::INTERACTIVE_SESSION_SOURCES;
use crate::rollout::policy::should_persist_response_item_for_memories;
use codex_api::ResponseEvent;
use codex_otel::OtelManager;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TokenUsage;
use codex_utils_sanitizer::redact_secrets;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

#[derive(Clone, Debug)]
pub(in crate::memories) struct RequestContext {
    pub(in crate::memories) model_info: ModelInfo,
    pub(in crate::memories) otel_manager: OtelManager,
    pub(in crate::memories) reasoning_effort: Option<ReasoningEffortConfig>,
    pub(in crate::memories) reasoning_summary: ReasoningSummaryConfig,
    pub(in crate::memories) turn_metadata_header: Option<String>,
}

struct JobResult {
    outcome: JobOutcome,
    token_usage: Option<TokenUsage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JobOutcome {
    SucceededWithOutput,
    SucceededNoOutput,
    Failed,
}

struct Stats {
    claimed: usize,
    succeeded_with_output: usize,
    succeeded_no_output: usize,
    failed: usize,
    total_token_usage: Option<TokenUsage>,
}

/// Phase 1 model output payload.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct StageOneOutput {
    /// Detailed markdown raw memory for a single rollout.
    #[serde(rename = "raw_memory")]
    pub(crate) raw_memory: String,
    /// Compact summary line used for routing and indexing.
    #[serde(rename = "rollout_summary")]
    pub(crate) rollout_summary: String,
    /// Optional slug used to derive rollout summary artifact filenames.
    #[serde(default, rename = "rollout_slug")]
    pub(crate) rollout_slug: Option<String>,
}

/// Runs memory phase 1 in strict step order:
/// 1) claim eligible rollout jobs
/// 2) build one stage-1 request context
/// 3) run stage-1 extraction jobs in parallel
/// 4) emit metrics and logs
pub(in crate::memories) async fn run(session: &Arc<Session>, config: &Config) {
    let _phase_one_e2e_timer = session
        .services
        .otel_manager
        .start_timer(metrics::MEMORY_PHASE_ONE_E2E_MS, &[])
        .ok();

    // 1. Claim startup job.
    let Some(claimed_candidates) = claim_startup_jobs(session, &config.memories).await else {
        return;
    };
    if claimed_candidates.is_empty() {
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            1,
            &[("status", "skipped_no_candidates")],
        );
        return;
    }

    // 2. Build request.
    let stage_one_context = build_request_context(session, config).await;

    // 3. Run the parallel sampling.
    let outcomes = run_jobs(session, claimed_candidates, stage_one_context).await;

    // 4. Metrics and logs.
    let counts = aggregate_stats(outcomes);
    emit_metrics(session, &counts);
    info!(
        "memory stage-1 extraction complete: {} job(s) claimed, {} succeeded ({} with output, {} no output), {} failed",
        counts.claimed,
        counts.succeeded_with_output + counts.succeeded_no_output,
        counts.succeeded_with_output,
        counts.succeeded_no_output,
        counts.failed
    );
}

/// JSON schema used to constrain phase-1 model output.
pub fn output_schema() -> Value {
    json!({
        "type": "object",
        "properties": {
            "rollout_summary": { "type": "string" },
            "rollout_slug": { "type": ["string", "null"] },
            "raw_memory": { "type": "string" }
        },
        "required": ["rollout_summary", "rollout_slug", "raw_memory"],
        "additionalProperties": false
    })
}

impl RequestContext {
    pub(in crate::memories) fn from_turn_context(
        turn_context: &TurnContext,
        turn_metadata_header: Option<String>,
        model_info: ModelInfo,
    ) -> Self {
        Self {
            model_info,
            turn_metadata_header,
            otel_manager: turn_context.otel_manager.clone(),
            reasoning_effort: turn_context.reasoning_effort,
            reasoning_summary: turn_context.reasoning_summary,
        }
    }
}

async fn claim_startup_jobs(
    session: &Arc<Session>,
    memories_config: &MemoriesConfig,
) -> Option<Vec<codex_state::Stage1JobClaim>> {
    let Some(state_db) = session.services.state_db.as_deref() else {
        // This should not happen.
        warn!("state db unavailable while claiming phase-1 startup jobs; skipping");
        return None;
    };

    let allowed_sources = INTERACTIVE_SESSION_SOURCES
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    match state_db
        .claim_stage1_jobs_for_startup(
            session.conversation_id,
            codex_state::Stage1StartupClaimParams {
                scan_limit: phase_one::THREAD_SCAN_LIMIT,
                max_claimed: memories_config.max_rollouts_per_startup,
                max_age_days: memories_config.max_rollout_age_days,
                min_rollout_idle_hours: memories_config.min_rollout_idle_hours,
                allowed_sources: allowed_sources.as_slice(),
                lease_seconds: phase_one::JOB_LEASE_SECONDS,
            },
        )
        .await
    {
        Ok(claims) => Some(claims),
        Err(err) => {
            warn!("state db claim_stage1_jobs_for_startup failed during memories startup: {err}");
            session.services.otel_manager.counter(
                metrics::MEMORY_PHASE_ONE_JOBS,
                1,
                &[("status", "failed_claim")],
            );
            None
        }
    }
}

async fn build_request_context(session: &Arc<Session>, config: &Config) -> RequestContext {
    let model_name = config
        .memories
        .phase_1_model
        .clone()
        .unwrap_or(phase_one::MODEL.to_string());
    let model = session
        .services
        .models_manager
        .get_model_info(&model_name, config)
        .await;
    let turn_context = session.new_default_turn().await;
    RequestContext::from_turn_context(
        turn_context.as_ref(),
        turn_context.turn_metadata_state.current_header_value(),
        model,
    )
}

async fn run_jobs(
    session: &Arc<Session>,
    claimed_candidates: Vec<codex_state::Stage1JobClaim>,
    stage_one_context: RequestContext,
) -> Vec<JobResult> {
    futures::stream::iter(claimed_candidates.into_iter())
        .map(|claim| {
            let session = Arc::clone(session);
            let stage_one_context = stage_one_context.clone();
            async move { job::run(session.as_ref(), claim, &stage_one_context).await }
        })
        .buffer_unordered(phase_one::CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await
}

mod job {
    use super::*;

    pub(in crate::memories) async fn run(
        session: &Session,
        claim: codex_state::Stage1JobClaim,
        stage_one_context: &RequestContext,
    ) -> JobResult {
        let thread = claim.thread;
        let (stage_one_output, token_usage) = match sample(
            session,
            &thread.rollout_path,
            &thread.cwd,
            stage_one_context,
        )
        .await
        {
            Ok(output) => output,
            Err(reason) => {
                result::failed(
                    session,
                    thread.id,
                    &claim.ownership_token,
                    &reason.to_string(),
                )
                .await;
                return JobResult {
                    outcome: JobOutcome::Failed,
                    token_usage: None,
                };
            }
        };

        if stage_one_output.raw_memory.is_empty() || stage_one_output.rollout_summary.is_empty() {
            return JobResult {
                outcome: result::no_output(session, thread.id, &claim.ownership_token).await,
                token_usage,
            };
        }

        JobResult {
            outcome: result::success(
                session,
                thread.id,
                &claim.ownership_token,
                thread.updated_at.timestamp(),
                &stage_one_output.raw_memory,
                &stage_one_output.rollout_summary,
                stage_one_output.rollout_slug.as_deref(),
            )
            .await,
            token_usage,
        }
    }

    /// Extract the rollout and perform the actual sampling.
    async fn sample(
        session: &Session,
        rollout_path: &Path,
        rollout_cwd: &Path,
        stage_one_context: &RequestContext,
    ) -> anyhow::Result<(StageOneOutput, Option<TokenUsage>)> {
        let (rollout_items, _, _) = RolloutRecorder::load_rollout_items(rollout_path).await?;
        let rollout_contents = serialize_filtered_rollout_response_items(&rollout_items)?;

        let prompt = Prompt {
            input: vec![ResponseItem::Message {
                id: None,
                role: "user".to_string(),
                content: vec![ContentItem::InputText {
                    text: build_stage_one_input_message(
                        &stage_one_context.model_info,
                        rollout_path,
                        rollout_cwd,
                        &rollout_contents,
                    )?,
                }],
                end_turn: None,
                phase: None,
            }],
            tools: Vec::new(),
            parallel_tool_calls: false,
            base_instructions: BaseInstructions {
                text: phase_one::PROMPT.to_string(),
            },
            personality: None,
            output_schema: Some(output_schema()),
        };

        let mut client_session = session.services.model_client.new_session();
        let mut stream = client_session
            .stream(
                &prompt,
                &stage_one_context.model_info,
                &stage_one_context.otel_manager,
                stage_one_context.reasoning_effort,
                stage_one_context.reasoning_summary,
                stage_one_context.turn_metadata_header.as_deref(),
            )
            .await?;

        // TODO(jif) we should have a shared helper somewhere for this.
        // Unwrap the stream.
        let mut result = String::new();
        let mut token_usage = None;
        while let Some(message) = stream.next().await.transpose()? {
            match message {
                ResponseEvent::OutputTextDelta(delta) => result.push_str(&delta),
                ResponseEvent::OutputItemDone(item) => {
                    if result.is_empty()
                        && let ResponseItem::Message { content, .. } = item
                        && let Some(text) = crate::compact::content_items_to_text(&content)
                    {
                        result.push_str(&text);
                    }
                }
                ResponseEvent::Completed {
                    token_usage: usage, ..
                } => {
                    token_usage = usage;
                    break;
                }
                _ => {}
            }
        }

        let mut output: StageOneOutput = serde_json::from_str(&result)?;
        output.raw_memory = redact_secrets(output.raw_memory);
        output.rollout_summary = redact_secrets(output.rollout_summary);
        output.rollout_slug = output.rollout_slug.map(redact_secrets);

        Ok((output, token_usage))
    }

    mod result {
        use super::*;

        pub(in crate::memories) async fn failed(
            session: &Session,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
            reason: &str,
        ) {
            tracing::warn!("Phase 1 job failed for thread {thread_id}: {reason}");
            if let Some(state_db) = session.services.state_db.as_deref() {
                let _ = state_db
                    .mark_stage1_job_failed(
                        thread_id,
                        ownership_token,
                        reason,
                        phase_one::JOB_RETRY_DELAY_SECONDS,
                    )
                    .await;
            }
        }

        pub(in crate::memories) async fn no_output(
            session: &Session,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
        ) -> JobOutcome {
            let Some(state_db) = session.services.state_db.as_deref() else {
                return JobOutcome::Failed;
            };

            if state_db
                .mark_stage1_job_succeeded_no_output(thread_id, ownership_token)
                .await
                .unwrap_or(false)
            {
                JobOutcome::SucceededNoOutput
            } else {
                JobOutcome::Failed
            }
        }

        pub(in crate::memories) async fn success(
            session: &Session,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
            source_updated_at: i64,
            raw_memory: &str,
            rollout_summary: &str,
            rollout_slug: Option<&str>,
        ) -> JobOutcome {
            let Some(state_db) = session.services.state_db.as_deref() else {
                return JobOutcome::Failed;
            };

            if state_db
                .mark_stage1_job_succeeded(
                    thread_id,
                    ownership_token,
                    source_updated_at,
                    raw_memory,
                    rollout_summary,
                    rollout_slug,
                )
                .await
                .unwrap_or(false)
            {
                JobOutcome::SucceededWithOutput
            } else {
                JobOutcome::Failed
            }
        }
    }

    /// Serializes filtered stage-1 memory items for prompt inclusion.
    fn serialize_filtered_rollout_response_items(
        items: &[RolloutItem],
    ) -> crate::error::Result<String> {
        let filtered = items
            .iter()
            .filter_map(|item| {
                if let RolloutItem::ResponseItem(item) = item
                    && should_persist_response_item_for_memories(item)
                {
                    Some(item.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        serde_json::to_string(&filtered).map_err(|err| {
            CodexErr::InvalidRequest(format!("failed to serialize rollout memory: {err}"))
        })
    }
}

fn aggregate_stats(outcomes: Vec<JobResult>) -> Stats {
    let claimed = outcomes.len();
    let mut succeeded_with_output = 0;
    let mut succeeded_no_output = 0;
    let mut failed = 0;
    let mut total_token_usage = TokenUsage::default();
    let mut has_token_usage = false;

    for outcome in outcomes {
        match outcome.outcome {
            JobOutcome::SucceededWithOutput => succeeded_with_output += 1,
            JobOutcome::SucceededNoOutput => succeeded_no_output += 1,
            JobOutcome::Failed => failed += 1,
        }

        if let Some(token_usage) = outcome.token_usage {
            total_token_usage.add_assign(&token_usage);
            has_token_usage = true;
        }
    }

    Stats {
        claimed,
        succeeded_with_output,
        succeeded_no_output,
        failed,
        total_token_usage: has_token_usage.then_some(total_token_usage),
    }
}

fn emit_metrics(session: &Session, counts: &Stats) {
    if counts.claimed > 0 {
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            counts.claimed as i64,
            &[("status", "claimed")],
        );
    }
    if counts.succeeded_with_output > 0 {
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            counts.succeeded_with_output as i64,
            &[("status", "succeeded")],
        );
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_OUTPUT,
            counts.succeeded_with_output as i64,
            &[],
        );
    }
    if counts.succeeded_no_output > 0 {
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            counts.succeeded_no_output as i64,
            &[("status", "succeeded_no_output")],
        );
    }
    if counts.failed > 0 {
        session.services.otel_manager.counter(
            metrics::MEMORY_PHASE_ONE_JOBS,
            counts.failed as i64,
            &[("status", "failed")],
        );
    }
    if let Some(token_usage) = counts.total_token_usage.as_ref() {
        session.services.otel_manager.histogram(
            metrics::MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.total_tokens.max(0),
            &[("token_type", "total")],
        );
        session.services.otel_manager.histogram(
            metrics::MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.input_tokens.max(0),
            &[("token_type", "input")],
        );
        session.services.otel_manager.histogram(
            metrics::MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.cached_input(),
            &[("token_type", "cached_input")],
        );
        session.services.otel_manager.histogram(
            metrics::MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.output_tokens.max(0),
            &[("token_type", "output")],
        );
        session.services.otel_manager.histogram(
            metrics::MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.reasoning_output_tokens.max(0),
            &[("token_type", "reasoning_output")],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::JobOutcome;
    use super::JobResult;
    use super::aggregate_stats;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;

    #[test]
    fn count_outcomes_sums_token_usage_across_all_jobs() {
        let counts = aggregate_stats(vec![
            JobResult {
                outcome: JobOutcome::SucceededWithOutput,
                token_usage: Some(TokenUsage {
                    input_tokens: 10,
                    cached_input_tokens: 2,
                    output_tokens: 3,
                    reasoning_output_tokens: 1,
                    total_tokens: 13,
                }),
            },
            JobResult {
                outcome: JobOutcome::SucceededNoOutput,
                token_usage: Some(TokenUsage {
                    input_tokens: 7,
                    cached_input_tokens: 1,
                    output_tokens: 2,
                    reasoning_output_tokens: 0,
                    total_tokens: 9,
                }),
            },
            JobResult {
                outcome: JobOutcome::Failed,
                token_usage: None,
            },
        ]);

        assert_eq!(counts.claimed, 3);
        assert_eq!(counts.succeeded_with_output, 1);
        assert_eq!(counts.succeeded_no_output, 1);
        assert_eq!(counts.failed, 1);
        assert_eq!(
            counts.total_token_usage,
            Some(TokenUsage {
                input_tokens: 17,
                cached_input_tokens: 3,
                output_tokens: 5,
                reasoning_output_tokens: 1,
                total_tokens: 22,
            })
        );
    }

    #[test]
    fn count_outcomes_keeps_usage_empty_when_no_job_reports_it() {
        let counts = aggregate_stats(vec![
            JobResult {
                outcome: JobOutcome::SucceededWithOutput,
                token_usage: None,
            },
            JobResult {
                outcome: JobOutcome::Failed,
                token_usage: None,
            },
        ]);

        assert_eq!(counts.claimed, 2);
        assert_eq!(counts.total_token_usage, None);
    }
}
