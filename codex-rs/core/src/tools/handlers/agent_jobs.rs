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
use crate::tools::handlers::collab::build_agent_spawn_config;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use async_trait::async_trait;
use codex_protocol::ThreadId;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use once_cell::sync::Lazy;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use std::collections::HashMap;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::Duration;
use tokio::time::Instant;
use uuid::Uuid;

pub struct AgentJobsHandler;

const MIN_WAIT_TIMEOUT_MS: i64 = 1_000;
const DEFAULT_WAIT_TIMEOUT_MS: i64 = 30_000;
const MAX_WAIT_TIMEOUT_MS: i64 = 300_000;
const STATUS_POLL_INTERVAL_MS: u64 = 250;

static ACTIVE_JOB_RUNNERS: Lazy<Mutex<HashMap<String, JoinHandle<()>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

#[derive(Debug, Deserialize)]
struct SpawnAgentsOnCsvArgs {
    csv_path: String,
    instruction: String,
    id_column: Option<String>,
    job_name: Option<String>,
    output_csv_path: Option<String>,
    output_schema: Option<Value>,
    max_concurrency: Option<usize>,
    auto_export: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct JobIdArgs {
    job_id: String,
}

#[derive(Debug, Deserialize)]
struct RunAgentJobArgs {
    job_id: String,
    max_concurrency: Option<usize>,
    auto_export: Option<bool>,
}

#[derive(Debug, Deserialize)]
struct WaitAgentJobArgs {
    job_id: String,
    timeout_ms: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct ExportAgentJobCsvArgs {
    job_id: String,
    path: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ReportAgentJobResultArgs {
    job_id: String,
    item_id: String,
    result: Value,
}

#[derive(Debug, Serialize)]
struct AgentJobToolResult {
    job_id: String,
    status: String,
    total_items: usize,
    pending_items: usize,
    running_items: usize,
    completed_items: usize,
    failed_items: usize,
    output_csv_path: String,
    runner_active: bool,
}

#[derive(Debug, Serialize)]
struct SpawnAgentsOnCsvResult {
    job_id: String,
    started: bool,
    output_csv_path: String,
    total_items: usize,
}

#[derive(Debug, Serialize)]
struct WaitAgentJobResult {
    status: AgentJobToolResult,
    timed_out: bool,
}

#[derive(Debug, Serialize)]
struct ExportAgentJobCsvResult {
    job_id: String,
    path: String,
    row_count: usize,
}

#[derive(Debug, Serialize)]
struct ReportAgentJobResultToolResult {
    accepted: bool,
}

#[derive(Debug, Clone)]
struct JobRunnerOptions {
    max_concurrency: usize,
    spawn_config: Config,
    child_depth: i32,
    auto_export: bool,
}

#[async_trait]
impl ToolHandler for AgentJobsHandler {
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
            "run_agent_job" => run_agent_job::handle(session, turn, arguments).await,
            "get_agent_job_status" => get_agent_job_status::handle(session, arguments).await,
            "wait_agent_job" => wait_agent_job::handle(session, arguments).await,
            "export_agent_job_csv" => export_agent_job_csv::handle(session, turn, arguments).await,
            "report_agent_job_result" => report_agent_job_result::handle(session, arguments).await,
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported agent job tool {other}"
            ))),
        }
    }
}

