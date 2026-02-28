use crate::Prompt;
use crate::RolloutRecorder;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::config::types::MemoriesConfig;
use crate::config::types::MemoriesStageOneSource;
use crate::error::CodexErr;
use crate::memories::metrics;
use crate::memories::phase_one;
use crate::memories::prompts::build_stage_one_input_message;
use crate::rollout::policy::should_persist_response_item_for_memories;
use chrono::DateTime;
use chrono::Utc;
use codex_api::ResponseEvent;
use codex_otel::OtelManager;
use codex_protocol::ThreadId;
use codex_protocol::config_types::ReasoningSummary as ReasoningSummaryConfig;
use codex_protocol::models::BaseInstructions;
use codex_protocol::models::ContentItem;
use codex_protocol::models::ResponseItem;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ReasoningEffort as ReasoningEffortConfig;
use codex_protocol::protocol::RolloutItem;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::TokenUsage;
use codex_secrets::redact_secrets;
use codex_state::ThreadMetadataBuilder;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::Value;
use serde_json::json;
use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::info;
use tracing::warn;
use uuid::Uuid;

const SCRATCHPAD_SOURCE: &str = "scratchpad";
const SCRATCHPAD_HEADER: &str = "# Codex Scratchpad";
const SCRATCHPAD_MAX_ENTRIES: usize = 120;
const SCRATCHPAD_MAX_GRAPH_EDGES: usize = 600;
const SCRATCHPAD_MAX_SUMMARY_CHARS: usize = 220;

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

#[derive(Debug, Clone, PartialEq, Eq)]
struct ScratchpadEntry {
    id: String,
    kind: String,
    tags: Vec<String>,
    text: String,
    refs: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ScratchpadDocument {
    task: Option<String>,
    session_id: Option<String>,
    created_at: Option<String>,
    entries: Vec<ScratchpadEntry>,
}

/// Runs memory phase 1 in strict step order:
/// 1) sync enabled repo-local scratchpad threads
/// 2) claim eligible rollout jobs
/// 3) build one stage-1 request context
/// 4) run stage-1 extraction jobs in parallel
/// 5) emit metrics and logs
pub(in crate::memories) async fn run(session: &Arc<Session>, config: &Config) {
    let _phase_one_e2e_timer = session
        .services
        .otel_manager
        .start_timer(metrics::MEMORY_PHASE_ONE_E2E_MS, &[])
        .ok();

    // 1. Sync repo-local scratchpad artifacts into state DB when enabled.
    if let Err(err) = sync_scratchpad_threads(session, config).await {
        warn!("memory stage-1 scratchpad sync failed: {err}");
    }

    // 2. Claim startup job.
    let Some(claimed_candidates) = claim_startup_jobs(session, config).await else {
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

    // 3. Build request.
    let stage_one_context = build_request_context(session, config).await;

    // 4. Run the parallel sampling.
    let outcomes = run_jobs(session, claimed_candidates, stage_one_context).await;

    // 5. Metrics and logs.
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
            reasoning_effort: Some(phase_one::REASONING_EFFORT),
            reasoning_summary: turn_context.reasoning_summary,
        }
    }
}

