//! Prepare inline content and measure container children before viewport layout.

use super::text::InlineText;
use super::text::Paint;
use super::text::PreparedText;
use crate::ui_tree::RowProps;
use crate::ui_tree::RowWrap;
use crate::ui_tree::UiNode;

#[derive(Debug, PartialEq)]
pub(super) struct RowPaint {
    pub(super) gap: Paint,
    pub(super) leading: Paint,
}

#[derive(Debug, PartialEq)]
pub(super) struct PreparedNode {
    pub(super) kind: NodeKind,
    pub(super) widths: Widths,
}

#[derive(Debug, Default, PartialEq)]
pub(super) struct Widths {
    pub(super) minimum: u64,
    pub(super) preferred: u64,
}

#[derive(Debug, PartialEq)]
pub(super) enum NodeKind {
    Text(PreparedText),
    Col {
        children: Vec<PreparedNode>,
        gap: u64,
    },
    Row {
        children: Vec<PreparedNode>,
        props: RowProps,
        paint: RowPaint,
    },
    Empty,
}

/// A column interrupts inline flow even when its contents have zero height.
#[derive(Default)]
struct Flow {
    blocks: Vec<PreparedNode>,
    line: Vec<PreparedNode>,
    paint: Paint,
}

impl Flow {
    fn take_line(&mut self) -> Option<PreparedNode> {
        if self.line.len() <= 1 {
            self.line.pop()
        } else {
            PreparedNode::row(
                std::mem::take(&mut self.line),
                RowProps::default(),
                self.paint.clone(),
            )
        }
    }
}

impl PreparedNode {
    pub(super) fn from_node(node: UiNode, paint: Paint) -> Option<Self> {
        let mut flow = Flow {
            paint: paint.clone(),
            ..Flow::default()
        };
        let mut inline = InlineText::default();
        Self::collect_inline(&mut flow, &mut inline, node, paint);
        flow.line.extend(Self::text(inline));
        let last = flow.take_line();
        if flow.blocks.is_empty() {
            return last;
        }
        flow.blocks.extend(last);
        if flow.blocks.len() == 1 {
            return flow.blocks.pop();
        }
        Some(Self::col(flow.blocks, /*gap*/ 0))
    }

    fn col(children: Vec<Self>, gap: u64) -> Self {
        let mut widths = Widths::default();
        for child in &children {
            widths.minimum = widths.minimum.max(child.widths.minimum);
            widths.preferred = widths.preferred.max(child.widths.preferred);
        }
        Self {
            widths,
            kind: NodeKind::Col { children, gap },
        }
    }

    fn text(inline: InlineText) -> Option<Self> {
        let text = inline.finish();
        if text.lines.is_empty() {
            return None;
        }
        let mut widths = Widths::default();
        for line in &text.lines {
            let mut width = 0;
            let mut word = 0;
            for (i, chunk) in line.iter().enumerate() {
                let cells = chunk.width_at(width);
                if chunk.prefers_break_before(i) {
                    word = 0;
                }
                word += cells;
                widths.minimum = widths.minimum.max(word as u64);
                width += cells + usize::from(chunk.trailing_space.is_some());
            }
            widths.preferred = widths.preferred.max(width as u64);
        }
        Some(Self {
            kind: NodeKind::Text(text),
            widths,
        })
    }

    fn row(mut children: Vec<Self>, props: RowProps, paint: Paint) -> Option<Self> {
        children.retain(|child| {
            // An authored blank line occupies a row even when it paints no cells.
            !child.is_empty()
        });
        if children.is_empty() {
            return None;
        }
        let gaps = props.gap.saturating_mul(children.len() as u64 - 1);
        let widths = Widths {
            minimum: match props.wrap {
                RowWrap::Nowrap => children
                    .iter()
                    .fold(gaps, |w, c| w.saturating_add(c.widths.minimum)),
                RowWrap::Wrap => children
                    .iter()
                    .map(|c| c.widths.minimum)
                    .max()
                    .unwrap_or_default(),
            },
            preferred: children
                .iter()
                .fold(gaps, |w, c| w.saturating_add(c.widths.preferred)),
        };
        Some(Self {
            widths,
            kind: NodeKind::Row {
                children,
                props,
                paint: RowPaint {
                    leading: Paint {
                        background: paint.background,
                        ..Paint::default()
                    },
                    gap: paint,
                },
            },
        })
    }

    /// In flex layout, consecutive strings form one anonymous item unless entirely whitespace.
    fn flex_items(children: Vec<Option<UiNode>>, paint: &Paint) -> Vec<Self> {
        let mut items = Vec::new();
        let mut strings = InlineText::default();
        for child in children.into_iter().flatten() {
            match child {
                UiNode::String(raw) => strings.push_flex_leaf(raw, paint.clone()),
                child => {
                    items.extend(Self::text(std::mem::take(&mut strings)));
                    // An explicit empty element still separates flex gaps.
                    items.push(
                        Self::from_node(child, paint.clone()).unwrap_or_else(|| Self {
                            kind: NodeKind::Empty,
                            widths: Widths::default(),
                        }),
                    );
                }
            }
        }
        items.extend(Self::text(strings));
        items
    }

    fn collect_inline(flow: &mut Flow, inline: &mut InlineText, node: UiNode, mut paint: Paint) {
        let parent_preserves_spaces = paint.preserve_whitespace;
        let children = match node {
            UiNode::String(raw) => {
                inline.push_leaf(raw, paint);
                return;
            }
            UiNode::Code { children } => {
                paint.inline_code = true;
                paint.preserve_whitespace = true;
                paint.foreground = None;
                paint.background = None;
                paint.modifiers = paint
                    .modifiers
                    .remove_modifier(ratatui::style::Modifier::BOLD);
                children
            }
            UiNode::Col { props, children } => {
                flow.line.extend(Self::text(std::mem::take(inline)));
                let line = flow.take_line();
                flow.blocks.extend(line);
                let children = Self::flex_items(children, &paint);
                flow.blocks.push(Self::col(children, props.gap));
                return;
            }
            UiNode::Row { props, children } => {
                flow.line.extend(Self::text(std::mem::take(inline)));
                if let Some(color) = props.background {
                    paint.background = Some(color);
                }
                let children = if props.justify.is_some() {
                    Self::flex_items(children, &paint)
                } else {
                    // Existing providers apply the gap to each authored child.
                    children
                        .into_iter()
                        .flatten()
                        .filter_map(|node| Self::from_node(node, paint.clone()))
                        .collect()
                };
                flow.line.extend(Self::row(children, props, paint));
                return;
            }
            UiNode::Text { props, children } => {
                props.apply_to(&mut paint);
                children
            }
            UiNode::Link { props, children } => {
                paint.href = Some(props.href.0.into());
                paint.modifiers = paint.modifiers.underlined();
                children
            }
            UiNode::Bold { children } => {
                paint.modifiers = paint.modifiers.bold();
                children
            }
            UiNode::Italic { children } => {
                paint.modifiers = paint.modifiers.italic();
                children
            }
            UiNode::Underline { children } => {
                paint.modifiers = paint.modifiers.underlined();
                children
            }
        };
        for node in children.into_iter().flatten() {
            Self::collect_inline(flow, inline, node, paint.clone());
        }
        if !parent_preserves_spaces {
            inline.end_span();
        }
    }

    fn is_empty(&self) -> bool {
        self.widths.preferred == 0
            && matches!(&self.kind, NodeKind::Text(text) if !text.has_authored_break)
    }
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