mod spawn_agents_on_csv {
    use super::*;

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
            let mut item_id = source_id
                .clone()
                .unwrap_or_else(|| format!("row-{row_index}"));
            if !seen_ids.insert(item_id.clone()) {
                item_id = format!("{item_id}-{row_index}");
                seen_ids.insert(item_id.clone());
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
        let job_name = args
            .job_name
            .unwrap_or_else(|| format!("agent-job-{job_suffix}"));
        let auto_export = args.auto_export.unwrap_or(true);
        let job = db
            .create_agent_job(
                &codex_state::AgentJobCreateParams {
                    id: job_id.clone(),
                    name: job_name,
                    instruction: args.instruction,
                    auto_export,
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

        let options =
            build_runner_options(&session, &turn, args.max_concurrency, Some(auto_export)).await?;
        let started = start_job_runner(session, job_id.clone(), options).await?;

        let content = serde_json::to_string(&SpawnAgentsOnCsvResult {
            job_id,
            started,
            output_csv_path: job.output_csv_path,
            total_items: items.len(),
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

mod run_agent_job {
    use super::*;

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: RunAgentJobArgs = parse_arguments(arguments.as_str())?;
        let job_id = args.job_id;
        let db = required_state_db(&session)?;
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
        let options = build_runner_options(
            &session,
            &turn,
            args.max_concurrency,
            args.auto_export.or(Some(job.auto_export)),
        )
        .await?;
        let started = start_job_runner(session, job_id.clone(), options).await?;
        let status = render_job_status(db, job_id.as_str()).await?;
        let content = serde_json::to_string(&json!({
            "started": started,
            "status": status,
        }))
        .map_err(|err| {
            FunctionCallError::Fatal(format!("failed to serialize run_agent_job result: {err}"))
        })?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

mod get_agent_job_status {
    use super::*;

    pub async fn handle(
        session: Arc<Session>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: JobIdArgs = parse_arguments(arguments.as_str())?;
        let job_id = args.job_id;
        let db = required_state_db(&session)?;
        let status = render_job_status(db, job_id.as_str()).await?;
        let content = serde_json::to_string(&status).map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize get_agent_job_status result: {err}"
            ))
        })?;
        Ok(ToolOutput::Function {
            body: FunctionCallOutputBody::Text(content),
            success: Some(true),
        })
    }
}

mod wait_agent_job {
    use super::*;

    pub async fn handle(
        session: Arc<Session>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: WaitAgentJobArgs = parse_arguments(arguments.as_str())?;
        let db = required_state_db(&session)?;
        let timeout_ms = args.timeout_ms.unwrap_or(DEFAULT_WAIT_TIMEOUT_MS);
        let timeout_ms = match timeout_ms {
            ms if ms <= 0 => {
                return Err(FunctionCallError::RespondToModel(
                    "timeout_ms must be greater than zero".to_string(),
                ));
            }
            ms => ms.clamp(MIN_WAIT_TIMEOUT_MS, MAX_WAIT_TIMEOUT_MS),
        };

        let deadline = Instant::now() + Duration::from_millis(timeout_ms as u64);
        loop {
            let status = render_job_status(db.clone(), args.job_id.as_str()).await?;
            if matches!(status.status.as_str(), "completed" | "failed" | "cancelled") {
                let content = serde_json::to_string(&WaitAgentJobResult {
                    status,
                    timed_out: false,
                })
                .map_err(|err| {
                    FunctionCallError::Fatal(format!(
                        "failed to serialize wait_agent_job result: {err}"
                    ))
                })?;
                return Ok(ToolOutput::Function {
                    body: FunctionCallOutputBody::Text(content),
                    success: Some(true),
                });
            }
            if Instant::now() >= deadline {
                let content = serde_json::to_string(&WaitAgentJobResult {
                    status,
                    timed_out: true,
                })
                .map_err(|err| {
                    FunctionCallError::Fatal(format!(
                        "failed to serialize wait_agent_job timeout result: {err}"
                    ))
                })?;
                return Ok(ToolOutput::Function {
                    body: FunctionCallOutputBody::Text(content),
                    success: Some(true),
                });
            }
            tokio::time::sleep(Duration::from_millis(STATUS_POLL_INTERVAL_MS)).await;
        }
    }
}

mod export_agent_job_csv {
    use super::*;

    pub async fn handle(
        session: Arc<Session>,
        turn: Arc<TurnContext>,
        arguments: String,
    ) -> Result<ToolOutput, FunctionCallError> {
        let args: ExportAgentJobCsvArgs = parse_arguments(arguments.as_str())?;
        let job_id = args.job_id;
        let db = required_state_db(&session)?;
        let job = db
            .get_agent_job(job_id.as_str())
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to look up agent job {job_id}: {err}"
                ))
            })?
            .ok_or_else(|| {
                FunctionCallError::RespondToModel(format!("agent job {job_id} not found"))
            })?;
        let items = db
            .list_agent_job_items(job_id.as_str(), None, None)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to load items for agent job {job_id}: {err}"
                ))
            })?;
        let output_path = args.path.map_or_else(
            || PathBuf::from(job.output_csv_path.clone()),
            |path| turn.resolve_path(Some(path)),
        );
        if let Some(parent) = output_path.parent() {
            let parent_display = parent.display().to_string();
            tokio::fs::create_dir_all(parent).await.map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to create export directory {parent_display}: {err}"
                ))
            })?;
        }
        let csv_content = render_job_csv(job.input_headers.as_slice(), items.as_slice())?;
        let output_display = output_path.display().to_string();
        tokio::fs::write(&output_path, csv_content)
            .await
            .map_err(|err| {
                FunctionCallError::RespondToModel(format!(
                    "failed to write csv export {output_display}: {err}"
                ))
            })?;
        let content = serde_json::to_string(&ExportAgentJobCsvResult {
            job_id,
            path: output_display,
            row_count: items.len(),
        })
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize export_agent_job_csv result: {err}"
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
        FunctionCallError::RespondToModel(
            "sqlite state db is unavailable for this session; enable the sqlite feature"
                .to_string(),
        )
    })
}

