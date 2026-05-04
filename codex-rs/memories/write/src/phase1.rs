use crate::build_stage_one_input_message;
use crate::metrics::MEMORY_PHASE_ONE_E2E_MS;
use crate::metrics::MEMORY_PHASE_ONE_JOBS;
use crate::metrics::MEMORY_PHASE_ONE_OUTPUT;
use crate::metrics::MEMORY_PHASE_ONE_TOKEN_USAGE;
use crate::runtime::MemoryStartupContext;
use crate::runtime::StageOneRequestContext;
use codex_config::types::MemoriesConfig;
use codex_core::Prompt;
use codex_core::RolloutRecorder;
use codex_core::config::Config;
use codex_protocol::error::CodexErr;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::TokenUsage;
use codex_rollout::INTERACTIVE_SESSION_SOURCES;
use codex_rollout::should_persist_response_item_for_memories;
use codex_secrets::redact_secrets;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use tracing::info;
use tracing::warn;

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
pub async fn run(context: Arc<MemoryStartupContext>, config: Arc<Config>) {
    let stage_one_context = build_request_context(context.as_ref(), config.as_ref()).await;
    let _phase_one_e2e_timer = stage_one_context.start_timer(MEMORY_PHASE_ONE_E2E_MS);

    // 1. Claim startup job.
    let Some(claimed_candidates) = claim_startup_jobs(context.as_ref(), &config.memories).await
    else {
        return;
    };
    if claimed_candidates.is_empty() {
        stage_one_context.counter(
            MEMORY_PHASE_ONE_JOBS,
            /*inc*/ 1,
            &[("status", "skipped_no_candidates")],
        );
        return;
    }

    // 3. Run the parallel sampling.
    let outcomes = run_jobs(
        context,
        config,
        claimed_candidates,
        stage_one_context.clone(),
    )
    .await;

    // 4. Metrics and logs.
    let counts = aggregate_stats(outcomes);
    emit_metrics(&stage_one_context, &counts);
    info!(
        "memory stage-1 extraction complete: {} job(s) claimed, {} succeeded ({} with output, {} no output), {} failed",
        counts.claimed,
        counts.succeeded_with_output + counts.succeeded_no_output,
        counts.succeeded_with_output,
        counts.succeeded_no_output,
        counts.failed
    );
}

