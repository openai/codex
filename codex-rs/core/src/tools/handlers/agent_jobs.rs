use crate::agent::exceeds_thread_spawn_depth_limit;
use crate::agent::next_thread_spawn_depth;
use crate::agent::status::is_final;
use crate::codex::Session;
use crate::codex::TurnContext;
use crate::config::Config;
use crate::error::CodexErr;
use crate::function_tool::FunctionCallError;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolOutput;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::multi_agents::build_agent_spawn_config;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use codex_protocol::user_input::UserInput;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

pub struct BatchJobHandler;

const DEFAULT_AGENT_JOB_CONCURRENCY: usize = 16;
const MAX_AGENT_JOB_CONCURRENCY: usize = 64;
const STATUS_POLL_INTERVAL: Duration = Duration::from_millis(250);
const PROGRESS_EMIT_INTERVAL: Duration = Duration::from_secs(1);
const DEFAULT_AGENT_JOB_ITEM_TIMEOUT: Duration = Duration::from_secs(60 * 30);

#[derive(Debug, Deserialize)]
struct SpawnAgentsOnCsvArgs {
    csv_path: String,
    instruction: String,
    id_column: Option<String>,
    output_csv_path: Option<String>,
    output_schema: Option<Value>,
    max_concurrency: Option<usize>,
    max_workers: Option<usize>,
    max_runtime_seconds: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct ReportAgentJobResultArgs {
    job_id: String,
    item_id: String,
    result: Value,
    stop: Option<bool>,
}

#[derive(Debug, Serialize)]
struct SpawnAgentsOnCsvResult {
    job_id: String,
    status: String,
    output_csv_path: String,
    total_items: usize,
    completed_items: usize,
    failed_items: usize,
    job_error: Option<String>,
    failed_item_errors: Option<Vec<AgentJobFailureSummary>>,
}

#[derive(Debug, Serialize)]
struct AgentJobFailureSummary {
    item_id: String,
    source_id: Option<String>,
    last_error: String,
}

#[derive(Debug, Serialize)]
struct AgentJobProgressUpdate {
    job_id: String,
    total_items: usize,
    pending_items: usize,
    running_items: usize,
    completed_items: usize,
    failed_items: usize,
    eta_seconds: Option<u64>,
}

#[derive(Debug, Serialize)]
struct ReportAgentJobResultToolResult {
    accepted: bool,
}

#[derive(Debug, Clone)]
struct JobRunnerOptions {
    max_concurrency: usize,
    spawn_config: Config,
}

#[derive(Debug, Clone)]
struct ActiveJobItem {
    item_id: String,
    started_at: Instant,
}

struct JobProgressEmitter {
    started_at: Instant,
    last_emit_at: Instant,
    last_processed: usize,
    last_failed: usize,
}

impl JobProgressEmitter {
    fn new() -> Self {
        let now = Instant::now();
        let last_emit_at = now.checked_sub(PROGRESS_EMIT_INTERVAL).unwrap_or(now);
        Self {
            started_at: now,
            last_emit_at,
            last_processed: 0,
            last_failed: 0,
        }
    }

    async fn maybe_emit(
        &mut self,
        session: &Session,
        turn: &TurnContext,
        job_id: &str,
        progress: &codex_state::AgentJobProgress,
        force: bool,
    ) -> anyhow::Result<()> {
        let processed = progress.completed_items + progress.failed_items;
        let should_emit = force
            || processed != self.last_processed
            || progress.failed_items != self.last_failed
            || self.last_emit_at.elapsed() >= PROGRESS_EMIT_INTERVAL;
        if !should_emit {
            return Ok(());
        }
        let elapsed = self.started_at.elapsed().as_secs_f64();
        let eta_seconds = if processed > 0 && elapsed > 0.0 {
            let remaining = progress.total_items.saturating_sub(processed) as f64;
            let rate = processed as f64 / elapsed;
            if rate > 0.0 {
                Some((remaining / rate).round() as u64)
            } else {
                None
            }
        } else {
            None
        };
        let update = AgentJobProgressUpdate {
            job_id: job_id.to_string(),
            total_items: progress.total_items,
            pending_items: progress.pending_items,
            running_items: progress.running_items,
            completed_items: progress.completed_items,
            failed_items: progress.failed_items,
            eta_seconds,
        };
        let payload = serde_json::to_string(&update)?;
        session
            .notify_background_event(turn, format!("agent_job_progress:{payload}"))
            .await;
        self.last_emit_at = Instant::now();
        self.last_processed = processed;
        self.last_failed = progress.failed_items;
        Ok(())
    }
}

#[async_trait]
impl ToolHandler for BatchJobHandler {
    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<ToolOutput, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
            tool_name,
            payload,
            ..
        } = invocation;