async fn build_runner_options(
    session: &Arc<Session>,
    turn: &Arc<TurnContext>,
    requested_concurrency: Option<usize>,
    requested_auto_export: Option<bool>,
) -> Result<JobRunnerOptions, FunctionCallError> {
    let session_source = turn.session_source.clone();
    let child_depth = next_thread_spawn_depth(&session_source);
    if exceeds_thread_spawn_depth_limit(child_depth) {
        return Err(FunctionCallError::RespondToModel(
            "agent depth limit reached; this session cannot spawn more subagents".to_string(),
        ));
    }
    let max_concurrency =
        normalize_concurrency(requested_concurrency, turn.config.agent_max_threads);
    let base_instructions = session.get_base_instructions().await;
    let spawn_config = build_agent_spawn_config(&base_instructions, turn.as_ref(), child_depth)?;
    Ok(JobRunnerOptions {
        max_concurrency,
        spawn_config,
        child_depth,
        auto_export: requested_auto_export.unwrap_or(true),
    })
}

fn normalize_concurrency(requested: Option<usize>, max_threads: Option<usize>) -> usize {
    let requested = requested.unwrap_or(4).max(1);
    let requested = requested.min(64);
    if let Some(max_threads) = max_threads {
        requested.min(max_threads.max(1))
    } else {
        requested
    }
}

async fn start_job_runner(
    session: Arc<Session>,
    job_id: String,
    options: JobRunnerOptions,
) -> Result<bool, FunctionCallError> {
    cleanup_finished_runners().await;
    let mut runners = ACTIVE_JOB_RUNNERS.lock().await;
    if let Some(handle) = runners.get(job_id.as_str())
        && !handle.is_finished()
    {
        return Ok(false);
    }
    let db = required_state_db(&session)?;
    let job = db
        .get_agent_job(job_id.as_str())
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to load agent job {job_id}: {err}"))
        })?
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(format!("agent job {job_id} not found"))
        })?;
    let progress = db
        .get_agent_job_progress(job_id.as_str())
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to load agent job progress {job_id}: {err}"
            ))
        })?;
    if job.status.is_final() && progress.pending_items == 0 && progress.running_items == 0 {
        return Ok(false);
    }
    db.mark_agent_job_running(job_id.as_str())
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "failed to transition agent job {job_id} to running: {err}"
            ))
        })?;
    let job_id_for_task = job_id.clone();
    let handle = tokio::spawn(async move {
        if let Err(err) =
            run_agent_job_loop(session, db.clone(), job_id_for_task.clone(), options).await
        {
            let error_message = format!("job runner failed: {err}");
            let _ = db
                .mark_agent_job_failed(job_id_for_task.as_str(), error_message.as_str())
                .await;
        }
    });
    runners.insert(job_id, handle);
    Ok(true)
}

async fn cleanup_finished_runners() {
    let mut runners = ACTIVE_JOB_RUNNERS.lock().await;
    runners.retain(|_, handle| !handle.is_finished());
}

