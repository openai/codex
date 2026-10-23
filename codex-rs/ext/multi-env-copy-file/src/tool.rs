//! Copies one file between selected environments using each endpoint's captured permissions.
//! Paths are absolute and bytes stay outside model context; failed copies may leave a partial target.

use std::collections::BTreeMap;

use codex_tools::FunctionCallError;
use codex_tools::JsonSchema;
use codex_tools::JsonToolOutput;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolCall;
use codex_tools::ToolEnvironment;
use codex_tools::ToolExecutor;
use codex_tools::ToolName;
use codex_tools::ToolPayload;
use codex_tools::ToolSpec;
use codex_utils_path_uri::LegacyAppPathString;
use futures::StreamExt;
use serde::Deserialize;
use serde_json::json;

const TOOL_NAME: &str = "multi_env_copy_file";
const TOOL_DESCRIPTION: &str = "Copy a file between two different environments without loading it into context. Returns bytes copied after completion. Interrupted copies may leave files partially written.";

pub(crate) struct CopyFileTool;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CopyFileArgs {
    source_environment_id: String,
    source_path: String,
    target_environment_id: String,
    target_path: String,
}

impl CopyFileArgs {
    fn parse(arguments: &str) -> Result<Self, FunctionCallError> {
        serde_json::from_str(arguments)
            .map_err(|error| FunctionCallError::RespondToModel(error.to_string()))
    }
}

impl<'call> ToolExecutor<ToolCall<'call>> for CopyFileTool {
    fn tool_name(&self) -> ToolName {
        ToolName::plain(TOOL_NAME)
    }

    fn spec(&self) -> ToolSpec {
        let fields = [
            (
                "source_environment_id",
                "Source environment ID from <environment_context>.",
            ),
            (
                "source_path",
                "Absolute file path in the source environment. Parent directories must exist.",
            ),
            (
                "target_environment_id",
                "Target environment ID from <environment_context>.",
            ),
            (
                "target_path",
                "Absolute file path in the target environment. Parent directories must exist.",
            ),
        ];
        let properties = fields
            .iter()
            .map(|(name, description)| {
                (
                    name.to_string(),
                    JsonSchema::string(Some(description.to_string())),
                )
            })
            .collect::<BTreeMap<_, _>>();
        ToolSpec::Function(ResponsesApiTool {
            name: TOOL_NAME.to_string(),
            description: TOOL_DESCRIPTION.to_string(),
            strict: false,
            defer_loading: None,
            parameters: JsonSchema::object(
                properties,
                Some(fields.iter().map(|(name, _)| name.to_string()).collect()),
                Some(false.into()),
            ),
            output_schema: None,
        })
    }

    fn environment_arguments(
        &self,
        payload: &ToolPayload,
    ) -> Result<Vec<(&'static str, String)>, FunctionCallError> {
        let ToolPayload::Function { arguments } = payload else {
            return Err(FunctionCallError::Fatal(
                "expected copy arguments".to_string(),
            ));
        };
        let args = CopyFileArgs::parse(arguments)?;
        if args.source_environment_id == args.target_environment_id {
            return Err(FunctionCallError::RespondToModel(
                "source and target environment IDs must differ. To copy within one environment, use a shell command.".to_string(),
            ));
        }
        Ok(vec![
            ("source_environment_id", args.source_environment_id),
            ("target_environment_id", args.target_environment_id),
        ])
    }

    fn handle<'a>(&'a self, call: ToolCall<'call>) -> codex_tools::ToolExecutorFuture<'a>
    where
        'call: 'a,
    {
        Box::pin(async move {
            let args = CopyFileArgs::parse(call.function_arguments()?)?;
            let [source, target] = call.environments.as_slice() else {
                return Err(FunctionCallError::Fatal(
                    "copy requires a source and a target environment".to_string(),
                ));
            };
            let resolve_path = |environment: &ToolEnvironment<'_>, path: &str| {
                let id = &environment.environment_id;
                let convention = environment.cwd.infer_path_convention().ok_or_else(|| {
                    FunctionCallError::RespondToModel(format!(
                        "invalid working directory path for environment {id}"
                    ))
                })?;
                LegacyAppPathString::from_string(path)
                    .to_path_uri(convention)
                    .map_err(|_| {
                        FunctionCallError::RespondToModel(format!(
                            "provide an absolute {convention} file path for environment {id}"
                        ))
                    })
            };
            let source_path = resolve_path(source, &args.source_path)?;
            let target_path = resolve_path(target, &args.target_path)?;
            // The target writer pulls one block at a time when it supports streaming writes.
            // For an older executor it buffers this stream and sends a single write instead.
            // Report only the I/O error kind to the model. Executor connection errors can
            // include credentials from a signed websocket URL in their message.
            let contents = source
                .fs()
                .read_file_stream(&source_path)
                .await
                .map_err(|error| {
                    FunctionCallError::RespondToModel(format!(
                        "could not open copy source: {}",
                        error.kind()
                    ))
                })?;
            let bytes_copied = target
                .fs()
                .write_file_stream(&target_path, contents.boxed())
                .await
                .map_err(|error| {
                    FunctionCallError::RespondToModel(format!(
                        "copy failed; target may be truncated or partially written: {}",
                        error.kind()
                    ))
                })?;
            let output = json!({"bytes_copied": bytes_copied});
            Ok(Box::new(JsonToolOutput::new(output)) as Box<dyn codex_tools::ToolOutput>)
        })
    }
}
