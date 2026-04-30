//! IDE context data model and public helpers for TUI `/ide` support.

mod ipc;
mod prompt;
#[cfg(windows)]
mod windows_pipe;

pub(crate) use ipc::fetch_ide_context;
pub(crate) use prompt::apply_ide_context_to_user_input;
pub(crate) use prompt::extract_prompt_request_with_offset;
pub(crate) use prompt::has_prompt_context;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct IdeContext {
    pub(crate) active_file: Option<ActiveFile>,
    #[serde(default)]
    pub(crate) open_tabs: Vec<FileDescriptor>,
    #[serde(default)]
    pub(crate) process_env: Option<IdeProcessEnv>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ActiveFile {
    #[serde(flatten)]
    pub(crate) descriptor: FileDescriptor,
    pub(crate) selection: Range,
    #[serde(default)]
    pub(crate) active_selection_content: String,
    #[serde(default)]
    pub(crate) selections: Vec<Range>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FileDescriptor {
    pub(crate) label: String,
    pub(crate) path: String,
    #[serde(rename = "fsPath")]
    pub(crate) fs_path: String,
    #[serde(default)]
    pub(crate) start_line: Option<u32>,
    #[serde(default)]
    pub(crate) end_line: Option<u32>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct Range {
    pub(crate) start: Position,
    pub(crate) end: Position,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct Position {
    pub(crate) line: u32,
    pub(crate) character: u32,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
pub(crate) struct IdeProcessEnv {
    pub(crate) path: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use serde_json::json;

    #[test]
    fn deserializes_existing_ide_context_shape() {
        let value = json!({
            "activeFile": {
                "label": "lib.rs",
                "path": "src/lib.rs",
                "fsPath": "/repo/src/lib.rs",
                "selection": {
                    "start": { "line": 1, "character": 2 },
                    "end": { "line": 3, "character": 4 }
                },
                "activeSelectionContent": "selected",
                "selections": []
            },
            "openTabs": [
                {
                    "label": "main.rs",
                    "path": "src/main.rs",
                    "fsPath": "/repo/src/main.rs"
                }
            ]
        });

        let context: IdeContext = serde_json::from_value(value).expect("deserialize ide context");
        assert_eq!(
            context
                .active_file
                .as_ref()
                .map(|file| file.descriptor.path.as_str()),
            Some("src/lib.rs")
        );
        assert_eq!(context.open_tabs.len(), 1);
    }
}
