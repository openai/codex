use codex_extension_api::JsonToolOutput;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolSpec;
use codex_otel::MetricsClient;
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::json;

use crate::ADD_AD_HOC_NOTE_TOOL_NAME;
use crate::backend::AddAdHocMemoryNoteRequest;
use crate::backend::AddAdHocMemoryNoteResponse;
use crate::backend::MemoriesBackend;
use crate::metrics::record_tool_call;

use super::backend_error_to_function_call;
use super::memory_function_tool;
use super::memory_tool_name;
use super::parse_args;

#[derive(Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AddAdHocNoteArgs {
    /// Name of the note file to create, in
    /// YYYY-MM-DDTHH-MM-SS-<slug>.md format. The slug must use only lowercase
    /// ASCII letters, digits, and hyphens.
    #[schemars(
        length(min = 24, max = 128),
        regex(pattern = r"^\d{4}-\d{2}-\d{2}T\d{2}-\d{2}-\d{2}-[a-z0-9][a-z0-9-]{0,79}\.md$")
    )]
    filename: String,
    /// Verbatim Markdown note to append to the ad-hoc memory notes.
    #[schemars(length(min = 1))]
    note: String,
}

#[derive(Clone)]
pub(super) struct AddAdHocNoteTool<B> {
    pub(super) backend: B,
    pub(super) metrics_client: Option<MetricsClient>,
}

impl<B> ToolExecutor<ToolCall> for AddAdHocNoteTool<B>
where
    B: MemoriesBackend,
{
    fn tool_name(&self) -> ToolName {
        memory_tool_name(ADD_AD_HOC_NOTE_TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        memory_function_tool::<AddAdHocNoteArgs, AddAdHocMemoryNoteResponse>(
            ADD_AD_HOC_NOTE_TOOL_NAME,
            "Create one append-only ad-hoc memory note after the user explicitly asks Codex to remember, forget, or update something.",
        )
    }

    fn handle<'a>(&'a self, call: ToolCall) -> codex_extension_api::ToolExecutionFuture<'a> {
        Box::pin(async move {
            let backend = self.backend.clone();
            let args: AddAdHocNoteArgs = parse_args(&call)?;
            let response = backend
                .add_ad_hoc_note(AddAdHocMemoryNoteRequest {
                    filename: args.filename,
                    note: args.note,
                })
                .await;
            record_tool_call(
                self.metrics_client.as_ref(),
                ADD_AD_HOC_NOTE_TOOL_NAME,
                "ad_hoc_notes",
                response.is_ok(),
                "not_applicable",
            );
            let response = response.map_err(backend_error_to_function_call)?;
            Ok(Box::new(JsonToolOutput::new(json!(response))) as Box<dyn ToolOutput>)
        })
    }
}