/// Prune old un-used "dead" raw memories.
pub async fn prune(context: &MemoryStartupContext, config: &Config) {
    if let Some(db) = context.state_db() {
        let max_unused_days = config.memories.max_unused_days;
        match db
            .prune_stage1_outputs_for_retention(max_unused_days, crate::stage_one::PRUNE_BATCH_SIZE)
            .await
        {
            Ok(pruned) => {
                if pruned > 0 {
                    info!(
                        "memory startup pruned {pruned} stale stage-1 output row(s) older than {max_unused_days} days"
                    );
                }
            }
            Err(err) => {
                warn!(
                    "state db prune_stage1_outputs_for_retention failed during memories startup: {err}"
                );
            }
        }
    }
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

async fn claim_startup_jobs(
    context: &MemoryStartupContext,
    memories_config: &MemoriesConfig,
) -> Option<Vec<codex_state::Stage1JobClaim>> {
    let Some(state_db) = context.state_db() else {
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
            context.thread_id(),
            codex_state::Stage1StartupClaimParams {
                scan_limit: crate::stage_one::THREAD_SCAN_LIMIT,
                max_claimed: memories_config.max_rollouts_per_startup,
                max_age_days: memories_config.max_rollout_age_days,
                min_rollout_idle_hours: memories_config.min_rollout_idle_hours,
                allowed_sources: allowed_sources.as_slice(),
                lease_seconds: crate::stage_one::JOB_LEASE_SECONDS,
            },
        )
        .await
    {
        Ok(claims) => Some(claims),
        Err(err) => {
            warn!("state db claim_stage1_jobs_for_startup failed during memories startup: {err}");
            None
        }
    }
}

async fn build_request_context(
    context: &MemoryStartupContext,
    config: &Config,
) -> StageOneRequestContext {
    let model_name = config
        .memories
        .extract_model
        .clone()
        .unwrap_or(crate::stage_one::MODEL.to_string());
    context
        .stage_one_request_context(config, &model_name, crate::stage_one::REASONING_EFFORT)
        .await
}

async fn run_jobs(
    context: Arc<MemoryStartupContext>,
    config: Arc<Config>,
    claimed_candidates: Vec<codex_state::Stage1JobClaim>,
    stage_one_context: StageOneRequestContext,
) -> Vec<JobResult> {
    futures::stream::iter(claimed_candidates.into_iter())
        .map(|claim| {
            let context = Arc::clone(&context);
            let config = Arc::clone(&config);
            let stage_one_context = stage_one_context.clone();
            async move {
                job::run(context.as_ref(), config.as_ref(), claim, &stage_one_context).await
            }
        })
        .buffer_unordered(crate::stage_one::CONCURRENCY_LIMIT)
        .collect::<Vec<_>>()
        .await
}

mod job {
    use super::*;

    pub(crate) async fn run(
        context: &MemoryStartupContext,
        config: &Config,
        claim: codex_state::Stage1JobClaim,
        stage_one_context: &StageOneRequestContext,
    ) -> JobResult {
        let claimed_thread = claim.thread;
        let (stage_one_output, token_usage) = match sample(
            context,
            config,
            &claimed_thread.rollout_path,
            &claimed_thread.cwd,
            stage_one_context,
        )
        .await
        {
            Ok(output) => output,
            Err(reason) => {
                result::failed(
                    context,
                    claimed_thread.id,
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
                outcome: result::no_output(context, claimed_thread.id, &claim.ownership_token)
                    .await,
                token_usage,
            };
        }

        JobResult {
            outcome: result::success(
                context,
                claimed_thread.id,
                &claim.ownership_token,
                claimed_thread.updated_at.timestamp(),
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
        context: &MemoryStartupContext,
        config: &Config,
        rollout_path: &Path,
        rollout_cwd: &Path,
        stage_one_context: &StageOneRequestContext,
    ) -> anyhow::Result<(StageOneOutput, Option<TokenUsage>)> {
        let (rollout_items, _, _) = RolloutRecorder::load_rollout_items(rollout_path).await?;
        let rollout_contents = serialize_filtered_rollout_response_items(&rollout_items)?;

        let mut prompt = Prompt::default();
        prompt.input = vec![ResponseItem::Message {
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
            phase: None,
        }];
        prompt.base_instructions = BaseInstructions {
            text: crate::stage_one::PROMPT.to_string(),
        };
        prompt.output_schema = Some(output_schema());
        prompt.output_schema_strict = true;

        let (result, token_usage) = context
            .stream_stage_one_prompt(config, &prompt, stage_one_context)
            .await?;

        let mut output: StageOneOutput = serde_json::from_str(&result)?;
        output.raw_memory = redact_secrets(output.raw_memory);
        output.rollout_summary = redact_secrets(output.rollout_summary);
        output.rollout_slug = output.rollout_slug.map(redact_secrets);

        Ok((output, token_usage))
    }

    mod result {
        use super::*;

        pub(crate) async fn failed(
            context: &MemoryStartupContext,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
            reason: &str,
        ) {
            tracing::warn!("Phase 1 job failed for thread {thread_id}: {reason}");
            if let Some(state_db) = context.state_db() {
                let _ = state_db
                    .mark_stage1_job_failed(
                        thread_id,
                        ownership_token,
                        reason,
                        crate::stage_one::JOB_RETRY_DELAY_SECONDS,
                    )
                    .await;
            }
        }

        pub(crate) async fn no_output(
            context: &MemoryStartupContext,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
        ) -> JobOutcome {
            let Some(state_db) = context.state_db() else {
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

        pub(crate) async fn success(
            context: &MemoryStartupContext,
            thread_id: codex_protocol::ThreadId,
            ownership_token: &str,
            source_updated_at: i64,
            raw_memory: &str,
            rollout_summary: &str,
            rollout_slug: Option<&str>,
        ) -> JobOutcome {
            let Some(state_db) = context.state_db() else {
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
    pub(super) fn serialize_filtered_rollout_response_items(
        items: &[RolloutItem],
    ) -> codex_protocol::error::Result<String> {
        let filtered = items
            .iter()
            .filter_map(|item| {
                if let RolloutItem::ResponseItem(item) = item {
                    sanitize_response_item_for_memories(item)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        let serialized = serde_json::to_string(&filtered).map_err(|err| {
            CodexErr::InvalidRequest(format!("failed to serialize rollout memory: {err}"))
        })?;
        Ok(redact_secrets(serialized))
    }

    fn sanitize_response_item_for_memories(item: &ResponseItem) -> Option<ResponseItem> {
        let ResponseItem::Message {
            id,
            role,
            content,
            phase,
        } = item
        else {
            return should_persist_response_item_for_memories(item).then(|| item.clone());
        };

        if role == "developer" {
            return None;
        }

        if role != "user" {
            return Some(item.clone());
        }

        let content = content
            .iter()
            .filter(|content_item| !is_memory_excluded_contextual_user_fragment(content_item))
            .cloned()
            .collect::<Vec<_>>();
        if content.is_empty() {
            return None;
        }

        Some(ResponseItem::Message {
            id: id.clone(),
            role: role.clone(),
            content,
            phase: phase.clone(),
        })
    }

    fn is_memory_excluded_contextual_user_fragment(content_item: &ContentItem) -> bool {
        let ContentItem::InputText { text } = content_item else {
            return false;
        };

        matches_marked_fragment(text, "# AGENTS.md instructions for ", "</INSTRUCTIONS>")
            || matches_marked_fragment(text, "<skill>", "</skill>")
    }

    fn matches_marked_fragment(text: &str, start_marker: &str, end_marker: &str) -> bool {
        let trimmed = text.trim_start();
        let starts_with_marker = trimmed
            .get(..start_marker.len())
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(start_marker));
        let trimmed = trimmed.trim_end();
        let ends_with_marker = trimmed
            .get(trimmed.len().saturating_sub(end_marker.len())..)
            .is_some_and(|candidate| candidate.eq_ignore_ascii_case(end_marker));
        starts_with_marker && ends_with_marker
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn classifies_memory_excluded_fragments() {
            let cases = [
                (
                    "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>",
                    true,
                ),
                (
                    "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>",
                    true,
                ),
                (
                    "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>",
                    false,
                ),
                (
                    "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>",
                    false,
                ),
            ];

            for (text, expected) in cases {
                assert_eq!(
                    is_memory_excluded_contextual_user_fragment(&ContentItem::InputText {
                        text: text.to_string(),
                    }),
                    expected,
                    "{text}",
                );
            }
        }

        #[test]
        fn output_schema_requires_rollout_slug_and_keeps_it_nullable() {
            let schema = output_schema();
            let properties = schema
                .get("properties")
                .and_then(Value::as_object)
                .expect("properties object");
            let required = schema
                .get("required")
                .and_then(Value::as_array)
                .expect("required array");

            let mut required_keys = required
                .iter()
                .map(|key| key.as_str().expect("required key string"))
                .collect::<Vec<_>>();
            required_keys.sort_unstable();

            assert!(
                properties.contains_key("rollout_slug"),
                "schema should declare rollout_slug"
            );

            let rollout_slug_type = properties
                .get("rollout_slug")
                .and_then(Value::as_object)
                .and_then(|entry| entry.get("type"))
                .and_then(Value::as_array)
                .expect("rollout_slug type array");
            let mut rollout_slug_types = rollout_slug_type
                .iter()
                .map(|entry| entry.as_str().expect("type entry string"))
                .collect::<Vec<_>>();
            rollout_slug_types.sort_unstable();

            assert_eq!(
                required_keys,
                vec!["raw_memory", "rollout_slug", "rollout_summary"]
            );
            assert_eq!(rollout_slug_types, vec!["null", "string"]);
        }
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

fn emit_metrics(context: &StageOneRequestContext, counts: &Stats) {
    if counts.claimed > 0 {
        context.counter(
            MEMORY_PHASE_ONE_JOBS,
            counts.claimed as i64,
            &[("status", "claimed")],
        );
    }
    if counts.succeeded_with_output > 0 {
        context.counter(
            MEMORY_PHASE_ONE_JOBS,
            counts.succeeded_with_output as i64,
            &[("status", "succeeded")],
        );
        context.counter(
            MEMORY_PHASE_ONE_OUTPUT,
            counts.succeeded_with_output as i64,
            &[],
        );
    }
    if counts.succeeded_no_output > 0 {
        context.counter(
            MEMORY_PHASE_ONE_JOBS,
            counts.succeeded_no_output as i64,
            &[("status", "succeeded_no_output")],
        );
    }
    if counts.failed > 0 {
        context.counter(
            MEMORY_PHASE_ONE_JOBS,
            counts.failed as i64,
            &[("status", "failed")],
        );
    }
    if let Some(token_usage) = counts.total_token_usage.as_ref() {
        context.histogram(
            MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.total_tokens.max(0),
            &[("token_type", "total")],
        );
        context.histogram(
            MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.input_tokens.max(0),
            &[("token_type", "input")],
        );
        context.histogram(
            MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.cached_input(),
            &[("token_type", "cached_input")],
        );
        context.histogram(
            MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.output_tokens.max(0),
            &[("token_type", "output")],
        );
        context.histogram(
            MEMORY_PHASE_ONE_TOKEN_USAGE,
            token_usage.reasoning_output_tokens.max(0),
            &[("token_type", "reasoning_output")],
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn serializes_memory_rollout_with_agents_removed_but_environment_kept() {
        let mixed_contextual_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![
                ContentItem::InputText {
                    text:
                        "# AGENTS.md instructions for /tmp\n\n<INSTRUCTIONS>\nbody\n</INSTRUCTIONS>"
                            .to_string(),
                },
                ContentItem::InputText {
                    text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>"
                        .to_string(),
                },
            ],
            phase: None,
        };
        let skill_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text:
                    "<skill>\n<name>demo</name>\n<path>skills/demo/SKILL.md</path>\nbody\n</skill>"
                        .to_string(),
            }],
            phase: None,
        };
        let subagent_message = ResponseItem::Message {
            id: None,
            role: "user".to_string(),
            content: vec![ContentItem::InputText {
                text: "<subagent_notification>{\"agent_id\":\"a\",\"status\":\"completed\"}</subagent_notification>"
                    .to_string(),
            }],
            phase: None,
        };

        let serialized = job::serialize_filtered_rollout_response_items(&[
            RolloutItem::ResponseItem(mixed_contextual_message),
            RolloutItem::ResponseItem(skill_message),
            RolloutItem::ResponseItem(subagent_message.clone()),
        ])
        .expect("serialize");
        let parsed: Vec<ResponseItem> = serde_json::from_str(&serialized).expect("parse");

        assert_eq!(
            parsed,
            vec![
                ResponseItem::Message {
                    id: None,
                    role: "user".to_string(),
                    content: vec![ContentItem::InputText {
                        text: "<environment_context>\n<cwd>/tmp</cwd>\n</environment_context>"
                            .to_string(),
                    }],
                    phase: None,
                },
                subagent_message,
            ]
        );
    }

    #[test]
    fn serializes_memory_rollout_redacts_secrets_before_prompt_upload() {
        let serialized =
            job::serialize_filtered_rollout_response_items(&[RolloutItem::ResponseItem(
                ResponseItem::FunctionCallOutput {
                    call_id: "call_123".to_string(),
                    output: codex_protocol::models::FunctionCallOutputPayload {
                        body: codex_protocol::models::FunctionCallOutputBody::Text(
                            r#"{"token":"sk-abcdefghijklmnopqrstuvwxyz123456"}"#.to_string(),
                        ),
                        success: Some(true),
                    },
                },
            )])
            .expect("serialize");

        assert!(!serialized.contains("sk-abcdefghijklmnopqrstuvwxyz123456"));
        assert!(serialized.contains("[REDACTED_SECRET]"));
    }

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
