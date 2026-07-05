//! Semantic plain-text projection for mouse selection.
//!
//! This deliberately follows Markdown structure rather than removing characters from rendered
//! terminal rows. Styling delimiters and table chrome never enter the canonical copy string.

use std::path::Path;

use pulldown_cmark::Event;
use pulldown_cmark::Options;
use pulldown_cmark::Parser;
use pulldown_cmark::Tag;

pub(crate) fn render_markdown_selection_text(input: &str, cwd: Option<&Path>) -> String {
    let lines = super::render_markdown_lines_with_width_and_cwd(input, /*width*/ None, cwd);
    let mut text = String::new();
    for (line_index, line) in lines.into_iter().enumerate() {
        if line_index > 0 {
            text.push('\n');
        }
        for span in line.line.spans {
            text.push_str(&span.content);
        }
    }
    text
}

pub(crate) fn selection_text_contains_table(input: &str) -> bool {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TABLES);
    Parser::new_ext(input, options).any(|event| matches!(event, Event::Start(Tag::Table(_))))
}

#[cfg(test)]
#[path = "selection_tests.rs"]
mod tests;
