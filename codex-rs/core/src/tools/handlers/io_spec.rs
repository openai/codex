use codex_tools::JsonSchema;
use codex_tools::ResponsesApiNamespace;
use codex_tools::ResponsesApiNamespaceTool;
use codex_tools::ResponsesApiTool;
use codex_tools::ToolSpec;
use std::collections::BTreeMap;

pub(crate) const IO_NAMESPACE: &str = "io";
pub(crate) const IO_EXPERIMENTAL_TOOL: &str = "io";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct IoToolOptions {
    pub include_environment_id: bool,
}

pub(crate) fn create_io_tool_namespace(options: IoToolOptions) -> ToolSpec {
    ToolSpec::Namespace(ResponsesApiNamespace {
        name: IO_NAMESPACE.to_string(),
        description: "Native Code Mode file I/O for the selected Codex environment. Paths are resolved inside the target environment filesystem and may be relative, absolute, or explicit `env://current/...` refs.".to_string(),
        tools: vec![
            tool(
                "read_file",
                "Read the complete contents of a UTF-8 text file from the selected environment filesystem.",
                path_parameters(options, &["path"]),
            ),
            tool(
                "write_file",
                "Create or overwrite a UTF-8 text file in the selected environment filesystem.",
                write_file_parameters(options),
            ),
            tool(
                "edit_file",
                "Apply exact text replacements to a UTF-8 text file in the selected environment filesystem.",
                edit_file_parameters(options),
            ),
            tool(
                "create_directory",
                "Create a directory in the selected environment filesystem.",
                create_directory_parameters(options),
            ),
            tool(
                "list_directory",
                "List entries in a directory from the selected environment filesystem.",
                path_parameters(options, &["path"]),
            ),
            tool(
                "get_file_info",
                "Get basic metadata for a file or directory in the selected environment filesystem.",
                path_parameters(options, &["path"]),
            ),
            tool(
                "list_allowed_directories",
                "List readable and writable filesystem roots for the selected environment.",
                environment_parameters(options),
            ),
        ],
    })
}

fn tool(name: &str, description: &str, parameters: JsonSchema) -> ResponsesApiNamespaceTool {
    ResponsesApiNamespaceTool::Function(ResponsesApiTool {
        name: name.to_string(),
        description: description.to_string(),
        strict: false,
        defer_loading: None,
        parameters,
        output_schema: None,
    })
}

fn environment_parameters(options: IoToolOptions) -> JsonSchema {
    let mut properties = BTreeMap::new();
    maybe_insert_environment_id(&mut properties, options);
    JsonSchema::object(properties, /*required*/ None, Some(false.into()))
}

fn path_parameters(options: IoToolOptions, required: &[&str]) -> JsonSchema {
    let mut properties = BTreeMap::from([(
        "path".to_string(),
        JsonSchema::string(Some(path_description())),
    )]);
    maybe_insert_environment_id(&mut properties, options);
    JsonSchema::object(
        properties,
        Some(required.iter().map(|name| (*name).to_string()).collect()),
        Some(false.into()),
    )
}

fn write_file_parameters(options: IoToolOptions) -> JsonSchema {
    let mut properties = BTreeMap::from([
        (
            "path".to_string(),
            JsonSchema::string(Some(path_description())),
        ),
        (
            "content".to_string(),
            JsonSchema::string(Some("UTF-8 text content to write to the file.".to_string())),
        ),
    ]);
    maybe_insert_environment_id(&mut properties, options);
    JsonSchema::object(
        properties,
        Some(vec!["path".to_string(), "content".to_string()]),
        Some(false.into()),
    )
}

fn create_directory_parameters(options: IoToolOptions) -> JsonSchema {
    let mut properties = BTreeMap::from([
        (
            "path".to_string(),
            JsonSchema::string(Some(path_description())),
        ),
        (
            "recursive".to_string(),
            JsonSchema::boolean(Some(
                "Whether to create parent directories. Defaults to true.".to_string(),
            )),
        ),
    ]);
    maybe_insert_environment_id(&mut properties, options);
    JsonSchema::object(
        properties,
        Some(vec!["path".to_string()]),
        Some(false.into()),
    )
}

fn edit_file_parameters(options: IoToolOptions) -> JsonSchema {
    let edit_schema = JsonSchema::object(
        BTreeMap::from([
            (
                "oldText".to_string(),
                JsonSchema::string(Some(
                    "Text to replace. Must match exactly once.".to_string(),
                )),
            ),
            (
                "newText".to_string(),
                JsonSchema::string(Some("Replacement text.".to_string())),
            ),
        ]),
        Some(vec!["oldText".to_string(), "newText".to_string()]),
        Some(false.into()),
    );
    let mut properties = BTreeMap::from([
        (
            "path".to_string(),
            JsonSchema::string(Some(path_description())),
        ),
        (
            "edits".to_string(),
            JsonSchema::array(
                edit_schema,
                Some("Exact text replacements to apply in order.".to_string()),
            ),
        ),
        (
            "dryRun".to_string(),
            JsonSchema::boolean(Some(
                "When true, validate and preview the edit without writing. Defaults to false."
                    .to_string(),
            )),
        ),
    ]);
    maybe_insert_environment_id(&mut properties, options);
    JsonSchema::object(
        properties,
        Some(vec!["path".to_string(), "edits".to_string()]),
        Some(false.into()),
    )
}

fn maybe_insert_environment_id(
    properties: &mut BTreeMap<String, JsonSchema>,
    options: IoToolOptions,
) {
    if options.include_environment_id {
        properties.insert(
            "environment_id".to_string(),
            JsonSchema::string(Some(
                "Optional environment id from the <environment_context> block. If omitted, uses the primary environment.".to_string(),
            )),
        );
    }
}

fn path_description() -> String {
    "Path in the selected environment filesystem. Relative paths are resolved against that environment's cwd; absolute paths are interpreted inside that environment. `env://current/...` may be used for explicit environment refs.".to_string()
}