async fn claim_startup_jobs(
    session: &Arc<Session>,
    config: &Config,
) -> Option<Vec<codex_state::Stage1JobClaim>> {
    let memories_config = &config.memories;
    let Some(state_db) = session.services.state_db.as_deref() else {
        // This should not happen.
        warn!("state db unavailable while claiming phase-1 startup jobs; skipping");
        return None;
    };

    let scratchpad_source_enabled = has_scratchpad_stage_one_source(memories_config);
    let scratchpad_cwd = if scratchpad_source_enabled {
        find_repo_root_with_scratchpad(&config.cwd)
            .map(|repo_root| dunce::canonicalize(&repo_root).unwrap_or(repo_root))
            .map(|repo_root| repo_root.to_string_lossy().to_string())
    } else {
        None
    };

    let mut allowed_sources = memories_config
        .stage_1_sources
        .iter()
        .map(|source| source.as_db_source_str().to_string())
        .collect::<Vec<_>>();

    if scratchpad_source_enabled && scratchpad_cwd.is_none() {
        allowed_sources.retain(|source| source != SCRATCHPAD_SOURCE);
    }

    if allowed_sources.is_empty() {
        return Some(Vec::new());
    }

    match state_db
        .claim_stage1_jobs_for_startup(
            session.conversation_id,
            codex_state::Stage1StartupClaimParams {
                scan_limit: phase_one::THREAD_SCAN_LIMIT,
                max_claimed: memories_config.max_rollouts_per_startup,
                max_age_days: memories_config.max_rollout_age_days,
                min_rollout_idle_hours: memories_config.min_rollout_idle_hours,
                allowed_sources: allowed_sources.as_slice(),
                scratchpad_cwd: scratchpad_cwd.as_deref(),
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

#[derive(Clone, Debug)]
struct ScratchpadCandidate {
    path: PathBuf,
    updated_at: DateTime<Utc>,
}

async fn sync_scratchpad_threads(session: &Arc<Session>, config: &Config) -> anyhow::Result<()> {
    if !has_scratchpad_stage_one_source(&config.memories) {
        return Ok(());
    }

    let Some(state_db) = session.services.state_db.as_deref() else {
        return Ok(());
    };

    let Some(repo_root) = find_repo_root_with_scratchpad(&config.cwd) else {
        return Ok(());
    };

    let candidates = discover_scratchpad_candidates(&repo_root, &config.memories)?;
    if candidates.is_empty() {
        return Ok(());
    }

    for candidate in candidates {
        let canonical_path =
            dunce::canonicalize(&candidate.path).unwrap_or_else(|_| candidate.path.clone());
        let thread_uuid = Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            canonical_path.to_string_lossy().as_bytes(),
        );
        let Ok(thread_id) = ThreadId::from_string(&thread_uuid.to_string()) else {
            warn!(
                "memory stage-1 scratchpad sync skipped invalid synthetic thread id for {}",
                candidate.path.display()
            );
            continue;
        };

        let mut metadata_builder = ThreadMetadataBuilder::new(
            thread_id,
            candidate.path.clone(),
            candidate.updated_at,
            SessionSource::Exec,
        );
        metadata_builder.updated_at = Some(candidate.updated_at);
        metadata_builder.model_provider = Some(config.model_provider_id.clone());
        metadata_builder.cwd = repo_root.clone();
        metadata_builder.cli_version = Some(env!("CARGO_PKG_VERSION").to_string());

        let mut metadata = metadata_builder.build(config.model_provider_id.as_str());
        metadata.source = SCRATCHPAD_SOURCE.to_string();
        metadata.title = scratchpad_title_from_path(candidate.path.as_path());
        metadata.first_user_message = Some(format!("scratchpad: {}", metadata.title));

        if let Err(err) = state_db.upsert_thread(&metadata).await {
            warn!(
                "memory stage-1 scratchpad sync failed upserting {}: {err}",
                candidate.path.display()
            );
        }
    }

    Ok(())
}

fn has_scratchpad_stage_one_source(memories_config: &MemoriesConfig) -> bool {
    memories_config
        .stage_1_sources
        .contains(&MemoriesStageOneSource::Scratchpad)
}

fn find_repo_root_with_scratchpad(cwd: &Path) -> Option<PathBuf> {
    cwd.ancestors()
        .find(|ancestor| ancestor.join("scratch").join("codex").is_dir())
        .map(Path::to_path_buf)
}

fn discover_scratchpad_candidates(
    repo_root: &Path,
    memories_config: &MemoriesConfig,
) -> std::io::Result<Vec<ScratchpadCandidate>> {
    let now = Utc::now().timestamp();
    let min_updated_at = now.saturating_sub(
        memories_config
            .max_rollout_age_days
            .saturating_mul(24)
            .saturating_mul(3_600),
    );
    let max_updated_at =
        now.saturating_sub(memories_config.min_rollout_idle_hours.saturating_mul(3_600));

    let mut candidates = Vec::new();
    for relative_dir in [
        Path::new("scratch").join("codex").join("pads"),
        Path::new("scratch").join("codex").join("backups"),
    ] {
        let directory = repo_root.join(relative_dir);
        let Ok(entries) = std::fs::read_dir(directory) else {
            continue;
        };

        for entry in entries {
            let Ok(entry) = entry else {
                continue;
            };
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
                continue;
            }

            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            let Ok(modified) = metadata.modified() else {
                continue;
            };
            let updated_at: DateTime<Utc> = DateTime::<Utc>::from(modified);
            let updated_at_ts = updated_at.timestamp();
            if updated_at_ts < min_updated_at || updated_at_ts > max_updated_at {
                continue;
            }

            let Ok(contents) = std::fs::read_to_string(&path) else {
                continue;
            };
            if !contents.starts_with(SCRATCHPAD_HEADER) {
                continue;
            }

            candidates.push(ScratchpadCandidate { path, updated_at });
        }
    }

    candidates.sort_by(|a, b| {
        b.updated_at
            .cmp(&a.updated_at)
            .then_with(|| a.path.cmp(&b.path))
    });
    candidates.truncate(phase_one::THREAD_SCAN_LIMIT);

    Ok(candidates)
}

fn scratchpad_title_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .filter(|stem| !stem.is_empty())
        .unwrap_or("scratchpad")
        .to_string()
}