        let arguments = match payload {
            ToolPayload::Function { arguments } => arguments,
            _ => {
                return Err(FunctionCallError::RespondToModel(
                    "agent jobs handler received unsupported payload".to_string(),
                ));
            }
        };

        match tool_name.as_str() {
            "spawn_agents_on_csv" => spawn_agents_on_csv::handle(session, turn, arguments).await,
            "report_agent_job_result" => report_agent_job_result::handle(session, arguments).await,
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported agent job tool {other}"
            ))),
        }
    }
}

mod spawn_agents_on_csv {
    use super::*;

    /// Create a new agent job from a CSV and run it to completion.
    ///
    /// Each CSV row becomes a job item. The instruction string is a template where `{column}`
    /// placeholders are filled with values from that row. Results are reported by workers via
    /// `report_agent_job_result`, then exported to CSV on completion.
    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: SpawnAgentsOnCsvArgs = parse_arguments(arguments.as_str())?;
        if args.instruction.trim().is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "instruction must be non-empty".to_string(),
            ));
        }

        let db = required_state_db(&session)?;
        let input_path = turn.resolve_path(Some(args.csv_path));
        let input_path_display = input_path.display().to_string();
        let csv_content = tokio::fs::read_to_string(&input_path)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to read csv input {input_path_display}: {err}"
                ))
            })?;
        let (headers, rows) = parse_csv(csv_content.as_str()).map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to parse csv input: {err}"))
        })?;
        if headers.is_empty() {
            return Err(FunctionCallError::RespondToModel(
                "csv input must include a header row".to_string(),
            ));
        }
        ensure_unique_headers(headers.as_slice())?;

        let id_column_index = args.id_column.as_ref().map_or(Ok(None), |column_name| {
            headers
                .iter()
                .position(|header| header == column_name)
                .map(Some)
                .ok_or_else(|| {
                    FunctionCallError::RespondToModel(format!(
                        "id_column {column_name} was not found in csv headers"
                    ))
                })
        })?;

        let mut items = Vec::with_capacity(rows.len());
        let mut seen_ids = HashSet::new();
        for (idx, row) in rows.into_iter().enumerate() {
            if row.len() != headers.len() {
                let row_index = idx + 2;
                let row_len = row.len();
                let header_len = headers.len();
                return Err(FunctionCallError::RespondToModel(format!(
                    "csv row {row_index} has {row_len} fields but header has {header_len}"
                )));
            }

            let source_id = id_column_index
                .and_then(|index| row.get(index).cloned())
                .filter(|value| !value.trim().is_empty());
            let row_index = idx + 1;
            let base_item_id = source_id
                .clone()
                .unwrap_or_else(|| format!("row-{row_index}"));
            let mut item_id = base_item_id.clone();
            let mut suffix = 2usize;
            while !seen_ids.insert(item_id.clone()) {
                item_id = format!("{base_item_id}-{suffix}");
                suffix = suffix.saturating_add(1);
            }

            let row_object = headers
                .iter()
                .zip(row.iter())
                .map(|(header, value)| (header.clone(), Value::String(value.clone())))
                .collect::<serde_json::Map<_, _>>();
            items.push(codex_state::AgentJobItemCreateParams {
                item_id,
                row_index: idx as i64,
                source_id,
                row_json: Value::Object(row_object),
            });
        }

        let job_id = Uuid::new_v4().to_string();
        let output_csv_path = args.output_csv_path.map_or_else(
            || default_output_csv_path(input_path.as_path(), job_id.as_str()),
            |path| turn.resolve_path(Some(path)),
        );
        let job_suffix = &job_id[..8];
        let job_name = format!("agent-job-{job_suffix}");
        let max_runtime_seconds = normalize_max_runtime_seconds(
            args.max_runtime_seconds
                .or(turn.config.agent_job_max_runtime_seconds),
        )?;
        let _job = db
            .create_agent_job(
                &codex_state::AgentJobCreateParams {
                    id: job_id.clone(),
                    name: job_name,
                    instruction: args.instruction,
                    auto_export: true,
                    max_runtime_seconds,
                    output_schema_json: args.output_schema,
                    input_headers: headers,
                    input_csv_path: input_path.display().to_string(),
                    output_csv_path: output_csv_path.display().to_string(),
                },
                items.as_slice(),
            )
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!("failed to create agent job: {err}"))
            })?;

        let requested_concurrency = args.max_concurrency.or(args.max_workers);
        let options = match build_runner_options(&session, &turn, requested_concurrency).await {
            Ok(options) => options,
            Err(err) => {
                let error_message = err.to_string();
                let _ = db
                    .mark_agent_job_failed(job_id.as_str(), error_message.as_str())
                    .await;
                return Err(err);
            }
        };
        db.mark_agent_job_running(job_id.as_str())
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to transition agent job {job_id} to running: {err}"
                ))
            })?;
        let max_threads = turn.config.agent_max_threads;
        let effective_concurrency = options.max_concurrency;
        let message = format!(
            "agent job concurrency: job_id={job_id} requested={requested_concurrency:?} max_threads={max_threads:?} effective={effective_concurrency}"
        );
        let _ = session.notify_background_event(&turn, message).await;
        if let Err(err) = run_agent_job_loop(
            session.clone(),
            turn.clone(),
            db.clone(),
            job_id.clone(),
            options,
        )
        .await
        {
            let error_message = format!("job runner failed: {err}");
            let _ = db
                .mark_agent_job_failed(job_id.as_str(), error_message.as_str())
                .await;
            return Err(FunctionCallError::RespondToModel(format!(
                "agent job {job_id} failed: {err}"
            )));
        }

        let job = db
            .get_agent_job(job_id.as_str())
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to load agent job {job_id}: {err}"
                ))
            })?
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!("agent job {job_id} not found"))
            })?;
        let output_path = PathBuf::from(job.output_csv_path.clone());
        if !tokio::fs::try_exists(&output_path).await.unwrap_or(false) {
            export_job_csv_snapshot(db.clone(), &job)
                .await
                .map_err(|err| {
                    FunctionCallError::RespondToModel(format!(
                        "failed to export output csv {job_id}: {err}"
                    ))
                })?;
        }
        let progress = db
            .get_agent_job_progress(job_id.as_str())
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to load agent job progress {job_id}: {err}"
                ))
            })?;
        let mut job_error = job.last_error.clone().filter(|err| !err.trim().is_empty());
        let failed_item_errors = if progress.failed_items > 0 {
            let items = db
                .list_agent_job_items(
                    job_id.as_str(),
                    Some(codex_state::AgentJobItemStatus::Failed),
                    Some(5),
                )
                .await
                .unwrap_or_default();
            let summaries: Vec<_> = items
                .into_iter()
                .filter_map(|item| {
                    let last_error = item.last_error.unwrap_or_default();
                    if last_error.trim().is_empty() {
                        return None;
                    }
                    Some(AgentJobFailureSummary {
                        item_id: item.item_id,
                        source_id: item.source_id,
                        last_error,
                    })
                })
                .collect();
            if summaries.is_empty() {
                if job_error.is_none() {
                    job_error = Some(
                        "agent job has failed items but no error details were recorded".to_string(),
                    );
                }
                None
            } else {
                Some(summaries)
            }
        } else {
            None
        };
        let content = serde_json::to_string(&SpawnAgentsOnCsvResult {
            job_id,
            status: job.status.as_str().to_string(),
            output_csv_path: job.output_csv_path,
            total_items: progress.total_items,
            completed_items: progress.completed_items,
            failed_items: progress.failed_items,
            job_error,
            failed_item_errors,
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize spawn_agents_on_csv result: {err}"
            ))
        })?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

