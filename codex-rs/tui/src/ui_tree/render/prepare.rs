//! Prepare inline content and measure each row child before viewport layout.

use super::text::InlineText;
use super::text::Paint;
use super::text::PreparedText;
use crate::ui_tree::RowWrap;
use crate::ui_tree::UiNode;
use crate::ui_tree::theme::RenderTheme;
use ratatui::style::Style;

pub(super) struct PreparedNode<'a> {
    pub(super) kind: NodeKind<'a>,
    pub(super) widths: Widths,
}

pub(super) struct Widths {
    pub(super) minimum: u64,
    pub(super) preferred: u64,
}

pub(super) enum NodeKind<'a> {
    Text(PreparedText<'a>),
    Row {
        children: Vec<PreparedNode<'a>>,
        gap: u64,
        wrap: RowWrap,
        paint: Paint<'a>,
    },
}

impl<'a> PreparedNode<'a> {
    pub(super) fn from_node(
        node: &'a UiNode,
        style: Style,
        href: Option<&'a str>,
        theme: &RenderTheme<'_>,
    ) -> Option<Self> {
        let mut items = Vec::new();
        let mut inline = InlineText::default();
        Self::collect_inline(&mut items, &mut inline, node, style, href, theme);
        items.extend(Self::text(inline));
        if items.len() <= 1 {
            items.pop()
        } else {
            Self::row(
                items,
                /*gap*/ 0,
                RowWrap::Nowrap,
                Paint { style, href },
            )
        }
    }

    fn text(inline: InlineText<'a>) -> Option<Self> {
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

    fn row(mut children: Vec<Self>, gap: u64, wrap: RowWrap, paint: Paint<'a>) -> Option<Self> {
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
        inline: &mut InlineText<'a>,
        node: &'a UiNode,
        mut style: Style,
        mut href: Option<&'a str>,
        theme: &RenderTheme<'_>,
    ) {
        let children = match node {
            UiNode::String(raw) => {
                inline.push_leaf(raw, Paint { style, href });
                return;
            }
            UiNode::Row { props, children } => {
                items.extend(Self::text(std::mem::take(inline)));
                if let Some(color) = &props.background {
                    style = style.bg(color.resolve(theme));
                }
                let children = children
                    .iter()
                    .flatten()
                    .filter_map(|node| Self::from_node(node, style, href, theme))
                    .collect();
                items.extend(Self::row(
                    children,
                    props.gap,
                    props.wrap,
                    Paint { style, href },
                ));
                return;
            }
            UiNode::Text { props, children } => {
                style = props.style(style, theme);
                children
            }
            UiNode::Link { props, children } => {
                href = Some(&props.href.0);
                style = style.underlined();
                children
            }
            UiNode::Bold { children } => {
                style = style.bold();
                children
            }
            UiNode::Italic { children } => {
                style = style.italic();
                children
            }
            UiNode::Underline { children } => {
                style = style.underlined();
                children
            }
        };
        for node in children.iter().flatten() {
            Self::collect_inline(items, inline, node, style, href, theme);
        }
    }
}

#[cfg(test)]
#[path = "prepare_tests.rs"]
mod tests;
