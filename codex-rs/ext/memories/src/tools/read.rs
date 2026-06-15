use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use codex_otel::MetricsClient;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::DEFAULT_READ_MAX_TOKENS;
use crate::READ_TOOL_NAME;
use crate::backend::MemoriesBackend;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;
use crate::metrics::record_tool_call;
use crate::metrics::scope_from_path;
use crate::metrics::truncated_tag;

use super::backend_error_to_function_call;
use super::memory_function_tool;
use super::memory_tool_name;
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
    pub(super) metrics_client: Option<MetricsClient>,
}

impl<B> ToolExecutor<ToolCall> for ReadTool<B>
where
    B: MemoriesBackend,
{
    fn tool_name(&self) -> ToolName {
        memory_tool_name(READ_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        memory_function_tool::<ReadArgs, ReadMemoryResponse>(
            READ_TOOL_NAME,
            "Read a Codex memory file by relative path, optionally starting at a 1-indexed line offset and limiting the number of lines returned.",
        )
    }

    fn handle(&self, call: ToolCall) -> codex_extension_api::ToolExecutorFuture<'_> {
        Box::pin(self.handle_call(call))
    }
}

impl<B> ReadTool<B>
where
    B: MemoriesBackend,
{
    async fn handle_call(
        &self,
        call: ToolCall,
    ) -> Result<Box<dyn codex_extension_api::ToolOutput>, codex_extension_api::FunctionCallError>
    {
        let backend = self.backend.clone();
        let args: ReadArgs = parse_args(&call)?;
        let path = args.path;
        let scope = scope_from_path(path.as_str());
        let response = backend
            .read(ReadMemoryRequest {
                path: path.clone(),
                line_offset: args.line_offset.unwrap_or(1),
                max_lines: args.max_lines,
                max_tokens: DEFAULT_READ_MAX_TOKENS,
            })
            .await;
        record_tool_call(
            self.metrics_client.as_ref(),
            READ_TOOL_NAME,
            scope,
            response.is_ok(),
            truncated_tag(response.as_ref().ok().map(|response| response.truncated)),
        );
        let response = response.map_err(backend_error_to_function_call)?;
        Ok(Box::new(JsonToolOutput::new(json!(response))))
    }
}