mod report_agent_job_result {
    use super::*;

    pub async fn handle(
        session: Arc<Session>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: ReportAgentJobResultArgs = parse_arguments(arguments.as_str())?;
        if !args.result.is_object() {
            return Err(FunctionCallError::RespondToModel(
                "result must be a JSON object".to_string(),
            ));
        }
        let db = required_state_db(&session)?;
        let reporting_thread_id = session.conversation_id.to_string();
        let accepted = db
            .report_agent_job_item_result(
                args.job_id.as_str(),
                args.item_id.as_str(),
                reporting_thread_id.as_str(),
                &args.result,
            )
            .await
            .map_err(|err| {
                let job_id = args.job_id.as_str();
                let item_id = args.item_id.as_str();
                FunctionCallError::RespondToModel(format!(
                    "failed to record agent job result for {job_id} / {item_id}: {err}"
                ))
            })?;
        if accepted && args.stop.unwrap_or(false) {
            let message = "cancelled by worker request";
            let _ = db
                .mark_agent_job_cancelled(args.job_id.as_str(), message)
                .await;
        }
        let content =
            serde_json::to_string(&ReportAgentJobResultToolResult { accepted }).map_err(|err| {
                FunctionCallError::Fatal(format!(
                    "failed to serialize report_agent_job_result result: {err}"
                ))
            })?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

fn required_state_db(
    session: &Arc<Session>,
) -> Result<Arc<codex_state::StateRuntime>, FunctionCallError> {
    session.state_db().ok_or_else(|| {
        FunctionCallError::Fatal(
            "sqlite state db is unavailable for this session; enable the sqlite feature"
                .to_string(),
        )
    })
}

async fn build_runner_options(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    requested_concurrency: Option<usize>,
) -> Result<JobRunnerOptions, FunctionCallError> {
    let session_source = turn.session_source.clone();
    let child_depth = next_thread_spawn_depth(&session_source);
    let max_depth = turn.config.agent_max_depth;
    if exceeds_thread_spawn_depth_limit(child_depth, max_depth) {
        return Err(FunctionCallError::RespondToModel(
            "agent depth limit reached; this session cannot spawn more subagents".to_string(),
        ));
    }
    let max_concurrency =
        normalize_concurrency(requested_concurrency, turn.config.agent_max_threads);
    let base_instructions = session.get_base_instructions().await;
    let spawn_config = build_agent_spawn_config(&base_instructions, turn.as_ref())?;
    Ok(JobRunnerOptions {
        max_concurrency,
        spawn_config,
    })
}

fn normalize_concurrency(requested: Option<usize>, max_threads: Option<usize>) -> usize {
    let requested = requested.unwrap_or(DEFAULT_AGENT_JOB_CONCURRENCY).max(1);
    let requested = requested.min(MAX_AGENT_JOB_CONCURRENCY);
    if let Some(max_threads) = max_threads {
        requested.min(max_threads.max(1))
    } else {
        requested
    }
}

fn normalize_max_runtime_seconds(requested: Option<u64>) -> Result<Option<u64>, FunctionCallError> {
    let Some(requested) = requested else {
        return Ok(None);
    };
    if requested == 0 {
        return Err(FunctionCallError::RespondToModel(
            "max_runtime_seconds must be >= 1".to_string(),
        ));
    }
    Ok(Some(requested))
}

async fn run_agent_job_loop(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    db: Arc<codex_state::StateRuntime>,
    job_id: String,
    options: JobRunnerOptions,
) -> anyhow::Result<()> {
    let job = db
        .get_agent_job(job_id.as_str())
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent job {job_id} was not found"))?;
    let runtime_timeout = job_runtime_timeout(&job);
    let mut active_items: HashMap<ThreadId, ActiveJobItem> = HashMap::new();
    let mut progress_emitter = JobProgressEmitter::new();
    recover_running_items(
        session.clone(),
        db.clone(),
        job_id.as_str(),
        &mut active_items,
        runtime_timeout,
    )
    .await?;
    let initial_progress = db.get_agent_job_progress(job_id.as_str()).await?;
    progress_emitter
        .maybe_emit(&session, &turn, job_id.as_str(), &initial_progress, true)
        .await?;

    let mut cancel_requested = db.is_agent_job_cancelled(job_id.as_str()).await?;
    loop {
        let mut progressed = false;

        if !cancel_requested && db.is_agent_job_cancelled(job_id.as_str()).await? {
            cancel_requested = true;
            let _ = session
                .notify_background_event(
                    &turn,
                    format!("agent job {job_id} cancellation requested; stopping new workers"),
                )
                .await;
        }

        if !cancel_requested && active_items.len() < options.max_concurrency {
            let slots = options.max_concurrency - active_items.len();
            let pending_items = db
                .list_agent_job_items(
                    job_id.as_str(),
                    Some(codex_state::AgentJobItemStatus::Pending),
                    Some(slots),
                )
                .await?;
            for item in pending_items {
                let prompt = build_worker_prompt(&job, &item)?;
                let items = vec![UserInput::Text {
                    text: prompt,
                    text_elements: Vec::new(),
                }];
                let thread_id = match session
                    .services
                    .agent_control
                    .spawn_agent(
                        options.spawn_config.clone(),
                        items,
                        Some(SessionSource::SubAgent(SubAgentSource::Other(format!(
                            "agent_job:{job_id}"
                        )))),
                    )
                    .await
                {
                    Ok(thread_id) => thread_id,
                    Err(CodexErr::AgentLimitReached { .. }) => {
                        db.mark_agent_job_item_pending(
                            job_id.as_str(),
                            item.item_id.as_str(),
                            None,
                        )
                        .await?;
                        break;
                    }
                    Err(err) => {
                        let error_message = format!("failed to spawn worker: {err}");
                        db.mark_agent_job_item_failed(
                            job_id.as_str(),
                            item.item_id.as_str(),
                            error_message.as_str(),
                        )
                        .await?;
                        progressed = true;
                        continue;
                    }
                };
                let assigned = db
                    .mark_agent_job_item_running_with_thread(
                        job_id.as_str(),
                        item.item_id.as_str(),
                        thread_id.to_string().as_str(),
                    )
                    .await?;
                if !assigned {
                    let _ = session
                        .services
                        .agent_control
                        .shutdown_agent(thread_id)
                        .await;
                    continue;
                }
                active_items.insert(
                    thread_id,
                    ActiveJobItem {
                        item_id: item.item_id.clone(),
                        started_at: Instant::now(),
                    },
                );
                progressed = true;
            }
        }

        if reap_stale_active_items(
            session.clone(),
            db.clone(),
            job_id.as_str(),
            &mut active_items,
            runtime_timeout,
        )
        .await?
        {
            progressed = true;
        }

        let finished = find_finished_threads(session.clone(), &active_items).await;
        if finished.is_empty() {
            let progress = db.get_agent_job_progress(job_id.as_str()).await?;
            if cancel_requested {
                if progress.running_items == 0 && active_items.is_empty() {
                    break;
                }
            } else if progress.pending_items == 0
                && progress.running_items == 0
                && active_items.is_empty()
            {
                break;
            }
            if !progressed {
                tokio::time::sleep(STATUS_POLL_INTERVAL).await;
            }
            continue;
        }

        for (thread_id, item_id) in finished {
            finalize_finished_item(
                session.clone(),
                db.clone(),
                job_id.as_str(),
                item_id.as_str(),
                thread_id,
            )
            .await?;
            active_items.remove(&thread_id);
            let progress = db.get_agent_job_progress(job_id.as_str()).await?;
            progress_emitter
                .maybe_emit(&session, &turn, job_id.as_str(), &progress, false)
                .await?;
        }
    }

    let progress = db.get_agent_job_progress(job_id.as_str()).await?;
    if let Err(err) = export_job_csv_snapshot(db.clone(), &job).await {
        let message = format!("auto-export failed: {err}");
        db.mark_agent_job_failed(job_id.as_str(), message.as_str())
            .await?;
        return Ok(());
    }
    let cancelled = cancel_requested || db.is_agent_job_cancelled(job_id.as_str()).await?;
    if cancelled {
        let pending_items = progress.pending_items;
        let message =
            format!("agent job {job_id} cancelled with {pending_items} unprocessed items");
        let _ = session.notify_background_event(&turn, message).await;
        progress_emitter
            .maybe_emit(&session, &turn, job_id.as_str(), &progress, true)
            .await?;
        return Ok(());
    }
    if progress.failed_items > 0 {
        let failed_items = progress.failed_items;
        let message = format!("agent job completed with {failed_items} failed items");
        let _ = session.notify_background_event(&turn, message).await;
    }
    db.mark_agent_job_completed(job_id.as_str()).await?;
    let progress = db.get_agent_job_progress(job_id.as_str()).await?;
    progress_emitter
        .maybe_emit(&session, &turn, job_id.as_str(), &progress, true)
        .await?;
    Ok(())
}

async fn export_job_csv_snapshot(
    db: Arc<codex_state::StateRuntime>,
    job: &codex_state::AgentJob,
) -> anyhow::Result<()> {
    let items = db.list_agent_job_items(job.id.as_str(), None, None).await?;
    let csv_content = render_job_csv(job.input_headers.as_slice(), items.as_slice())
        .map_err(|err| anyhow::anyhow!("failed to render job csv for auto-export: {err}"))?;
    let output_path = PathBuf::from(job.output_csv_path.clone());
    if let Some(parent) = output_path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&output_path, csv_content).await?;
    Ok(())
}

async fn recover_running_items(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    runtime_timeout: Duration,
) -> anyhow::Result<()> {
    let running_items = db
        .list_agent_job_items(job_id, Some(codex_state::AgentJobItemStatus::Running), None)
        .await?;
    for item in running_items {
        if is_item_stale(&item, runtime_timeout) {
            let error_message = format!("worker exceeded max runtime of {runtime_timeout:?}");
            db.mark_agent_job_item_failed(job_id, item.item_id.as_str(), error_message.as_str())
                .await?;
            if let Some(assigned_thread_id) = item.assigned_thread_id.as_ref()
                && let Ok(thread_id) = ThreadId::from_string(assigned_thread_id.as_str())
            {
                let _ = session
                    .services
                    .agent_control
                    .shutdown_agent(thread_id)
                    .await;
            }
            continue;
        }
        let Some(assigned_thread_id) = item.assigned_thread_id.clone() else {
            db.mark_agent_job_item_failed(
                job_id,
                item.item_id.as_str(),
                "running item is missing assigned_thread_id",
            )
            .await?;
            continue;
        };
        let thread_id = match ThreadId::from_string(assigned_thread_id.as_str()) {
            Ok(thread_id) => thread_id,
            Err(err) => {
                let error_message = format!("invalid assigned_thread_id: {err:?}");
                db.mark_agent_job_item_failed(
                    job_id,
                    item.item_id.as_str(),
                    error_message.as_str(),
                )
                .await?;
                continue;
            }
        };
        if is_final(&session.services.agent_control.get_status(thread_id).await) {
            finalize_finished_item(
                session.clone(),
                db.clone(),
                job_id,
                item.item_id.as_str(),
                thread_id,
            )
            .await?;
        } else {
            active_items.insert(
                thread_id,
                ActiveJobItem {
                    item_id: item.item_id.clone(),
                    started_at: started_at_from_item(&item),
                },
            );
        }
    }
    Ok(())
}

async fn find_finished_threads(
    session: Arc<Session>,
    active_items: &HashMap<ThreadId, ActiveJobItem>,
) -> Vec<(ThreadId, String)> {
    let mut finished = Vec::new();
    for (thread_id, item) in active_items {
        if is_final(&session.services.agent_control.get_status(*thread_id).await) {
            finished.push((*thread_id, item.item_id.clone()));
        }
    }
    finished
}

async fn reap_stale_active_items(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    active_items: &mut HashMap<ThreadId, ActiveJobItem>,
    runtime_timeout: Duration,
) -> anyhow::Result<bool> {
    let mut stale = Vec::new();
    for (thread_id, item) in active_items.iter() {
        if item.started_at.elapsed() >= runtime_timeout {
            stale.push((*thread_id, item.item_id.clone()));
        }
    }
    if stale.is_empty() {
        return Ok(false);
    }
    for (thread_id, item_id) in stale {
        let error_message = format!("worker exceeded max runtime of {runtime_timeout:?}");
        db.mark_agent_job_item_failed(job_id, item_id.as_str(), error_message.as_str())
            .await?;
        let _ = session
            .services
            .agent_control
            .shutdown_agent(thread_id)
            .await;
        active_items.remove(&thread_id);
    }
    Ok(true)
}

async fn finalize_finished_item(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
    item_id: &str,
    thread_id: ThreadId,
) -> anyhow::Result<()> {
    let mut item = db
        .get_agent_job_item(job_id, item_id)
        .await?
        .ok_or_else(|| {
            anyhow::anyhow!("job item not found for finalization: {job_id}/{item_id}")
        })?;
    if item.result_json.is_none() {
        tokio::time::sleep(Duration::from_millis(250)).await;
        item = db
            .get_agent_job_item(job_id, item_id)
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("job item not found after grace period: {job_id}/{item_id}")
            })?;
    }
    if item.result_json.is_some() {
        if !db.mark_agent_job_item_completed(job_id, item_id).await? {
            db.mark_agent_job_item_failed(
                job_id,
                item_id,
                "worker reported result but item could not transition to completed",
            )
            .await?;
        }
    } else {
        db.mark_agent_job_item_failed(
            job_id,
            item_id,
            "worker finished without calling report_agent_job_result",
        )
        .await?;
    }
    let _ = session
        .services
        .agent_control
        .shutdown_agent(thread_id)
        .await;
    Ok(())
}

