//! Prompt rendering for IDE context injected into TUI user turns.

use codex_app_server_protocol::ByteRange;
use codex_app_server_protocol::TextElement;
use codex_app_server_protocol::UserInput;

use super::IdeContext;

const MAX_ACTIVE_SELECTION_CHARS: usize = 40_000;
const MAX_OPEN_TABS: usize = 100;
const MAX_OPEN_TABS_CHARS: usize = 20_000;
// Match the desktop app and IDE extension delimiter exactly. IDE context is serialized into the
// raw prompt before this marker, then transcript rendering strips back to the request after the last
// marker. Keeping the same marker and stripping semantics lets threads created with IDE context in
// one surface replay cleanly in the others.
const PROMPT_REQUEST_BEGIN: &str = "## My request for Codex:";

pub(crate) fn apply_ide_context_to_user_input(
    context: &IdeContext,
    items: &mut Vec<UserInput>,
) -> bool {
    let Some(context_text) = render_prompt_context(context) else {
        return false;
    };

    let prefix = format!("{context_text}\n{PROMPT_REQUEST_BEGIN}\n");
    if let Some(text_index) = items
        .iter()
        .position(|item| matches!(item, UserInput::Text { .. }))
    {
        // Prefix the existing text item in place so image and text items keep
        // the same relative order they had in the user's original submission.
        let item = std::mem::replace(
            &mut items[text_index],
            UserInput::Text {
                text: String::new(),
                text_elements: Vec::new(),
            },
        );
        let UserInput::Text {
            text,
            text_elements,
        } = item
        else {
            unreachable!("position matched a text item");
        };
        items[text_index] = prefixed_text_input(prefix, text, text_elements);
    } else {
        items.insert(
            0,
            UserInput::Text {
                text: prefix,
                text_elements: Vec::new(),
            },
        );
    }

    true
}

pub(crate) fn has_prompt_context(context: &IdeContext) -> bool {
    render_prompt_context(context).is_some()
}

pub(crate) fn extract_prompt_request_with_offset(message: &str) -> (&str, usize) {
    let Some((before_request, request)) = message.rsplit_once(PROMPT_REQUEST_BEGIN) else {
        return (message, 0);
    };

    let request_start = before_request.len() + PROMPT_REQUEST_BEGIN.len();
    let trimmed_request = request.trim();
    let leading_trimmed_len = request.len() - request.trim_start().len();
    (trimmed_request, request_start + leading_trimmed_len)
}

fn prefixed_text_input(prefix: String, text: String, text_elements: Vec<TextElement>) -> UserInput {
    let prefix_len = prefix.len();
    UserInput::Text {
        text: format!("{prefix}{text}"),
        text_elements: text_elements
            .into_iter()
            .map(|element| {
                let range = element.byte_range.clone();
                TextElement::new(
                    ByteRange {
                        start: range.start + prefix_len,
                        end: range.end + prefix_len,
                    },
                    element.placeholder().map(str::to_string),
                )
            })
            .collect(),
    }
}

