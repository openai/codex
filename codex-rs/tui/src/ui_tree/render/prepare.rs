//! Prepare inline content and measure each row child before viewport layout.

use super::text::InlineText;
use super::text::Paint;
use super::text::PreparedText;
use crate::ui_tree::RowWrap;
use crate::ui_tree::UiNode;

#[derive(Debug, PartialEq)]
pub(super) struct PreparedNode {
    pub(super) kind: NodeKind,
    pub(super) widths: Widths,
}

#[derive(Debug, PartialEq)]
pub(super) struct Widths {
    pub(super) minimum: u64,
    pub(super) preferred: u64,
}

#[derive(Debug, PartialEq)]
pub(super) enum NodeKind {
    Text(PreparedText),
    Row {
        children: Vec<PreparedNode>,
        gap: u64,
        wrap: RowWrap,
        paint: Paint,
    },
}

impl PreparedNode {
    pub(super) fn from_node(node: UiNode, paint: Paint) -> Option<Self> {
        let mut items = Vec::new();
        let mut inline = InlineText::default();
        Self::collect_inline(&mut items, &mut inline, node, paint.clone());
        items.extend(Self::text(inline));
        if items.len() <= 1 {
            items.pop()
        } else {
            Self::row(items, /*gap*/ 0, RowWrap::Nowrap, paint)
        }
    }

    fn text(inline: InlineText) -> Option<Self> {
        let text = inline.finish();
        if text.lines.is_empty() {
            return None;
        }
        let mut widths = Widths {
            minimum: 0,
            preferred: 0,
        };
        for line in &text.lines {
            let mut width = 0;
            for chunk in line {
                let cells = chunk.width_cells as u64;
                widths.minimum = widths.minimum.max(cells);
                width += cells + u64::from(chunk.trailing_space.is_some());
            }
            widths.preferred = widths.preferred.max(width);
        }
        Some(Self {
            kind: NodeKind::Text(text),
            widths,
        })
    }

    fn row(mut children: Vec<Self>, gap: u64, wrap: RowWrap, paint: Paint) -> Option<Self> {
        children.retain(|child| {
            child.widths.preferred > 0
                || match &child.kind {
                    NodeKind::Row { .. } => true,
                    NodeKind::Text(text) => text.has_authored_break,
                }
        });
        if children.is_empty() {
            return None;
        }
        let gaps = gap.saturating_mul(children.len() as u64 - 1);
        let widths = Widths {
            minimum: match wrap {
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
                gap,
                wrap,
                paint,
            },
        })
    }

    fn collect_inline(
        items: &mut Vec<Self>,
        inline: &mut InlineText,
        node: UiNode,
        mut paint: Paint,
    ) {
        let children = match node {
            UiNode::String(raw) => {
                inline.push_leaf(raw, paint);
                return;
            }
            UiNode::Row { props, children } => {
                items.extend(Self::text(std::mem::take(inline)));
                if let Some(color) = props.background {
                    paint.background = Some(color);
                }
                let children = children
                    .into_iter()
                    .flatten()
                    .filter_map(|node| Self::from_node(node, paint.clone()))
                    .collect();
                items.extend(Self::row(children, props.gap, props.wrap, paint));
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
            Self::collect_inline(items, inline, node, paint.clone());
        }
    }
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
