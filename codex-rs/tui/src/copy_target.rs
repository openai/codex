use std::sync::Arc;

use pulldown_cmark::CodeBlockKind;
use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;
use pulldown_cmark::TagEnd;

use crate::history_cell::HistoryCell;

const MAX_PREVIEW_CHARS: usize = 96;
const MAX_EXEC_CALLS: usize = 10;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum CopyTargetKind {
    AssistantResponse,
    CodeBlock,
    Command,
    Output,
}

impl CopyTargetKind {
    pub(crate) fn label(&self) -> &'static str {
        match self {
            Self::AssistantResponse => "Response",
            Self::CodeBlock => "Code",
            Self::Command => "Command",
            Self::Output => "Output",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyTarget {
    pub(crate) kind: CopyTargetKind,
    pub(crate) title: String,
    pub(crate) preview: String,
    pub(crate) content: String,
}

impl CopyTarget {
    pub(crate) fn new(
        kind: CopyTargetKind,
        title: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        let content = content.into();
        Self {
            kind,
            title: title.into(),
            preview: preview_for(&content),
            content,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CopyTargetGroup {
    pub(crate) targets: Vec<CopyTarget>,
}

impl CopyTargetGroup {
    pub(crate) fn new(targets: Vec<CopyTarget>) -> Self {
        Self { targets }
    }
}

pub(crate) fn build_copy_targets(
    last_agent_markdown: Option<&str>,
    cells: &[Arc<dyn HistoryCell>],
) -> Vec<CopyTarget> {
    let mut targets = Vec::new();

    if let Some(markdown) = last_agent_markdown.filter(|text| !text.is_empty()) {
        targets.push(CopyTarget::new(
            CopyTargetKind::AssistantResponse,
            "Last response",
            markdown.to_string(),
        ));

        for (idx, block) in code_blocks_from_markdown(markdown).into_iter().enumerate() {
            let title = match block.language {
                Some(language) => format!("Code block {} ({language})", idx + 1),
                None => format!("Code block {}", idx + 1),
            };
            targets.push(CopyTarget::new(
                CopyTargetKind::CodeBlock,
                title,
                block.content,
            ));
        }
    }

    let mut exec_groups = 0usize;
    for cell in cells.iter().rev() {
        for group in cell.copy_target_groups() {
            if exec_groups >= MAX_EXEC_CALLS {
                break;
            }
            targets.extend(group.targets);
            exec_groups += 1;
        }
        if exec_groups >= MAX_EXEC_CALLS {
            break;
        }
    }

    targets
}

pub(crate) fn output_text_for_copy(output: &str) -> String {
    trim_trailing_line_endings(strip_ansi_escape_sequences(output))
}

pub(crate) fn trim_trailing_line_endings(mut text: String) -> String {
    while text.ends_with('\n') || text.ends_with('\r') {
        text.pop();
    }
    text
}

#[derive(Debug, PartialEq, Eq)]
struct CodeBlock {
    language: Option<String>,
    content: String,
}

fn code_blocks_from_markdown(markdown: &str) -> Vec<CodeBlock> {
    let parser = Parser::new_ext(markdown, Options::all());
    let mut blocks = Vec::new();
    let mut current: Option<CodeBlock> = None;

    for event in parser {
        match event {
            Event::Start(Tag::CodeBlock(kind)) => {
                current = Some(CodeBlock {
                    language: language_from_code_block_kind(kind),
                    content: String::new(),
                });
            }
            Event::Text(text) => {
                if let Some(block) = &mut current {
                    block.content.push_str(&text);
                }
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some(mut block) = current.take() {
                    block.content = trim_trailing_line_endings(block.content);
                    if !block.content.is_empty() {
                        blocks.push(block);
                    }
                }
            }
            _ => {}
        }
    }

    blocks
}

fn language_from_code_block_kind(kind: CodeBlockKind<'_>) -> Option<String> {
    match kind {
        CodeBlockKind::Fenced(info) => info
            .split_whitespace()
            .next()
            .filter(|language| !language.is_empty())
            .map(ToOwned::to_owned),
        CodeBlockKind::Indented => None,
    }
}

fn preview_for(content: &str) -> String {
    let single_line = content
        .lines()
        .find(|line| !line.trim().is_empty())
        .unwrap_or("")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    truncate_chars(&single_line, MAX_PREVIEW_CHARS)
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    let mut chars = text.chars();
    let mut out: String = chars.by_ref().take(max_chars).collect();
    if chars.next().is_some() {
        out.push_str("...");
    }
    out
}

fn strip_ansi_escape_sequences(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch != '\u{1b}' {
            out.push(ch);
            continue;
        }

        match chars.peek().copied() {
            Some('[') => {
                chars.next();
                for c in chars.by_ref() {
                    if ('\u{40}'..='\u{7e}').contains(&c) {
                        break;
                    }
                }
            }
            Some(']') | Some('P') | Some('_') | Some('^') => {
                chars.next();
                let mut saw_esc = false;
                for c in chars.by_ref() {
                    if c == '\u{7}' || (saw_esc && c == '\\') {
                        break;
                    }
                    saw_esc = c == '\u{1b}';
                }
            }
            Some(_) => {
                chars.next();
            }
            None => {}
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn extracts_fenced_code_blocks_without_fences() {
        let markdown =
            "Intro\n\n```rust\nfn main() {\n    println!(\"hi\");\n}\n```\n\n- keep bullet\n";

        let blocks = code_blocks_from_markdown(markdown);

        assert_eq!(
            blocks,
            vec![CodeBlock {
                language: Some("rust".to_string()),
                content: "fn main() {\n    println!(\"hi\");\n}".to_string(),
            }]
        );
    }

    #[test]
    fn output_copy_strips_ansi_and_trailing_line_endings() {
        assert_eq!(
            output_text_for_copy("\u{1b}[31merror\u{1b}[0m\nnext\n"),
            "error\nnext"
        );
    }

    #[test]
    fn target_builder_keeps_markdown_bullets_in_response() {
        let targets = build_copy_targets(Some("- real bullet\n\ntext"), &[]);

        assert_eq!(
            targets,
            vec![CopyTarget {
                kind: CopyTargetKind::AssistantResponse,
                title: "Last response".to_string(),
                preview: "- real bullet".to_string(),
                content: "- real bullet\n\ntext".to_string(),
            }]
        );
    }
}
