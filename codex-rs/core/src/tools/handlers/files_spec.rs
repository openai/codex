use codex_tools::JsonSchema;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) const FILES_NAMESPACE: &str = "files";
pub(crate) const FILES_MATERIALIZE_TOOL_NAME: &str = "materialize";
pub(crate) const FILES_COPY_TOOL_NAME: &str = "copy";
pub(crate) const FILES_EXPORT_FOR_TOOL_NAME: &str = "export_for_tool";

pub(crate) fn create_files_namespace_tool() -> ToolSpec {
    ToolSpec::Namespace(ResponsesApiNamespace {
        name: FILES_NAMESPACE.to_string(),
        description: "Move Code Mode file refs between the workspace and provider/tool boundaries."
            .to_string(),
        tools: vec![
            ResponsesApiNamespaceTool::Function(materialize_tool()),
            ResponsesApiNamespaceTool::Function(copy_tool()),
            ResponsesApiNamespaceTool::Function(export_for_tool_tool()),
        ],
    })
}

fn materialize_tool() -> ResponsesApiTool {
    ResponsesApiTool {
        name: FILES_MATERIALIZE_TOOL_NAME.to_string(),
        description: "Materialize a source file ref into an environment file ref. The initial POC supports env://current/... refs; provider adapters can be added behind the same contract.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: source_target_schema(),
        output_schema: None,
    }
}

fn copy_tool() -> ResponsesApiTool {
    ResponsesApiTool {
        name: FILES_COPY_TOOL_NAME.to_string(),
        description:
            "Copy bytes from one file ref to another without exposing provider credentials to the model."
                .to_string(),
        strict: false,
        defer_loading: None,
        parameters: source_target_schema(),
        output_schema: None,
    }
}

fn export_for_tool_tool() -> ResponsesApiTool {
    ResponsesApiTool {
        name: FILES_EXPORT_FOR_TOOL_NAME.to_string(),
        description: "Export a file ref as a base64 data URI for tools that declare fileParam-style inputs. Use this from Code Mode generated code immediately before invoking the destination tool; do not log the returned data URI.".to_string(),
        strict: false,
        defer_loading: None,
        parameters: JsonSchema::object(
            BTreeMap::from([
                (
                    "file_uri".to_string(),
                    JsonSchema::string(Some("Source file ref, for example env://current/out.png.".to_string())),
                ),
                (
                    "mime_type".to_string(),
                    JsonSchema::string(Some("MIME type to use in the returned data URI, for example image/png.".to_string())),
                ),
            ]),
            Some(vec!["file_uri".to_string(), "mime_type".to_string()]),
            Some(false.into()),
        ),
        output_schema: None,
    }
}

fn source_target_schema() -> JsonSchema {
    JsonSchema::object(
        BTreeMap::from([
            (
                "source_uri".to_string(),
                JsonSchema::string(Some(
                    "Source file ref, for example env://current/report.pdf.".to_string(),
                )),
            ),
            (
                "target_uri".to_string(),
                JsonSchema::string(Some(
                    "Target file ref, for example env://current/out/report.pdf.".to_string(),
                )),
            ),
        ]),
        Some(vec!["source_uri".to_string(), "target_uri".to_string()]),
        Some(false.into()),
    )
}

#[cfg(test)]
#[path = "files_spec_tests.rs"]
mod tests;
