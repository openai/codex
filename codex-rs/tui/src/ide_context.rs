//! IDE context data model and public helpers for TUI `/ide` support.

mod ipc;
mod prompt;
#[cfg(windows)]
mod windows_pipe;

pub(crate) use ipc::IdeContextClient;
pub(crate) use ipc::IdeContextError;
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