fn redact_stage_one_output(mut output: StageOneOutput) -> StageOneOutput {
    output.raw_memory = redact_secrets(output.raw_memory);
    output.rollout_summary = redact_secrets(output.rollout_summary);
    output.rollout_slug = output.rollout_slug.map(redact_secrets);
    output
}

fn parse_scratchpad_document(contents: &str) -> Option<ScratchpadDocument> {
    let mut lines = contents.lines();
    if lines.next().map(str::trim) != Some(SCRATCHPAD_HEADER) {
        return None;
    }

    let mut document = ScratchpadDocument::default();
    for line in lines {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if let Some(entry) = parse_scratchpad_entry(trimmed) {
            document.entries.push(entry);
            continue;
        }

        if let Some((key, value)) = trimmed.split_once(':') {
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key.trim() {
                "task" => document.task = Some(value.to_string()),
                "session_id" => document.session_id = Some(value.to_string()),
                "created_at" => document.created_at = Some(value.to_string()),
                _ => {}
            }
        }
    }

    Some(document)
}

fn parse_scratchpad_entry(line: &str) -> Option<ScratchpadEntry> {
    let payload = line.strip_prefix("- ")?;
    let (prefix, text_with_refs) = payload.split_once(" :: ")?;

    let mut prefix_parts = prefix.split_whitespace();
    let id = prefix_parts.next()?.to_string();
    if !id.starts_with("CSP#") {
        return None;
    }

    let kind = prefix_parts.next()?.to_string();
    if kind.is_empty() {
        return None;
    }

    let tags = prefix_parts
        .filter_map(|token| {
            token
                .strip_prefix('#')
                .filter(|tag| !tag.is_empty())
                .map(str::to_string)
        })
        .collect::<Vec<_>>();

    let (text, refs) = split_scratchpad_entry_text_and_refs(text_with_refs.trim());
    if text.is_empty() {
        return None;
    }

    Some(ScratchpadEntry {
        id,
        kind,
        tags,
        text,
        refs,
    })
}

fn split_scratchpad_entry_text_and_refs(text: &str) -> (String, Vec<String>) {
    if let Some((body, refs_with_paren)) = text.rsplit_once(" (refs: ")
        && let Some(refs_body) = refs_with_paren.strip_suffix(')')
    {
        let refs = refs_body
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
            .collect::<Vec<_>>();
        return (normalize_inline_text(body), refs);
    }

    (normalize_inline_text(text), Vec::new())
}

