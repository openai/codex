use codex_extension_api::ExtensionToolExecutor;
use codex_extension_api::ExtensionToolFuture;
use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolName;
use codex_extension_api::ToolSpec;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::backend::DEFAULT_READ_MAX_TOKENS;
use crate::backend::MemoriesBackend;
use crate::backend::ReadMemoryRequest;
use crate::backend::ReadMemoryResponse;
use crate::local::LocalMemoriesBackend;

use super::READ_TOOL_NAME;
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
pub(super) struct ReadTool {
    pub(super) backend: LocalMemoriesBackend,
}

impl ExtensionToolExecutor for ReadTool {
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

#[cfg(test)]
mod tests {
    use codex_extension_api::ToolPayload;
    use codex_tools::ToolOutput;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    use super::*;

    #[tokio::test]
    async fn read_tool_reads_memory_file() {
        let tempdir = tempfile::tempdir().expect("tempdir");
        let memory_root = tempdir.path().join("memories");
        tokio::fs::create_dir_all(&memory_root)
            .await
            .expect("create memories dir");
        tokio::fs::write(
            memory_root.join("MEMORY.md"),
            "first line\nsecond needle line\nthird line\n",
        )
        .await
        .expect("write memory");
        let tool = ReadTool {
            backend: LocalMemoriesBackend::from_memory_root(&memory_root),
        };
        let payload = ToolPayload::Function {
            arguments: json!({
                "path": "MEMORY.md",
                "line_offset": 2,
                "max_lines": 1
            })
            .to_string(),
        };

        let output = tool
            .handle(ToolCall {
                call_id: "call-1".to_string(),
                tool_name: ToolName::plain(READ_TOOL_NAME),
                payload: payload.clone(),
            })
            .await
            .expect("read should succeed");

        assert_eq!(
            output.post_tool_use_response("call-1", &payload),
            Some(json!({
                "path": "MEMORY.md",
                "content": "second needle line\n",
                "start_line_number": 2,
                "truncated": true
            }))
        );
    }
}
