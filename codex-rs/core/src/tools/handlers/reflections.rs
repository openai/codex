use crate::function_tool::FunctionCallError;
use crate::tools::context::FunctionToolOutput;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolHandler;
use crate::tools::registry::ToolKind;
use codex_tools::REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME;
use codex_tools::REFLECTIONS_LIST_TOOL_NAME;
use codex_tools::REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME;
use codex_tools::REFLECTIONS_READ_TOOL_NAME;
use codex_tools::REFLECTIONS_SEARCH_TOOL_NAME;
use codex_tools::REFLECTIONS_WRITE_NOTE_TOOL_NAME;
use serde::Deserialize;
use serde::Serialize;
use serde_json::json;
use std::path::PathBuf;

#[derive(Debug, Deserialize)]
struct ListArgs {
    collection: String,
    start: Option<usize>,
    stop: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ReadArgs {
    kind: String,
    id: String,
    start: Option<usize>,
    stop: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct SearchArgs {
    scope: String,
    query: String,
    start: Option<usize>,
    stop: Option<usize>,
    log_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WriteNoteArgs {
    note_id: String,
    operation: String,
    content: String,
}

pub struct ReflectionsNewContextWindowHandler;

impl ToolHandler for ReflectionsNewContextWindowHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        if !matches!(invocation.payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{REFLECTIONS_NEW_CONTEXT_WINDOW_TOOL_NAME} handler received unsupported payload"
            )));
        }

        invocation
            .session
            .request_reflections_context_window_reset();
        Ok(FunctionToolOutput::from_text(
            "A fresh Reflections context window will start after this tool result is recorded."
                .to_string(),
            Some(true),
        ))
    }
}

pub struct ReflectionsGetContextRemainingHandler;

impl ToolHandler for ReflectionsGetContextRemainingHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        if !matches!(invocation.payload, ToolPayload::Function { .. }) {
            return Err(FunctionCallError::RespondToModel(format!(
                "{REFLECTIONS_GET_CONTEXT_REMAINING_TOOL_NAME} handler received unsupported payload"
            )));
        }

        let used_tokens = invocation.session.get_total_token_usage().await;
        let context_window_size = invocation.turn.model_context_window();
        let remaining_tokens =
            context_window_size.map(|size| size.saturating_sub(used_tokens).max(0));
        let content = json!({
            "context_window_size": context_window_size,
            "used_tokens": used_tokens,
            "remaining_tokens": remaining_tokens,
        })
        .to_string();

        Ok(FunctionToolOutput::from_text(content, Some(true)))
    }
}

pub struct ReflectionsListHandler;

impl ToolHandler for ReflectionsListHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let args: ListArgs = parse_function_args(&invocation, REFLECTIONS_LIST_TOOL_NAME)?;
        let (sidecar_path, rollout_path) = reflections_paths(&invocation).await?;
        let output = match args.collection.as_str() {
            "logs" => {
                crate::reflections::list_logs(&sidecar_path, &rollout_path, args.start, args.stop)
                    .await
            }
            "notes" => crate::reflections::list_notes(&sidecar_path, args.start, args.stop).await,
            other => Err(crate::reflections::StorageToolError::Invalid(format!(
                "unsupported Reflections collection `{other}`"
            ))),
        }
        .map_err(storage_error)?;

        serialize_output(output)
    }
}

pub struct ReflectionsReadHandler;

impl ToolHandler for ReflectionsReadHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let args: ReadArgs = parse_function_args(&invocation, REFLECTIONS_READ_TOOL_NAME)?;
        let (sidecar_path, rollout_path) = reflections_paths(&invocation).await?;
        match args.kind.as_str() {
            "log" => {
                let output = crate::reflections::read_log(
                    &sidecar_path,
                    &rollout_path,
                    &args.id,
                    args.start,
                    args.stop,
                )
                .await
                .map_err(storage_error)?;
                serialize_output(output)
            }
            "note" => {
                let output =
                    crate::reflections::read_note(&sidecar_path, &args.id, args.start, args.stop)
                        .await
                        .map_err(storage_error)?;
                serialize_output(output)
            }
            other => Err(FunctionCallError::RespondToModel(format!(
                "unsupported Reflections read kind `{other}`"
            ))),
        }
    }
}