fn build_worker_prompt(
    job: &codex_state::AgentJob,
    item: &codex_state::AgentJobItem,
) -> anyhow::Result<String> {
    let job_id = job.id.as_str();
    let item_id = item.item_id.as_str();
    let instruction = render_instruction_template(job.instruction.as_str(), &item.row_json);
    let output_schema = job
        .output_schema_json
        .as_ref()
        .map(serde_json::to_string_pretty)
        .transpose()?
        .unwrap_or_else(|| "{}".to_string());
    let row_json = serde_json::to_string_pretty(&item.row_json)?;
    Ok(format!(
        "You are processing one item for a generic agent job.\n\
Job ID: {job_id}\n\
Item ID: {item_id}\n\n\
Task instruction:\n\
{instruction}\n\n\
Input row (JSON):\n\
{row_json}\n\n\
Expected result schema (JSON Schema or {{}}):\n\
{output_schema}\n\n\
You MUST call the `report_agent_job_result` tool exactly once with:\n\
1. `job_id` = \"{job_id}\"\n\
2. `item_id` = \"{item_id}\"\n\
3. `result` = a JSON object that contains your analysis result for this row.\n\n\
If you need to stop the job early, include `stop` = true in the tool call.\n\n\
After the tool call succeeds, stop.",
    ))
}

