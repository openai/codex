use crate::function_tool::FunctionCallError;
use crate::session::session::Session;
use crate::tools::context::ToolInvocation;
use crate::tools::context::ToolPayload;
use crate::tools::file_broker::CodeModeFileBroker;
use crate::tools::file_broker::FileBrokerError;
use crate::tools::handlers::files_spec::FILES_COPY_TOOL_NAME;
use crate::tools::handlers::files_spec::FILES_EXPORT_FOR_TOOL_NAME;
use crate::tools::handlers::files_spec::FILES_MATERIALIZE_TOOL_NAME;
use crate::tools::handlers::files_spec::FILES_NAMESPACE;
use crate::tools::handlers::files_spec::create_files_namespace_tool;
use crate::tools::handlers::parse_arguments;
use crate::tools::registry::ToolExecutor;
use crate::tools::registry::ToolHandler;
use codex_tools::FileRef;
use codex_tools::JsonToolOutput;
use codex_tools::ToolName;
use codex_tools::ToolSpec;
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub(crate) struct FilesMaterializeHandler;
pub(crate) struct FilesCopyHandler;
pub(crate) struct FilesExportForToolHandler;

#[derive(Debug, Deserialize)]
struct SourceTargetArgs {
    source_uri: String,
    target_uri: String,
}

#[derive(Debug, Deserialize)]
struct ExportForToolArgs {
    file_uri: String,
    mime_type: String,
}

impl ToolExecutor<ToolInvocation> for FilesMaterializeHandler {
    type Output = JsonToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(FILES_NAMESPACE, FILES_MATERIALIZE_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(create_files_namespace_tool())
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let session = invocation.session.clone();
        let args = parse_source_target_args(invocation.payload, FILES_MATERIALIZE_TOOL_NAME)?;
        let broker = CodeModeFileBroker::new(invocation.turn.cwd.as_path());
        let result = match broker.copy(
            &parse_file_ref(&args.source_uri)?,
            &parse_file_ref(&args.target_uri)?,
        ) {
            Ok(result) => result,
            Err(error) => return Err(file_broker_error(error, &session).await),
        };
        Ok(JsonToolOutput::new(json!({
            "source_uri": result.source_ref,
            "file_uri": result.target_ref,
            "byte_count": result.byte_count,
        })))
    }
}

impl ToolHandler for FilesMaterializeHandler {}

impl ToolExecutor<ToolInvocation> for FilesCopyHandler {
    type Output = JsonToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(FILES_NAMESPACE, FILES_COPY_TOOL_NAME)
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let session = invocation.session.clone();
        let args = parse_source_target_args(invocation.payload, FILES_COPY_TOOL_NAME)?;
        let broker = CodeModeFileBroker::new(invocation.turn.cwd.as_path());
        let result = match broker.copy(
            &parse_file_ref(&args.source_uri)?,
            &parse_file_ref(&args.target_uri)?,
        ) {
            Ok(result) => result,
            Err(error) => return Err(file_broker_error(error, &session).await),
        };
        Ok(JsonToolOutput::new(json!({
            "source_uri": result.source_ref,
            "target_uri": result.target_ref,
            "byte_count": result.byte_count,
        })))
    }
}

impl ToolHandler for FilesCopyHandler {}

impl ToolExecutor<ToolInvocation> for FilesExportForToolHandler {
    type Output = JsonToolOutput;

    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(FILES_NAMESPACE, FILES_EXPORT_FOR_TOOL_NAME)
    }

    async fn handle(&self, invocation: ToolInvocation) -> Result<Self::Output, FunctionCallError> {
        let session = invocation.session.clone();
        let args = parse_export_for_tool_args(invocation.payload)?;
        let broker = CodeModeFileBroker::new(invocation.turn.cwd.as_path());
        let result = match broker.export_data_uri(&parse_file_ref(&args.file_uri)?, &args.mime_type)
        {
            Ok(result) => result,
            Err(error) => return Err(file_broker_error(error, &session).await),
        };
        Ok(JsonToolOutput::new(json!({
            "file_uri": result.source_ref,
            "mime_type": result.mime_type,
            "data_uri": result.data_uri,
            "byte_count": result.byte_count,
        })))
    }
}

impl ToolHandler for FilesExportForToolHandler {}

fn parse_source_target_args(
    payload: ToolPayload,
    tool_name: &str,
) -> Result<SourceTargetArgs, FunctionCallError> {
    parse_payload_arguments(payload, tool_name).and_then(|arguments| parse_arguments(&arguments))
}

fn parse_export_for_tool_args(
    payload: ToolPayload,
) -> Result<ExportForToolArgs, FunctionCallError> {
    parse_payload_arguments(payload, FILES_EXPORT_FOR_TOOL_NAME)
        .and_then(|arguments| parse_arguments(&arguments))
}

fn parse_payload_arguments(
    payload: ToolPayload,
    tool_name: &str,
) -> Result<String, FunctionCallError> {
    match payload {
        ToolPayload::Function { arguments } => Ok(arguments),
        _ => Err(FunctionCallError::RespondToModel(format!(
            "{FILES_NAMESPACE}.{tool_name} received unsupported payload"
        ))),
    }
}

fn parse_file_ref(raw: &str) -> Result<FileRef, FunctionCallError> {
    FileRef::parse(raw)
        .map_err(|err| FunctionCallError::RespondToModel(format!("invalid file ref: {err}")))
}

async fn file_broker_error(error: FileBrokerError, session: &Arc<Session>) -> FunctionCallError {
    if !error.should_include_active_provider_status() {
        return FunctionCallError::RespondToModel(error.to_string());
    }

    let manager = session.services.mcp_connection_manager.read().await;
    let mcp_tools = manager.list_all_tools().await;
    let registry = CodeModeFileBroker::active_provider_registry(&mcp_tools);
    FunctionCallError::RespondToModel(format!(
        "{}. Active broker providers: {}",
        error,
        registry.summary()
    ))
}