async fn run_agent_job_loop(
    session: Arc<Session>,
    db: Arc<codex_state::StateRuntime>,
    job_id: String,
    options: JobRunnerOptions,
) -> anyhow::Result<()> {
    let mut active_items: HashMap<ThreadId, String> = HashMap::new();
    recover_running_items(
        session.clone(),
        db.clone(),
        job_id.as_str(),
        &mut active_items,
    )
    .await?;

    let job = db
        .get_agent_job(job_id.as_str())
        .await?
        .ok_or_else(|| anyhow::anyhow!("agent job {job_id} was not found"))?;

    loop {
        let mut progressed = false;

        if active_items.len() < options.max_concurrency {
            let slots = options.max_concurrency - active_items.len();
            let pending_items = db
                .list_agent_job_items(
                    job_id.as_str(),
                    Some(codex_state::AgentJobItemStatus::Pending),
                    Some(slots),
                )
                .await?;
            for item in pending_items {
                if !db
                    .mark_agent_job_item_running(job_id.as_str(), item.item_id.as_str())
                    .await?
                {
                    continue;
                }
                let prompt = build_worker_prompt(&job, &item)?;
                let thread_id = match session
                    .services
                    .agent_control
                    .spawn_agent(
                        options.spawn_config.clone(),
                        prompt,
                        Some(SessionSource::SubAgent(SubAgentSource::ThreadSpawn {
                            parent_thread_id: session.conversation_id,
                            depth: options.child_depth,
                        })),
                    )
                    .await
                {
                    Ok(thread_id) => thread_id,
                    Err(CodexErr::AgentLimitReached { .. }) => {
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
                    .set_agent_job_item_thread(
                        job_id.as_str(),
                        item.item_id.as_str(),
                        thread_id.to_string().as_str(),
                    )
                    .await?;
                if !assigned {
                    db.mark_agent_job_item_failed(
                        job_id.as_str(),
                        item.item_id.as_str(),
                        "failed to assign worker thread to job item",
                    )
                    .await?;
                    let _ = session
                        .services
                        .agent_control
                        .shutdown_agent(thread_id)
                        .await;
                    progressed = true;
                    continue;
                }
                active_items.insert(thread_id, item.item_id.clone());
                progressed = true;
            }
        }

        let finished = find_finished_threads(session.clone(), &active_items).await;
        if finished.is_empty() {
            let progress = db.get_agent_job_progress(job_id.as_str()).await?;
            if progress.pending_items == 0 && progress.running_items == 0 && active_items.is_empty()
            {
                break;
            }
            if !progressed {
                tokio::time::sleep(Duration::from_millis(STATUS_POLL_INTERVAL_MS)).await;
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
        }
    }

    let progress = db.get_agent_job_progress(job_id.as_str()).await?;
    if progress.failed_items > 0 {
        let failed_items = progress.failed_items;
        let message = format!("job completed with {failed_items} failed items");
        db.mark_agent_job_failed(job_id.as_str(), message.as_str())
            .await?;
    } else {
        if options.auto_export {
            if let Err(err) = export_job_csv_snapshot(db.clone(), &job).await {
                let message = format!("auto-export failed: {err}");
                db.mark_agent_job_failed(job_id.as_str(), message.as_str())
                    .await?;
                return Ok(());
            }
        }
        db.mark_agent_job_completed(job_id.as_str()).await?;
    }
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
    active_items: &mut HashMap<ThreadId, String>,
) -> anyhow::Result<()> {
    let running_items = db
        .list_agent_job_items(job_id, Some(codex_state::AgentJobItemStatus::Running), None)
        .await?;
    for item in running_items {
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
            active_items.insert(thread_id, item.item_id.clone());
        }
    }
    Ok(())
}

async fn find_finished_threads(
    session: Arc<Session>,
    active_items: &HashMap<ThreadId, String>,
) -> Vec<(ThreadId, String)> {
    let mut finished = Vec::new();
    for (thread_id, item_id) in active_items {
        if is_final(&session.services.agent_control.get_status(*thread_id).await) {
            finished.push((*thread_id, item_id.clone()));
        }
    }
    finished
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
    let instruction = job.instruction.as_str();
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
After the tool call succeeds, stop.",
    ))
}

async fn render_job_status(
    db: Arc<codex_state::StateRuntime>,
    job_id: &str,
) -> Result<AgentJobToolResult, FunctionCallError> {
    let job = db
        .get_agent_job(job_id)
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!("failed to fetch agent job {job_id}: {err}"))
        })?
        .ok_or_else(|| {
            FunctionCallError::RespondToModel(format!("agent job {job_id} not found"))
        })?;
    let progress = db.get_agent_job_progress(job_id).await.map_err(|err| {
        FunctionCallError::RespondToModel(format!("failed to fetch progress for {job_id}: {err}"))
    })?;
    cleanup_finished_runners().await;
    let runners = ACTIVE_JOB_RUNNERS.lock().await;
    let runner_active = runners
        .get(job_id)
        .is_some_and(|handle| !handle.is_finished());
    Ok(AgentJobToolResult {
        job_id: job.id,
        status: job.status.as_str().to_string(),
        total_items: progress.total_items,
        pending_items: progress.pending_items,
        running_items: progress.running_items,
        completed_items: progress.completed_items,
        failed_items: progress.failed_items,
        output_csv_path: job.output_csv_path,
        runner_active,
    })
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
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut row: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut in_quotes = false;
    let mut chars = content.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '"' => {
                if in_quotes {
                    if chars.peek().is_some_and(|next| *next == '"') {
                        field.push('"');
                        let _ = chars.next();
                    } else {
                        in_quotes = false;
                    }
                } else {
                    in_quotes = true;
                }
            }
            ',' if !in_quotes => {
                row.push(std::mem::take(&mut field));
            }
            '\n' if !in_quotes => {
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            '\r' if !in_quotes => {
                if chars.peek().is_some_and(|next| *next == '\n') {
                    continue;
                }
                row.push(std::mem::take(&mut field));
                rows.push(std::mem::take(&mut row));
            }
            other => field.push(other),
        }
    }
    if in_quotes {
        return Err("unterminated quoted field".to_string());
    }
    if !field.is_empty() || !row.is_empty() {
        row.push(field);
        rows.push(row);
    }
    if rows.is_empty() {
        return Ok((Vec::new(), Vec::new()));
    }
    let mut headers = rows.remove(0);
    if let Some(first) = headers.first_mut() {
        *first = first.trim_start_matches('\u{feff}').to_string();
    }
    let data_rows = rows
        .into_iter()
        .filter(|row| row.iter().any(|value| !value.is_empty()))
        .collect();
    Ok((headers, data_rows))
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
}
