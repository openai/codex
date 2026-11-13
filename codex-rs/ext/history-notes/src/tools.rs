//! Registers each typed request with its backend and preserves model-only media in tool outputs.

use crate::backend::Handle;
use crate::backend::Request;
use crate::requests::HISTORY;
use crate::requests::HistoryListItems;
use crate::requests::HistoryListWindows;
use crate::requests::HistoryReadItem;
use crate::requests::HistorySearchContents;
use crate::requests::NOTES;
use crate::requests::Namespace;
use crate::requests::NotesAppendToFile;
use crate::requests::NotesListFilesByPrefix;
use crate::requests::NotesReadFile;
use crate::requests::NotesSearchContents;
use crate::requests::NotesWriteFile;
use codex_extension_api::FunctionCallError;
use codex_extension_api::ResponsesApiTool;
use codex_extension_api::ToolCall;
use codex_extension_api::ToolExecutor;
use codex_extension_api::ToolExecutorFuture;
use codex_extension_api::ToolName;
use codex_extension_api::ToolOutput;
use codex_extension_api::ToolPayload;
use codex_extension_api::ToolSpec;
use codex_extension_api::parse_tool_input_schema;
use codex_protocol::models::FunctionCallOutputBody;
use codex_protocol::models::FunctionCallOutputContentItem;
use codex_protocol::models::FunctionCallOutputPayload;
use codex_protocol::models::ImageReference;
use codex_protocol::models::ResponseInputItem;
use codex_protocol::openai_models::HistoryToolMessages;
use codex_protocol::openai_models::NotesToolMessages;
use codex_protocol::openai_models::ToolMessage;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ToolExposure;
use codex_tools::ToolOutputSchema;
use codex_tools::with_catalog_tool_message;
use codex_utils_output_truncation::TruncationPolicy;
use schemars::JsonSchema;
use schemars::r#gen::SchemaSettings;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;
use std::marker::PhantomData;
use std::sync::Arc;

type Executor = Arc<dyn for<'call> ToolExecutor<ToolCall<'call>>>;

// Keep the catalog key, tool name, request type and HTTP route in one registration.
macro_rules! register_tools {
    ($( $catalog:ident: $messages:ty => $namespace:ident {
        $( $name:ident: $request:ty $( [parallel = $parallel:literal] )? ),+ $(,)?
    } ),+ $(,)?) => {
        $($(impl Request for $request {
            const NAMESPACE: Namespace = $namespace;
            const NAME: &'static str = stringify!($name);
            $(const PARALLEL: bool = $parallel;)?
        })+)+

        pub(crate) fn history_notes_tools<B>(
            backend: B,
            $($catalog: Option<&$messages>,)+
        ) -> Vec<Executor>
        where
            $($(B: Handle<$request>,)+)+
        {
            vec![$($(Arc::new(HistoryNotesTool::<$request, B> {
                request: PhantomData,
                backend: backend.clone(),
                spec: tool_spec::<$request, <B as Handle<$request>>::Output>(
                    $catalog.and_then(|messages| messages.namespace_description.as_deref()),
                    $catalog.and_then(|messages| messages.$name.as_ref()),
                ),
            }) as Executor,)+)+]
        }
    };
}

register_tools! {
    history: HistoryToolMessages => HISTORY {
        list_windows: HistoryListWindows,
        list_items: HistoryListItems,
        read_item: HistoryReadItem,
        search_contents: HistorySearchContents,
    },
    notes: NotesToolMessages => NOTES {
        list_files_by_prefix: NotesListFilesByPrefix,
        read_file: NotesReadFile,
        search_contents: NotesSearchContents,
        append_to_file: NotesAppendToFile [parallel = false],
        write_file: NotesWriteFile [parallel = false],
    },
}

pub(crate) fn schema<T: JsonSchema>(option_add_null_type: bool) -> Value {
    let schema = SchemaSettings::draft2019_09()
        .with(|settings| {
            settings.inline_subschemas = true;
            settings.option_add_null_type = option_add_null_type;
        })
        .into_generator()
        .into_root_schema_for::<T>()
        .schema;
    json!(schema)
}

#[expect(
    clippy::expect_used,
    reason = "the bundled request schemas are derived from fixed, documented types"
)]
fn tool_spec<R: Request, S: ToolResponse>(
    namespace_description: Option<&str>,
    message: Option<&ToolMessage>,
) -> ToolSpec {
    let mut parameters = parse_tool_input_schema(&schema::<R>(/*option_add_null_type*/ false))
        .expect("valid history and notes request schema");
    // Schemars sorts required keys. The published note write schemas put text before path.
    if let Some(required) = &mut parameters.required {
        required.sort_by_key(|name| name != "text");
    }
    let description = parameters
        .description
        .take()
        .expect("history and notes requests have descriptions")
        .into_string();
    let tool = ResponsesApiTool {
        name: R::NAME.to_string(),
        description,
        strict: false,
        parameters,
        output_schema: S::output_schema(),
        defer_loading: None,
    };
    let tool = S::with_catalog_message(tool, message);
    ToolSpec::Namespace(ResponsesApiNamespace {
        name: R::NAMESPACE.name.to_string(),
        description: namespace_description
            .unwrap_or(R::NAMESPACE.description)
            .to_string(),
        tools: vec![ResponsesApiNamespaceTool::Function(tool)],
    })
}