fn normalize_inline_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn parse_lot_field(text: &str, field_name: &str) -> Option<String> {
    text.split('|').find_map(|part| {
        let part = part.trim();
        if let Some((key, value)) = part.split_once('=')
            && key.trim().eq_ignore_ascii_case(field_name)
        {
            let value = normalize_inline_text(value.trim());
            if !value.is_empty() {
                return Some(value);
            }
        }
        None
    })
}

fn truncate_for_summary(text: &str, max_chars: usize) -> String {
    let text = normalize_inline_text(text);
    if text.chars().count() <= max_chars {
        return text;
    }

    let mut truncated = text.chars().take(max_chars).collect::<String>();
    truncated.push_str("...");
    truncated
}

fn build_scratchpad_rollout_summary(document: &ScratchpadDocument, fallback_title: &str) -> String {
    let title = document
        .task
        .as_deref()
        .filter(|task| !task.trim().is_empty())
        .unwrap_or(fallback_title);

    let Some(latest_entry) = document.entries.last() else {
        return format!("Scratchpad `{title}`: no CSP entries recorded yet.");
    };

    if latest_entry.kind.eq_ignore_ascii_case("LOT") {
        let conclusion = parse_lot_field(&latest_entry.text, "conc");
        let next = parse_lot_field(&latest_entry.text, "next");
        if conclusion.is_some() || next.is_some() {
            let mut summary = format!("Scratchpad `{title}`");
            if let Some(conclusion) = conclusion {
                let conclusion = truncate_for_summary(&conclusion, SCRATCHPAD_MAX_SUMMARY_CHARS);
                summary.push_str(&format!(" validated: {conclusion}"));
            }
            if let Some(next) = next {
                let next = truncate_for_summary(&next, SCRATCHPAD_MAX_SUMMARY_CHARS);
                summary.push_str(&format!("; next: {next}"));
            }
            return summary;
        }
    }

    let latest_text = truncate_for_summary(&latest_entry.text, SCRATCHPAD_MAX_SUMMARY_CHARS);
    format!(
        "Scratchpad `{title}` latest {}: {latest_text}",
        latest_entry.kind
    )
}

fn scratchpad_rollout_slug(document: &ScratchpadDocument, fallback_title: &str) -> Option<String> {
    let title = document
        .task
        .as_deref()
        .filter(|task| !task.trim().is_empty())
        .unwrap_or(fallback_title)
        .trim();
    if title.is_empty() {
        return None;
    }

    Some(format!("scratchpad-{title}"))
}