fn render_instruction_template(instruction: &str, row_json: &Value) -> String {
    const OPEN_BRACE_SENTINEL: &str = "__CODEX_OPEN_BRACE__";
    const CLOSE_BRACE_SENTINEL: &str = "__CODEX_CLOSE_BRACE__";

    let mut rendered = instruction
        .replace("{{", OPEN_BRACE_SENTINEL)
        .replace("}}", CLOSE_BRACE_SENTINEL);
    let Some(row) = row_json.as_object() else {
        return rendered
            .replace(OPEN_BRACE_SENTINEL, "{")
            .replace(CLOSE_BRACE_SENTINEL, "}");
    };
    for (key, value) in row {
        let placeholder = format!("{{{key}}}");
        let replacement = value
            .as_str()
            .map(str::to_string)
            .unwrap_or_else(|| value.to_string());
        rendered = rendered.replace(placeholder.as_str(), replacement.as_str());
    }
    rendered
        .replace(OPEN_BRACE_SENTINEL, "{")
        .replace(CLOSE_BRACE_SENTINEL, "}")
}

fn ensure_unique_headers(headers: &[String]) -> Result<(), FunctionCallError> {
    let mut seen = HashSet::new();
    for header in headers {
        if !seen.insert(header) {
            return Err(FunctionCallError::RespondToModel(format!(
                "csv header {header} is duplicated"
            )));
        }
    }
    Ok(())
}

