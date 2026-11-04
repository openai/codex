//! Render the visible prefix of a horizontal display tree into one terminal line.

use super::MAX_TEXT_BYTES;
use super::UiNode;
use super::UiTree;
use crate::history_cell::sanitize_user_text;
use crate::line_truncation::truncate_line_to_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use ratatui::style::Modifier;

impl UiTree {
    /// Render one row. On overflow, fit an unlinked ellipsis inside the supplied width.
    pub(crate) fn render_line(&self, width: u16) -> HyperlinkLine {
        let width = usize::from(width);
        let Some(root) = self.0.as_ref().filter(|_| width > 0) else {
            return HyperlinkLine::default();
        };
        let mut line = node_line(root, width);
        if line.width() > width {
            line.line = truncate_line_to_width(line.line, width - 1);
            line.clip_hyperlinks_to_width(line.width());
            line.line.push_span("…");
        }
        line
    }
}

fn node_line(node: &UiNode, width: usize) -> HyperlinkLine {
    match node {
        UiNode::String(raw) => {
            // Built-in values are constructed directly, bypassing the wire decoder.
            let raw = &raw[..raw.floor_char_boundary(MAX_TEXT_BYTES)];
            let first = raw.split('\n').next().unwrap_or_default();
            sanitize_user_text(first.into())
                .replace('\t', "    ")
                .into()
        }
        UiNode::Link { props, children } => {
            let mut line = row_line(children, /*gap*/ 0, width);
            for span in &mut line.line.spans {
                span.style = span.style.add_modifier(Modifier::UNDERLINED);
            }
            let width = line.width();
            if width > 0 {
                line.hyperlinks = vec![TerminalHyperlink::web(0..width, props.href.0.clone())];
            }
            line
        }
        UiNode::Row { props, children } => row_line(children, props.gap, width),
        UiNode::Text { children, .. } => row_line(children, /*gap*/ 0, width),
    }
}

fn row_line(children: &[Option<UiNode>], gap: u64, width: usize) -> HyperlinkLine {
    let mut result = HyperlinkLine::default();
    let mut used = 0;
    for child in children.iter().flatten() {
        let child = node_line(child, width);
        let child_width = child.width();
        if child_width == 0 {
            continue;
        }
        if used > 0 && gap > 0 {
            // One column past the viewport is sufficient to detect overflow.
            let spacing = gap.min((width + 1 - used) as u64) as usize;
            result.line.push_span(" ".repeat(spacing));
            used += spacing;
        }
        let offset = used;
        result.line.spans.extend(child.line.spans);
        result
            .hyperlinks
            .extend(child.hyperlinks.into_iter().map(|mut link| {
                link.columns = link.columns.start + offset..link.columns.end + offset;
                link
            }));
        used += child_width;
        if used > width {
            break;
        }
    }
    result
}
