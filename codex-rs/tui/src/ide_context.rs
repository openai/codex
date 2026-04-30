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
    active_file: Option<ActiveFile>,
    #[serde(default)]
    open_tabs: Vec<FileDescriptor>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct ActiveFile {
    #[serde(flatten)]
    descriptor: FileDescriptor,
    selection: Range,
    #[serde(default)]
    active_selection_content: String,
    #[serde(default)]
    selections: Vec<Range>,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
struct FileDescriptor {
    label: String,
    path: String,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Range {
    start: Position,
    end: Position,
}

#[derive(Debug, Clone, Deserialize, PartialEq)]
struct Position {
    line: u32,
    character: u32,
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
                    "fsPath": "/repo/src/main.rs",
                    "startLine": 2,
                    "endLine": 10
                }
            ],
            "processEnv": {
                "path": "/usr/bin"
            }
        });

        let context: IdeContext = serde_json::from_value(value).expect("deserialize ide context");
        assert_eq!(
            context,
            IdeContext {
                active_file: Some(ActiveFile {
                    descriptor: FileDescriptor {
                        label: "lib.rs".to_string(),
                        path: "src/lib.rs".to_string(),
                    },
                    selection: Range {
                        start: Position {
                            line: 1,
                            character: 2,
                        },
                        end: Position {
                            line: 3,
                            character: 4,
                        },
                    },
                    active_selection_content: "selected".to_string(),
                    selections: Vec::new(),
                }),
                open_tabs: vec![FileDescriptor {
                    label: "main.rs".to_string(),
                    path: "src/main.rs".to_string(),
                }],
            }
        );
    }
}