pub struct ReflectionsSearchHandler;

impl ToolHandler for ReflectionsSearchHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let args: SearchArgs = parse_function_args(&invocation, REFLECTIONS_SEARCH_TOOL_NAME)?;
        if !matches!(args.scope.as_str(), "all" | "logs" | "notes") {
            return Err(FunctionCallError::RespondToModel(format!(
                "unsupported Reflections search scope `{}`",
                args.scope
            )));
        }
        let (sidecar_path, rollout_path) = reflections_paths(&invocation).await?;
        let output = crate::reflections::search(
            &sidecar_path,
            &rollout_path,
            &args.scope,
            &args.query,
            args.log_id.as_deref(),
            args.start,
            args.stop,
        )
        .await
        .map_err(storage_error)?;
        serialize_output(output)
    }
}

pub struct ReflectionsWriteNoteHandler;

impl ToolHandler for ReflectionsWriteNoteHandler {
    type Output = FunctionToolOutput;

    fn kind(&self) -> ToolKind {
        ToolKind::Function
    }

    async fn is_mutating(&self, _invocation: &ToolInvocation) -> bool {
        true
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let args: WriteNoteArgs =
            parse_function_args(&invocation, REFLECTIONS_WRITE_NOTE_TOOL_NAME)?;
        let (sidecar_path, _) = reflections_paths(&invocation).await?;
        let output = crate::reflections::write_note(
            &sidecar_path,
            &args.note_id,
            &args.operation,
            &args.content,
        )
        .await
        .map_err(storage_error)?;
        serialize_output(output)
    }
}

fn parse_function_args<T>(
    invocation: &ToolInvocation,
    tool_name: &str,
) -> Result<T, FunctionCallError>
where
    T: for<'de> Deserialize<'de>,
{
    let ToolPayload::Function { arguments } = &invocation.payload else {
        return Err(FunctionCallError::RespondToModel(format!(
            "{tool_name} handler received unsupported payload"
        )));
    };
    parse_arguments(arguments)
}

async fn reflections_paths(
    invocation: &ToolInvocation,
) -> Result<(PathBuf, PathBuf), FunctionCallError> {
    invocation
        .session
        .try_ensure_rollout_materialized()
        .await
        .map_err(|err| {
            FunctionCallError::RespondToModel(format!(
                "Reflections requires a persisted rollout path: {err}"
            ))
        })?;
    invocation.session.flush_rollout().await.map_err(|err| {
        FunctionCallError::RespondToModel(format!(
            "failed to flush Reflections rollout before reading storage: {err}"
        ))
    })?;
    let rollout_path = invocation.session.current_rollout_path().await.ok_or_else(|| {
        FunctionCallError::RespondToModel(
            "Reflections storage requires a persisted rollout path and is unavailable for ephemeral sessions"
                .to_string(),
        )
    })?;
    let sidecar_path = crate::reflections::sidecar_path_for_rollout(&rollout_path);
    Ok((sidecar_path, rollout_path))
}

fn storage_error(err: crate::reflections::StorageToolError) -> FunctionCallError {
    FunctionCallError::RespondToModel(err.to_string())
}

fn serialize_output<T>(output: T) -> Result<FunctionToolOutput, FunctionCallError>
where
    T: Serialize,
{
    serde_json::to_string(&output)
        .map(|content| FunctionToolOutput::from_text(content, Some(true)))
        .map_err(|err| {
            FunctionCallError::Fatal(format!(
                "failed to serialize Reflections tool output: {err}"
            ))
        })
}