fn build_scratchpad_raw_memory(
    document: &ScratchpadDocument,
    scratchpad_path: &Path,
    source_updated_at: DateTime<Utc>,
    fallback_title: &str,
) -> String {
    let title = document
        .task
        .as_deref()
        .filter(|task| !task.trim().is_empty())
        .unwrap_or(fallback_title);
    let entries_start_index = document
        .entries
        .len()
        .saturating_sub(SCRATCHPAD_MAX_ENTRIES);
    let entries = &document.entries[entries_start_index..];

    let mut body = String::new();
    let _ = writeln!(body, "---");
    let _ = writeln!(body, "source: scratchpad");
    let _ = writeln!(body, "task: {title}");
    if let Some(session_id) = &document.session_id {
        let _ = writeln!(body, "session_id: {session_id}");
    }
    if let Some(created_at) = &document.created_at {
        let _ = writeln!(body, "created_at: {created_at}");
    }
    let _ = writeln!(body, "scratchpad_path: {}", scratchpad_path.display());
    let _ = writeln!(
        body,
        "source_updated_at: {}",
        source_updated_at.to_rfc3339()
    );
    let _ = writeln!(body, "entry_count: {}", document.entries.len());
    let _ = writeln!(body, "---");
    let _ = writeln!(body);

    body.push_str("## Scratchpad Trail\n");
    if entries.is_empty() {
        body.push_str("- No CSP entries found.\n\n");
    } else {
        for entry in entries {
            let tags = if entry.tags.is_empty() {
                String::new()
            } else {
                format!(" #{}", entry.tags.join(" #"))
            };
            let _ = writeln!(
                body,
                "- {} {}{} :: {}",
                entry.id, entry.kind, tags, entry.text
            );
            if !entry.refs.is_empty() {
                let _ = writeln!(body, "  refs: {}", entry.refs.join(", "));
            }
        }
        let _ = writeln!(body);
    }

    body.push_str("## Breadcrumb Graph\n");
    body.push_str("Nodes:\n");
    if entries.is_empty() {
        body.push_str("- none\n");
    } else {
        for entry in entries {
            let tags = if entry.tags.is_empty() {
                "none".to_string()
            } else {
                entry.tags.join(",")
            };
            let refs = if entry.refs.is_empty() {
                "none".to_string()
            } else {
                entry.refs.join(",")
            };
            let _ = writeln!(
                body,
                "- node={} kind={} tags=[{}] refs=[{}]",
                entry.id, entry.kind, tags, refs
            );
        }
    }

    body.push_str("Edges:\n");
    let mut edges = BTreeSet::new();
    for pair in entries.windows(2) {
        edges.insert(format!("{} -> {} (follows)", pair[0].id, pair[1].id));
    }
    for entry in entries {
        for tag in &entry.tags {
            edges.insert(format!("{} -> tag:{} (topic)", entry.id, tag));
        }
        for reference in &entry.refs {
            edges.insert(format!("{} -> ref:{} (touches)", entry.id, reference));
        }
    }

    if edges.is_empty() {
        body.push_str("- none\n");
    } else {
        for (emitted, edge) in edges.into_iter().enumerate() {
            if emitted >= SCRATCHPAD_MAX_GRAPH_EDGES {
                body.push_str("- ...\n");
                break;
            }
            let _ = writeln!(body, "- {edge}");
        }
    }

    body
}