fn job_runtime_timeout(job: &codex_state::AgentJob) -> Duration {
    job.max_runtime_seconds
        .map(Duration::from_secs)
        .unwrap_or(DEFAULT_AGENT_JOB_ITEM_TIMEOUT)
}

fn started_at_from_item(item: &codex_state::AgentJobItem) -> Instant {
    let now = chrono::Utc::now();
    let age = now.signed_duration_since(item.updated_at);
    if let Ok(age) = age.to_std() {
        Instant::now().checked_sub(age).unwrap_or_else(Instant::now)
    } else {
        Instant::now()
    }
}

fn is_item_stale(item: &codex_state::AgentJobItem, runtime_timeout: Duration) -> bool {
    let now = chrono::Utc::now();
    if let Ok(age) = now.signed_duration_since(item.updated_at).to_std() {
        age >= runtime_timeout
    } else {
        false
    }
}

fn default_output_csv_path(input_csv_path: &Path, job_id: &str) -> PathBuf {
    let stem = input_csv_path
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("agent_job_output");
    let job_suffix = &job_id[..8];
    input_csv_path.with_file_name(format!("{stem}.agent-job-{job_suffix}.csv"))
}

fn parse_csv(content: &str) -> Result<(Vec<String>, Vec<Vec<String>>), String> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(true)
        .flexible(true)
        .from_reader(content.as_bytes());
    let headers_record = reader.headers().map_err(|err| err.to_string())?;
    let mut headers: Vec<String> = headers_record.iter().map(str::to_string).collect();
    if let Some(first) = headers.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.map_err(|err| err.to_string())?;
        let row: Vec<String> = record.iter().map(str::to_string).collect();
        if row.iter().all(std::string::String::is_empty) {
            continue;
        }
        rows.push(row);
    }
    Ok((headers, rows))
}

