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