fn scratchpad_stage_one_output(
    scratchpad_path: &Path,
    source_updated_at: DateTime<Utc>,
) -> anyhow::Result<StageOneOutput> {
    let contents = std::fs::read_to_string(scratchpad_path)?;
    let document = parse_scratchpad_document(&contents).ok_or_else(|| {
        anyhow::anyhow!(
            "scratchpad file does not match expected format: {}",
            scratchpad_path.display()
        )
    })?;
    let title = scratchpad_title_from_path(scratchpad_path);

    Ok(StageOneOutput {
        raw_memory: build_scratchpad_raw_memory(
            &document,
            scratchpad_path,
            source_updated_at,
            &title,
        ),
        rollout_summary: build_scratchpad_rollout_summary(&document, &title),
        rollout_slug: scratchpad_rollout_slug(&document, &title),
    })
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
        let (stage_one_output, token_usage) =
            match sample(session, &thread, stage_one_context).await {
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

    /// Extract a stage-1 output from either rollout JSONL or scratchpad markdown.
    async fn sample(
        session: &Session,
        thread: &codex_state::ThreadMetadata,
        stage_one_context: &RequestContext,
    ) -> anyhow::Result<(StageOneOutput, Option<TokenUsage>)> {
        if thread.source == SCRATCHPAD_SOURCE {
            let output = scratchpad_stage_one_output(&thread.rollout_path, thread.updated_at)?;
            return Ok((redact_stage_one_output(output), None));
        }

        let (output, token_usage) = sample_rollout(
            session,
            &thread.rollout_path,
            &thread.cwd,
            stage_one_context,
        )
        .await?;
        Ok((redact_stage_one_output(output), token_usage))
    }

    async fn sample_rollout(
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

        let output: StageOneOutput = serde_json::from_str(&result)?;
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
    use super::build_scratchpad_rollout_summary;
    use super::parse_scratchpad_document;
    use super::scratchpad_stage_one_output;
    use chrono::TimeZone;
    use chrono::Utc;
    use codex_protocol::protocol::TokenUsage;
    use pretty_assertions::assert_eq;
    use tempfile::tempdir;

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

    #[test]
    fn parse_scratchpad_document_extracts_metadata_and_entries() {
        let contents = "# Codex Scratchpad
 task: memory-sweep
 session_id: 019ca40f-d497-7783-9629-31903cea584e
 created_at: 2026-02-28T12:03:50Z
 format: - CSP#<id> <KIND> #tags :: text (refs: ...)
 - CSP#260228-120648Z-930b LOT #scratchpad #phase1 :: goal=Find issue | conc=Working parser | next=Add tests (refs: src/a.rs:10, src/b.rs:20)
 - CSP#260228-121006Z-c6ec FIX #memory :: Implemented branch
";

        let document = parse_scratchpad_document(contents).expect("scratchpad should parse");

        assert_eq!(document.task.as_deref(), Some("memory-sweep"));
        assert_eq!(
            document.session_id.as_deref(),
            Some("019ca40f-d497-7783-9629-31903cea584e")
        );
        assert_eq!(document.created_at.as_deref(), Some("2026-02-28T12:03:50Z"));
        assert_eq!(document.entries.len(), 2);
        assert_eq!(document.entries[0].id, "CSP#260228-120648Z-930b");
        assert_eq!(document.entries[0].kind, "LOT");
        assert_eq!(
            document.entries[0].tags,
            vec!["scratchpad".to_string(), "phase1".to_string()]
        );
        assert_eq!(
            document.entries[0].refs,
            vec!["src/a.rs:10".to_string(), "src/b.rs:20".to_string()]
        );
        assert_eq!(document.entries[1].kind, "FIX");
    }

    #[test]
    fn build_scratchpad_rollout_summary_uses_lot_conclusion_and_next() {
        let contents = "# Codex Scratchpad
 task: memory-sweep
 - CSP#260228-120648Z-930b LOT #scratchpad :: goal=Find issue | conc=Confirmed scratchpad branch | next=Run regression tests
";
        let document = parse_scratchpad_document(contents).expect("scratchpad should parse");

        let summary = build_scratchpad_rollout_summary(&document, "fallback-title");

        assert_eq!(
            summary,
            "Scratchpad `memory-sweep` validated: Confirmed scratchpad branch; next: Run regression tests"
        );
    }

    #[test]
    fn scratchpad_stage_one_output_contains_graph_breadcrumbs() {
        let temp = tempdir().expect("tempdir");
        let scratchpad_path = temp.path().join("csp__memory-sweep.md");
        std::fs::write(
            &scratchpad_path,
            "# Codex Scratchpad
 task: memory-sweep
 - CSP#1 LOT #alpha :: goal=One | conc=Done first | next=Two (refs: src/lib.rs:10)
 - CSP#2 FIX #alpha #beta :: Applied fix (refs: src/lib.rs:22)
",
        )
        .expect("write scratchpad");

        let updated_at = Utc
            .timestamp_opt(1_701_000_000, 0)
            .single()
            .expect("timestamp");
        let output =
            scratchpad_stage_one_output(&scratchpad_path, updated_at).expect("stage-one output");

        assert_eq!(
            output.rollout_summary,
            "Scratchpad `memory-sweep` latest FIX: Applied fix"
        );
        assert_eq!(
            output.rollout_slug.as_deref(),
            Some("scratchpad-memory-sweep")
        );
        assert!(output.raw_memory.contains("## Breadcrumb Graph"));
        assert!(output.raw_memory.contains("CSP#1 -> CSP#2 (follows)"));
        assert!(output.raw_memory.contains("CSP#2 -> tag:beta (topic)"));
        assert!(
            output
                .raw_memory
                .contains("CSP#2 -> ref:src/lib.rs:22 (touches)")
        );
    }
}
