use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::agent_jobs_spec::create_spawn_agents_on_csv_tool;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use codex_tools::ToolName;
use codex_tools::ToolSpec;

use super::*;

pub struct SpawnAgentsOnCsvHandler;

impl ToolExecutor<ToolInvocation> for SpawnAgentsOnCsvHandler {
    type Output = FunctionToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::plain("spawn_agents_on_csv")
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_spawn_agents_on_csv_tool())
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let ToolInvocation {
            session,
            turn,
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

        handle(session, turn, arguments).await
    }
}

impl ToolHandler for SpawnAgentsOnCsvHandler {
    fn matches_kind(&self, payload: &ToolPayload) -> bool {
        matches!(payload, ToolPayload::Function { .. })
    }
}

/// Create a new agent job from a CSV and run it to completion.
///
/// Each CSV row becomes a job item. The instruction string is a template where `{column}`
/// placeholders are filled with values from that row. Results are reported by workers via
/// `report_agent_job_result`, then exported to CSV on completion.
pub async fn handle(
    session: Arc<Session>,
    turn: Arc<TurnContext>,
    arguments: String,
) -> Result<FunctionToolOutput, FunctionCallError> {
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
        || default_output_csv_path(&input_path, job_id.as_str()),
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
            FunctionCallError::RespondToModel(format!("failed to load agent job {job_id}: {err}"))
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
    Ok(FunctionToolOutput::from_text(content, Some(true)))
}
