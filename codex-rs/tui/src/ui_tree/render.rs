//! Render the visible prefix of a horizontal display tree into one terminal line.

use super::UiNode;
use super::UiTree;
use crate::history_cell::sanitize_user_text;
use crate::line_truncation::truncate_line_to_width;
use crate::terminal_hyperlinks::HyperlinkLine;
use crate::terminal_hyperlinks::TerminalHyperlink;
use ratatui::style::Modifier;

const MAX_DEPTH: usize = 16;
const MAX_NODES: usize = 256;
const MAX_TEXT_BYTES: usize = 16 * 1024;

struct Budget {
    nodes: usize,
    text_bytes: usize,
}

impl UiNode {
    /// Render one row. On overflow, fit an unlinked ellipsis inside the supplied width.
    pub(crate) fn render_line(&self, width: u16) -> HyperlinkLine {
        let width = usize::from(width);
        if width == 0 {
            return HyperlinkLine::default();
        }
        let mut budget = Budget {
            nodes: MAX_NODES,
            text_bytes: MAX_TEXT_BYTES,
        };
        let mut line = node_line(Some(self), /*depth*/ 0, &mut budget).unwrap_or_default();
        if line.width() > width {
            line.line = truncate_line_to_width(line.line, width - 1);
            line.clip_hyperlinks_to_width(line.width());
            line.line.push_span("…");
        }
        line
    }
}

fn node_line(node: Option<&UiNode>, depth: usize, budget: &mut Budget) -> Option<HyperlinkLine> {
    if budget.nodes == 0 {
        return None;
    }
    budget.nodes -= 1;
    if depth > MAX_DEPTH {
        return None;
    }
    match node? {
        UiNode::Text { props, .. } => {
            let raw = &props.text;
            if budget.text_bytes == 0 && !raw.is_empty() {
                return None;
            }
            let end = raw.floor_char_boundary(raw.len().min(budget.text_bytes));
            let first = raw[..end].split('\n').next().unwrap_or_default();
            budget.text_bytes -= first.len();
            let visible = sanitize_user_text(first.into()).replace('\t', "    ");
            Some(visible.into())
        }
        UiNode::Link {
            props, children, ..
        } => {
            let mut line = row_line(children, /*gap*/ 0, depth, budget)?;
            for span in &mut line.line.spans {
                span.style = span.style.add_modifier(Modifier::UNDERLINED);
            }
            let width = line.width();
            if width > 0 {
                line.hyperlinks = vec![TerminalHyperlink::web(0..width, props.href.0.clone())];
            }
            Some(line)
        }
        UiNode::Row {
            props, children, ..
        } => {
            // A spacing unit is four points; a terminal cell is eight points across.
            let gap = (props.gap.unwrap_or(0.0).clamp(0.0, 32.0) / 2.0).ceil() as usize;
            row_line(children, gap, depth, budget)
        }
        UiNode::Spacer { .. } => Some(" ".into()),
        UiNode::Divider { .. } => Some("│".into()),
    }
}

fn row_line(
    children: &[UiTree],
    gap: usize,
    depth: usize,
    budget: &mut Budget,
) -> Option<HyperlinkLine> {
    let mut result: Option<HyperlinkLine> = None;
    let mut used = 0;
    for child in children {
        if budget.nodes == 0 {
            break;
        }
        let Some(child) = node_line(child.as_ref(), depth + 1, budget) else {
            continue;
        };
        let spacing = if result.is_some() { gap } else { 0 };
        let line = result.get_or_insert_default();
        if spacing > 0 {
            line.line.push_span(" ".repeat(spacing));
        }
        let offset = used + spacing;
        used = offset + child.width();
        line.line.spans.extend(child.line.spans);
        line.hyperlinks
            .extend(child.hyperlinks.into_iter().map(|mut link| {
                link.columns = link.columns.start + offset..link.columns.end + offset;
                link
            }));
    }
    result
}
