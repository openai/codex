use std::sync::Arc;

use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::Value;

use crate::backend::MemoriesBackendError;
use crate::local::LocalMemoriesBackend;
use crate::schema;

mod list;
mod read;
mod search;

const LIST_TOOL_NAME: &str = "memory_list";
const READ_TOOL_NAME: &str = "memory_read";
const SEARCH_TOOL_NAME: &str = "memory_search";

pub(crate) fn memory_tools(backend: LocalMemoriesBackend) -> Vec<Arc<dyn ExtensionToolExecutor>> {
    vec![
        Arc::new(list::ListTool {
            backend: backend.clone(),
        }),
        Arc::new(read::ReadTool {
            backend: backend.clone(),
        }),
        Arc::new(search::SearchTool { backend }),
    ]
}

fn function_tool<I: JsonSchema, O: JsonSchema>(name: &str, description: &str) -> ToolSpec {
    ToolSpec::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters: parse_tool_input_schema(&schema::input_schema_for::<I>())
            .unwrap_or_else(|err| panic!("generated input schema for {name} should parse: {err}")),
        output_schema: Some(schema::output_schema_for::<O>()),
    })
}

fn parse_args<T: for<'de> Deserialize<'de>>(call: &ToolCall) -> Result<T, FunctionCallError> {
    let arguments = call.function_arguments()?;
    let value = if arguments.trim().is_empty() {
        Value::Object(serde_json::Map::new())
    } else {
        serde_json::from_str(arguments)
            .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))?
    };
    serde_json::from_value(value).map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

fn clamp_max_results(requested: Option<usize>, default: usize, max: usize) -> usize {
    requested.unwrap_or(default).clamp(1, max)
}

fn backend_error_to_function_call(err: MemoriesBackendError) -> FunctionCallError {
    match err {
        MemoriesBackendError::InvalidPath { .. }
        | MemoriesBackendError::InvalidCursor { .. }
        | MemoriesBackendError::NotFound { .. }
        | MemoriesBackendError::InvalidLineOffset
        | MemoriesBackendError::InvalidMaxLines
        | MemoriesBackendError::LineOffsetExceedsFileLength
        | MemoriesBackendError::NotFile { .. }
        | MemoriesBackendError::EmptyQuery
        | MemoriesBackendError::InvalidMatchWindow => {
            FunctionCallError::RespondToModel(err.to_string())
        }
        MemoriesBackendError::Io(_) => FunctionCallError::Fatal(err.to_string()),
    }
}
