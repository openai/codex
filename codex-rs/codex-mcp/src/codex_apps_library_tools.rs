use std::collections::HashSet;
use std::sync::Arc;

use rmcp::model::Tool;
use rmcp::model::ToolAnnotations;
use serde_json::Map;
use serde_json::Value as JsonValue;

use crate::mcp::CODEX_APPS_MCP_SERVER_NAME;
use crate::mcp_connection_manager::ToolInfo;

pub const SEARCH_LIBRARY_FILES_TOOL_NAME: &str = "search_library_files";
pub const LIST_LIBRARY_DIRECTORY_NODES_TOOL_NAME: &str = "list_library_directory_nodes";
pub const DOWNLOAD_LIBRARY_FILE_TOOL_NAME: &str = "download_library_file";
pub const CREATE_LIBRARY_FILE_TOOL_NAME: &str = "create_library_file";
pub const WRITEBACK_LIBRARY_FILE_TOOL_NAME: &str = "writeback_library_file";

pub fn is_codex_apps_library_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        SEARCH_LIBRARY_FILES_TOOL_NAME
            | LIST_LIBRARY_DIRECTORY_NODES_TOOL_NAME
            | DOWNLOAD_LIBRARY_FILE_TOOL_NAME
            | CREATE_LIBRARY_FILE_TOOL_NAME
            | WRITEBACK_LIBRARY_FILE_TOOL_NAME
    )
}

pub(crate) fn append_builtin_codex_apps_library_tools(
    mut tools: Vec<ToolInfo>,
    server_instructions: Option<&str>,
) -> Vec<ToolInfo> {
    let existing_names = tools
        .iter()
        .filter(|tool| tool.server_name == CODEX_APPS_MCP_SERVER_NAME)
        .map(|tool| tool.tool.name.to_string())
        .collect::<HashSet<_>>();

    for tool in builtin_codex_apps_library_tools(server_instructions) {
        if existing_names.contains(tool.tool.name.as_ref()) {
            continue;
        }
        tools.push(tool);
    }

    tools
}

fn builtin_codex_apps_library_tools(server_instructions: Option<&str>) -> Vec<ToolInfo> {
    vec![
        library_tool(
            SEARCH_LIBRARY_FILES_TOOL_NAME,
            "Search or list recent library files through the ChatGPT File API. Use this for library discovery instead of local path walking. Optional arguments: q, limit, cursor, category, state.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "q": {
                        "type": "string",
                        "description": "Optional filename substring query. Omit to list recent files."
                    },
                    "limit": {
                        "type": "integer",
                        "minimum": 1,
                        "maximum": 200,
                        "description": "Maximum number of results to return. Defaults to 10."
                    },
                    "cursor": {
                        "type": "string",
                        "description": "Optional pagination cursor from a previous response."
                    },
                    "category": {
                        "type": "string",
                        "description": "Optional library file category filter."
                    },
                    "state": {
                        "type": "string",
                        "description": "Optional library file state filter."
                    }
                },
                "additionalProperties": false
            }),
            ToolAnnotations {
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
                read_only_hint: Some(true),
                title: None,
            },
            server_instructions,
        ),
        library_tool(
            LIST_LIBRARY_DIRECTORY_NODES_TOOL_NAME,
            "List the immediate child folders and files for a library directory through the ChatGPT File API. Use parent_directory_id omitted for the synthetic root.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "parent_directory_id": {
                        "type": "string",
                        "description": "Optional parent directory id. Omit to list the synthetic library root."
                    },
                    "cursor": {
                        "type": "string",
                        "description": "Optional pagination cursor from a previous response."
                    }
                },
                "additionalProperties": false
            }),
            ToolAnnotations {
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
                read_only_hint: Some(true),
                title: None,
            },
            server_instructions,
        ),
        library_tool(
            DOWNLOAD_LIBRARY_FILE_TOOL_NAME,
            "Download a library file into a managed per-thread local cache and return the hydrated local filesystem path. Use this when a task needs normal local file access.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_id": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Backing storage file_id for the library file."
                    },
                    "file_name": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Filename to use for the hydrated local copy."
                    },
                    "library_file_id": {
                        "type": "string",
                        "description": "Optional library object id for sidecar metadata."
                    }
                },
                "required": ["file_id", "file_name"],
                "additionalProperties": false
            }),
            ToolAnnotations {
                destructive_hint: Some(false),
                idempotent_hint: Some(true),
                open_world_hint: Some(false),
                read_only_hint: Some(true),
                title: None,
            },
            server_instructions,
        ),
        library_tool(
            CREATE_LIBRARY_FILE_TOOL_NAME,
            "Create a new text file in the user's library through the ChatGPT File API. V1 is create-new-only: do not use this to update, replace, or delete an existing library file.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "file_name": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Name for the new library file, including extension."
                    },
                    "content": {
                        "type": "string",
                        "description": "UTF-8 text content for the new file. Maximum size is 500 MB."
                    }
                },
                "required": ["file_name", "content"],
                "additionalProperties": false
            }),
            ToolAnnotations {
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
                read_only_hint: Some(false),
                title: None,
            },
            server_instructions,
        ),
        library_tool(
            WRITEBACK_LIBRARY_FILE_TOOL_NAME,
            "Write back a hydrated local library file by creating a new library file only if the local copy changed. V1 never updates, replaces, or deletes the source file.",
            serde_json::json!({
                "type": "object",
                "properties": {
                    "local_path": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Absolute local path returned by download_library_file for the hydrated copy."
                    },
                    "file_name": {
                        "type": "string",
                        "minLength": 1,
                        "description": "Optional name for the new library file. Defaults to the hydrated source filename."
                    }
                },
                "required": ["local_path"],
                "additionalProperties": false
            }),
            ToolAnnotations {
                destructive_hint: Some(false),
                idempotent_hint: Some(false),
                open_world_hint: Some(false),
                read_only_hint: Some(false),
                title: None,
            },
            server_instructions,
        ),
    ]
}

fn library_tool(
    name: &str,
    description: &str,
    input_schema: JsonValue,
    annotations: ToolAnnotations,
    server_instructions: Option<&str>,
) -> ToolInfo {
    ToolInfo {
        server_name: CODEX_APPS_MCP_SERVER_NAME.to_string(),
        callable_name: name.to_string(),
        callable_namespace: "mcp__codex_apps__".to_string(),
        server_instructions: server_instructions.map(str::to_string),
        tool: Tool {
            name: name.to_string().into(),
            title: None,
            description: Some(description.to_string().into()),
            input_schema: Arc::new(input_schema.as_object().cloned().unwrap_or_else(Map::new)),
            output_schema: None,
            annotations: Some(annotations),
            execution: None,
            icons: None,
            meta: None,
        },
        connector_id: None,
        connector_name: None,
        plugin_display_names: Vec::new(),
        connector_description: None,
    }
}