fn render_job_csv(
    headers: &[String],
    items: &[codex_state::AgentJobItem],
) -> Result<String, FunctionCallError> {
    let mut csv = String::new();
    let mut output_headers = headers.to_vec();
    output_headers.extend([
        "job_id".to_string(),
        "item_id".to_string(),
        "row_index".to_string(),
        "source_id".to_string(),
        "status".to_string(),
        "attempt_count".to_string(),
        "last_error".to_string(),
        "result_json".to_string(),
        "reported_at".to_string(),
        "completed_at".to_string(),
    ]);
    csv.push_str(
        output_headers
            .iter()
            .map(|header| csv_escape(header.as_str()))
            .collect::<Vec<_>>()
            .join(",")
            .as_str(),
    );
    csv.push('\n');
    for item in items {
        let row_object = item.row_json.as_object().ok_or_else(|| {
            let item_id = item.item_id.as_str();
            FunctionCallError::RespondToModel(format!(
                "row_json for item {item_id} is not a JSON object"
            ))
        })?;
        let mut row_values = Vec::new();
        for header in headers {
            let value = row_object
                .get(header)
                .map_or_else(String::new, value_to_csv_string);
            row_values.push(csv_escape(value.as_str()));
        }
        row_values.push(csv_escape(item.job_id.as_str()));
        row_values.push(csv_escape(item.item_id.as_str()));
        row_values.push(csv_escape(item.row_index.to_string().as_str()));
        row_values.push(csv_escape(
            item.source_id.clone().unwrap_or_default().as_str(),
        ));
        row_values.push(csv_escape(item.status.as_str()));
        row_values.push(csv_escape(item.attempt_count.to_string().as_str()));
        row_values.push(csv_escape(
            item.last_error.clone().unwrap_or_default().as_str(),
        ));
        row_values.push(csv_escape(
            item.result_json
                .as_ref()
                .map_or_else(String::new, std::string::ToString::to_string)
                .as_str(),
        ));
        row_values.push(csv_escape(
            item.reported_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_default()
                .as_str(),
        ));
        row_values.push(csv_escape(
            item.completed_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_default()
                .as_str(),
        ));
        csv.push_str(row_values.join(",").as_str());
        csv.push('\n');
    }
    Ok(csv)
}

