use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolFuture;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::DEFAULT_READ_MAX_TOKENS;
use crate::READ_TOOL_NAME;
use crate::backend::MemoriesBackend;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;

use super::backend_error_to_function_call;
use super::function_tool;
use super::parse_args;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ReadArgs {
    path: String,
    #[schemars(range(min = 1))]
    line_offset: Option<usize>,
    #[schemars(range(min = 1))]
    max_lines: Option<usize>,
}

#[derive(Clone)]
pub(super) struct ReadTool<B> {
    pub(super) backend: B,
}

impl<B> ExtensionToolExecutor for ReadTool<B>
where
    B: MemoriesBackend,
{
    fn tool_name(&self) -> ToolName {
        ToolName::plain(READ_TOOL_NAME)
    }

    fn spec(&self) -> Option<ToolSpec> {
        Some(function_tool::<ReadArgs, ReadMemoryResponse>(
            READ_TOOL_NAME,
            "Read a Codex memory file by relative path, optionally starting at a 1-indexed line offset and limiting the number of lines returned.",
        ))
    }

    fn handle(&self, call: ToolCall) -> ExtensionToolFuture<'_> {
        let backend = self.backend.clone();
        Box::pin(async move {
            let args: ReadArgs = parse_args(&call)?;
            let response = backend
                .read(ReadMemoryRequest {
                    path: args.path,
                    line_offset: args.line_offset.unwrap_or(1),
                    max_lines: args.max_lines,
                    max_tokens: DEFAULT_READ_MAX_TOKENS,
                })
                .await
                .map_err(backend_error_to_function_call)?;
            Ok(JsonToolOutput::new(json!(response)))
        })
    }
}