fn render_prompt_context(context: &IdeContext) -> Option<String> {
    let mut ide_context_section = String::new();

    if let Some(active_file) = &context.active_file {
        ide_context_section.push_str(&format!(
            "\n## Active file: {}\n",
            active_file.descriptor.path
        ));
    }

    if let Some(active_file) = &context.active_file {
        let selected_ranges = if active_file.selections.is_empty() {
            std::slice::from_ref(&active_file.selection)
        } else {
            active_file.selections.as_slice()
        }
        .iter()
        .filter(|range| range.start != range.end)
        .collect::<Vec<_>>();

        if !selected_ranges.is_empty()
            && (active_file.active_selection_content.is_empty() || selected_ranges.len() > 1)
        {
            if selected_ranges.len() == 1 {
                ide_context_section.push_str("\n## Active selection range:\n");
            } else {
                ide_context_section.push_str("\n## Active selection ranges:\n");
            }
            for range in selected_ranges {
                // Render ranges as 1-based positions for the prompt.
                let start_line = range.start.line + 1;
                let start_column = range.start.character + 1;
                let end_line = range.end.line + 1;
                let end_column = range.end.character + 1;
                ide_context_section.push_str(&format!(
                    "- {}: line {start_line}, column {start_column} to line {end_line}, column {end_column}\n",
                    active_file.descriptor.path
                ));
            }
        }
    }

    if let Some(active_file) = &context.active_file
        && !active_file.active_selection_content.is_empty()
    {
        ide_context_section.push_str("\n## Active selection of the file:\n");
        let selection = active_file.active_selection_content.as_str();
        if let Some((truncate_at, _)) = selection.char_indices().nth(MAX_ACTIVE_SELECTION_CHARS) {
            ide_context_section.push_str(&selection[..truncate_at]);
            ide_context_section.push_str(&format!(
                "\n[Selection truncated to {MAX_ACTIVE_SELECTION_CHARS} characters.]\n"
            ));
        } else {
            ide_context_section.push_str(selection);
        }
    }

    if !context.open_tabs.is_empty() {
        ide_context_section.push_str("\n## Open tabs:\n");
        let mut rendered_tabs = 0;
        let mut rendered_tab_chars = 0;
        for tab in &context.open_tabs {
            if rendered_tabs >= MAX_OPEN_TABS {
                break;
            }

            let tab_line = format!("- {}: {}\n", tab.label, tab.path);
            if rendered_tab_chars + tab_line.len() > MAX_OPEN_TABS_CHARS {
                break;
            }

            ide_context_section.push_str(&tab_line);
            rendered_tabs += 1;
            rendered_tab_chars += tab_line.len();
        }

        let omitted_tabs = context.open_tabs.len() - rendered_tabs;
        if omitted_tabs > 0 {
            ide_context_section.push_str(&format!("[{omitted_tabs} open tabs omitted.]\n"));
        }
    }

    if ide_context_section.is_empty() {
        None
    } else {
        Some(format!(
            "# Context from my IDE setup:\n{ide_context_section}"
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::super::ActiveFile;
    use super::super::FileDescriptor;
    use super::super::IdeContext;
    use super::super::Position;
    use super::super::Range;
    use super::*;
    use pretty_assertions::assert_eq;
    use std::path::PathBuf;

    fn descriptor(label: &str, path: &str) -> FileDescriptor {
        FileDescriptor {
            label: label.to_string(),
            path: path.to_string(),
        }
    }

    #[test]
    fn render_prompt_context_matches_app_format() {
        let context = IdeContext {
            active_file: Some(ActiveFile {
                descriptor: descriptor("lib.rs", "src/lib.rs"),
                selection: Range {
                    start: Position {
                        line: 4,
                        character: 0,
                    },
                    end: Position {
                        line: 6,
                        character: 1,
                    },
                },
                active_selection_content: "fn selected() {}".to_string(),
                selections: Vec::new(),
            }),
            open_tabs: vec![
                descriptor("lib.rs", "src/lib.rs"),
                descriptor("main.rs", "src/main.rs"),
            ],
        };

        assert_eq!(
            render_prompt_context(&context),
            Some(
                "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n## Active selection of the file:\nfn selected() {}\n## Open tabs:\n- lib.rs: src/lib.rs\n- main.rs: src/main.rs\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn render_prompt_context_omits_empty_context() {
        let context = IdeContext {
            active_file: None,
            open_tabs: Vec::new(),
        };

        assert_eq!(render_prompt_context(&context), None);
    }

    #[test]
    fn apply_ide_context_uses_desktop_prompt_request_delimiter() {
        let context = IdeContext {
            active_file: Some(ActiveFile {
                descriptor: descriptor("lib.rs", "src/lib.rs"),
                selection: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 0,
                    },
                },
                active_selection_content: String::new(),
                selections: Vec::new(),
            }),
            open_tabs: Vec::new(),
        };
        let text = "Ask $figma".to_string();
        let mut items = vec![
            UserInput::LocalImage {
                path: PathBuf::from("/tmp/screenshot.png"),
            },
            UserInput::Text {
                text,
                text_elements: vec![TextElement::new(
                    ByteRange { start: 4, end: 10 },
                    Some("$figma".to_string()),
                )],
            },
        ];

        assert!(apply_ide_context_to_user_input(&context, &mut items));

        let expected_prefix = "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n## My request for Codex:\n";
        let prefix_len = expected_prefix.len();
        assert_eq!(
            items,
            vec![
                UserInput::LocalImage {
                    path: PathBuf::from("/tmp/screenshot.png"),
                },
                UserInput::Text {
                    text: format!("{expected_prefix}Ask $figma"),
                    text_elements: vec![TextElement::new(
                        ByteRange {
                            start: prefix_len + 4,
                            end: prefix_len + 10,
                        },
                        Some("$figma".to_string()),
                    )],
                },
            ]
        );
    }

    #[test]
    fn extract_prompt_request_returns_text_after_last_delimiter() {
        let message =
            "# Context\n## My request for Codex:\nFirst\n## My request for Codex:\n  Second\n";

        assert_eq!(
            extract_prompt_request_with_offset(message),
            ("Second", message.find("Second").expect("request offset"))
        );
    }

    #[test]
    fn render_prompt_context_includes_selection_ranges_without_content() {
        let first_range = Range {
            start: Position {
                line: 1,
                character: 2,
            },
            end: Position {
                line: 1,
                character: 5,
            },
        };
        let second_range = Range {
            start: Position {
                line: 3,
                character: 0,
            },
            end: Position {
                line: 4,
                character: 1,
            },
        };
        let context = IdeContext {
            active_file: Some(ActiveFile {
                descriptor: descriptor("lib.rs", "src/lib.rs"),
                selection: first_range.clone(),
                active_selection_content: String::new(),
                selections: vec![first_range, second_range],
            }),
            open_tabs: Vec::new(),
        };

        assert_eq!(
            render_prompt_context(&context),
            Some(
                "# Context from my IDE setup:\n\n## Active file: src/lib.rs\n\n## Active selection ranges:\n- src/lib.rs: line 2, column 3 to line 2, column 6\n- src/lib.rs: line 4, column 1 to line 5, column 2\n"
                    .to_string()
            )
        );
    }

    #[test]
    fn render_prompt_context_truncates_large_selection() {
        let context = IdeContext {
            active_file: Some(ActiveFile {
                descriptor: descriptor("large.txt", "large.txt"),
                selection: Range {
                    start: Position {
                        line: 0,
                        character: 0,
                    },
                    end: Position {
                        line: 0,
                        character: 1,
                    },
                },
                active_selection_content: format!("{}tail", "a".repeat(MAX_ACTIVE_SELECTION_CHARS)),
                selections: Vec::new(),
            }),
            open_tabs: Vec::new(),
        };

        let rendered = render_prompt_context(&context).expect("rendered IDE context");
        assert!(rendered.contains(&format!(
            "[Selection truncated to {MAX_ACTIVE_SELECTION_CHARS} characters.]"
        )));
        assert!(!rendered.contains("tail"));
    }

    #[test]
    fn render_prompt_context_omits_excess_open_tabs() {
        let open_tabs = (0..MAX_OPEN_TABS + 2)
            .map(|index| descriptor(&format!("file-{index}.rs"), &format!("src/file-{index}.rs")))
            .collect::<Vec<_>>();
        let context = IdeContext {
            active_file: None,
            open_tabs,
        };

        let rendered = render_prompt_context(&context).expect("rendered IDE context");
        assert!(rendered.contains("- file-99.rs: src/file-99.rs\n"));
        assert!(!rendered.contains("- file-100.rs: src/file-100.rs\n"));
        assert!(rendered.contains("[2 open tabs omitted.]\n"));
    }
}