fn value_to_csv_string(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(s) => s.clone(),
        Value::Bool(b) => b.to_string(),
        Value::Number(n) => n.to_string(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

fn csv_escape(value: &str) -> String {
    if value.contains(',') || value.contains('\n') || value.contains('\r') || value.contains('"') {
        let escaped = value.replace('"', "\"\"");
        format!("\"{escaped}\"")
    } else {
        value.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn parse_csv_supports_quotes_and_commas() {
        let input = "id,name\n1,\"alpha, beta\"\n2,gamma\n";
        let (headers, rows) = parse_csv(input).expect("csv parse");
        assert_eq!(headers, vec!["id".to_string(), "name".to_string()]);
        assert_eq!(
            rows,
            vec![
                vec!["1".to_string(), "alpha, beta".to_string()],
                vec!["2".to_string(), "gamma".to_string()]
            ]
        );
    }

    #[test]
    fn csv_escape_quotes_when_needed() {
        assert_eq!(csv_escape("simple"), "simple");
        assert_eq!(csv_escape("a,b"), "\"a,b\"");
        assert_eq!(csv_escape("a\"b"), "\"a\"\"b\"");
    }

    #[test]
    fn render_instruction_template_expands_placeholders_and_escapes_braces() {
        let row = json!({
            "path": "src/lib.rs",
            "area": "test",
            "file path": "docs/readme.md",
        });
        let rendered = render_instruction_template(
            "Review {path} in {area}. Also see {file path}. Use {{literal}}.",
            &row,
        );
        assert_eq!(
            rendered,
            "Review src/lib.rs in test. Also see docs/readme.md. Use {literal}."
        );
    }

    #[test]
    fn render_instruction_template_leaves_unknown_placeholders() {
        let row = json!({
            "path": "src/lib.rs",
        });
        let rendered = render_instruction_template("Check {path} then {missing}", &row);
        assert_eq!(rendered, "Check src/lib.rs then {missing}");
    }

    #[test]
    fn ensure_unique_headers_rejects_duplicates() {
        let headers = vec!["path".to_string(), "path".to_string()];
        let Err(err) = ensure_unique_headers(headers.as_slice()) else {
            panic!("expected duplicate header error");
        };
        assert_eq!(
            err,
            FunctionCallError::RespondToModel("csv header path is duplicated".to_string())
        );
    }
}