struct HistoryNotesTool<R, B> {
    request: PhantomData<R>,
    backend: B,
    spec: ToolSpec,
}

impl<'call, R: Request, B: Handle<R>> ToolExecutor<ToolCall<'call>> for HistoryNotesTool<R, B> {
    fn tool_name(&self) -> ToolName {
        ToolName::namespaced(R::NAMESPACE.name, R::NAME)
    }

    fn spec(&self) -> ToolSpec {
        self.spec.clone()
    }

    fn exposure(&self) -> ToolExposure {
        B::EXPOSURE
    }

    fn supports_parallel_tool_calls(&self) -> bool {
        R::PARALLEL
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let input = parse_arguments(call.function_arguments()?)?;
            let response = self
                .backend
                .handle(call.truncation_policy, input)
                .await
                .map_err(FunctionCallError::RespondToModel)?;
            response.into_tool_output(call.truncation_policy)
        })
    }
}

fn parse_arguments<T: DeserializeOwned>(arguments: &str) -> Result<T, FunctionCallError> {
    serde_json::from_str(if arguments.trim().is_empty() {
        "{}"
    } else {
        arguments
    })
    .map_err(|err| FunctionCallError::RespondToModel(err.to_string()))
}

/// Supplies the advertised output schema and renders a backend response for the model.
pub(crate) trait ToolResponse {
    fn output_schema() -> Option<ToolOutputSchema> {
        None
    }

    fn with_catalog_message(
        tool: ResponsesApiTool,
        message: Option<&ToolMessage>,
    ) -> ResponsesApiTool {
        with_catalog_tool_message(tool, message)
    }

    fn into_tool_output(
        self,
        budget: TruncationPolicy,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError>;
}

impl ToolResponse for Value {
    fn into_tool_output(
        self,
        _budget: TruncationPolicy,
    ) -> Result<Box<dyn ToolOutput>, FunctionCallError> {
        Ok(Box::new(HistoryNotesToolOutput::new(self)?))
    }
}

struct HistoryNotesToolOutput {
    result: Value,
    output: FunctionCallOutputPayload,
}

impl HistoryNotesToolOutput {
    fn new(mut result: Value) -> Result<Self, FunctionCallError> {
        // Separate attachments before serializing any text or retaining log output.
        let images = result.as_object_mut().and_then(|map| map.remove("images"));
        // The server applies the requested output budget before encryption.
        let mut output = match result.get("encrypted_output").and_then(Value::as_str) {
            Some(encrypted_content) => FunctionCallOutputPayload::from_content_items(vec![
                FunctionCallOutputContentItem::EncryptedContent {
                    encrypted_content: encrypted_content.to_string(),
                },
            ]),
            None => FunctionCallOutputPayload::from_text(result.to_string()),
        };
        if let Some(images) = images {
            let invalid_image = || {
                FunctionCallError::RespondToModel(
                    "History backend returned invalid image content.".to_string(),
                )
            };
            let images = images.as_array().ok_or_else(invalid_image)?;
            let mut content = match output.body {
                FunctionCallOutputBody::Text(text) => {
                    vec![FunctionCallOutputContentItem::InputText { text }]
                }
                FunctionCallOutputBody::ContentItems(content) => content,
            };
            for image in images {
                let data = image
                    .get("data")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_image)?;
                let mime_type = image
                    .get("mime_type")
                    .and_then(Value::as_str)
                    .ok_or_else(invalid_image)?;
                let detail =
                    serde_json::from_value(image.get("detail").cloned().unwrap_or(Value::Null))
                        .map_err(|_| invalid_image())?;
                content.push(FunctionCallOutputContentItem::InputImage {
                    image: ImageReference::Inline {
                        image_url: format!("data:{mime_type};base64,{data}"),
                    },
                    detail,
                });
            }
            output = FunctionCallOutputPayload::from_content_items(content);
        }
        Ok(Self { result, output })
    }
}

impl ToolOutput for HistoryNotesToolOutput {
    fn log_output(&self) -> String {
        JsonToolOutput::new(self.result.clone()).log_output()
    }

    fn success_for_logging(&self) -> bool {
        true
    }

    fn post_tool_use_response(&self, _call_id: &str, _payload: &ToolPayload) -> Option<Value> {
        // Hooks must not receive model-only image attachments.
        Some(self.result.clone())
    }

    fn to_response_item(&self, call_id: &str, _payload: &ToolPayload) -> ResponseInputItem {
        ResponseInputItem::FunctionCallOutput {
            call_id: call_id.to_string(),
            output: self.output.clone(),
        }
    }

    fn code_mode_result(&self, _payload: &ToolPayload) -> Value {
        Value::String("History tools are unavailable in code mode.".to_string())
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
