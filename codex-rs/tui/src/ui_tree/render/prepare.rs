//! Prepare inline content and measure container children before viewport layout.

use super::text::InlineText;
use super::text::Paint;
use super::text::PreparedText;
use crate::ui_tree::BoxProps;
use crate::ui_tree::DirectedBox;
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
    pub(super) stretches: bool,
    pub(super) widths: Widths,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct Widths {
    pub(super) minimum: u64,
    pub(super) preferred: u64,
}

#[derive(Debug, PartialEq)]
pub(super) enum NodeKind {
    Text(PreparedText),
    Clamped {
        content: Box<PreparedNode>,
        max_lines: u64,
        marker: Paint,
    },
    Col {
        children: Vec<PreparedNode>,
        gap: u64,
        spacers: usize,
    },
    Row {
        children: Vec<PreparedNode>,
        props: RowProps,
        paint: RowPaint,
    },
    Spacer,
    Empty,
}

/// A block interrupts inline flow even when its contents have zero height.
#[derive(Default)]
struct Flow {
    blocks: Vec<PreparedNode>,
    line: Vec<PreparedNode>,
    paint: Paint,
}

impl Flow {
    fn finish_line(&mut self, inline: &mut InlineText) {
        self.line.extend(PreparedNode::text(std::mem::take(inline)));
        let line = self.take_line();
        self.blocks.extend(line);
    }

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
        let container = matches!(
            node,
            UiNode::Col { .. } | UiNode::Box { .. } | UiNode::Row { .. }
        );
        let mut flow = Flow {
            paint: paint.clone(),
            ..Flow::default()
        };
        let mut inline = InlineText::default();
        Self::collect_inline(&mut flow, &mut inline, node, paint);
        flow.line.extend(Self::text(inline));
        let last = flow.take_line();
        let mut node = if flow.blocks.is_empty() {
            last?
        } else {
            flow.blocks.extend(last);
            if flow.blocks.len() == 1 {
                flow.blocks.pop()?
            } else {
                Self::col(flow.blocks, /*gap*/ 0)
            }
        };
        // An inline wrapper interrupts cross-axis stretch.
        node.stretches &= container;
        Some(node)
    }

    // Emergency word breaks allow each chunk to fit independently. Flex rows
    // retain the space required by their children and gaps.
    fn breakable_minimum(&self) -> u64 {
        match &self.kind {
            NodeKind::Text(text) => text
                .lines
                .iter()
                .flatten()
                .map(super::text::Chunk::minimum_cells)
                .max()
                .unwrap_or_default() as u64,
            NodeKind::Col { children, .. } => children
                .iter()
                .map(Self::breakable_minimum)
                .max()
                .unwrap_or_default(),
            NodeKind::Clamped { .. }
            | NodeKind::Row { .. }
            | NodeKind::Spacer
            | NodeKind::Empty => self.widths.minimum,
        }
    }

    fn col(children: Vec<Self>, gap: u64) -> Self {
        let mut widths = Widths::default();
        for child in &children {
            widths.minimum = widths.minimum.max(child.widths.minimum);
            widths.preferred = widths.preferred.max(child.widths.preferred);
        }
        let spacers = children
            .iter()
            .filter(|c| matches!(c.kind, NodeKind::Spacer))
            .count();
        Self {
            stretches: spacers > 0,
            widths,
            kind: NodeKind::Col {
                children,
                gap,
                spacers,
            },
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
            stretches: false,
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
            stretches: children.iter().any(|c| {
                c.stretches || (props.wrap == RowWrap::Wrap && matches!(c.kind, NodeKind::Spacer))
            }),
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
                    items.push(Self::flex_item(child, paint));
                }
            }
        }
        items.extend(Self::text(strings));
        items
    }

    fn flex_item(child: UiNode, paint: &Paint) -> Self {
        if matches!(child, UiNode::Spacer { .. }) {
            return Self {
                stretches: false,
                kind: NodeKind::Spacer,
                widths: Widths::default(),
            };
        }
        Self::from_node(child, paint.clone()).unwrap_or_else(|| Self {
            stretches: false,
            kind: NodeKind::Empty,
            widths: Widths::default(),
        })
    }

    fn collect_inline(flow: &mut Flow, inline: &mut InlineText, node: UiNode, mut paint: Paint) {
        let parent_preserves_spaces = paint.preserve_whitespace;
        let children = match node {
            UiNode::String(raw) => {
                inline.push_leaf(raw, paint);
                return;
            }
            UiNode::Spacer { .. } => {
                // Outside a flex container the empty div interrupts inline flow.
                flow.finish_line(inline);
                flow.blocks.push(Self {
                    stretches: false,
                    kind: NodeKind::Empty,
                    widths: Widths::default(),
                });
                return;
            }
            UiNode::Badge {
                props,
                children: [label],
            } => {
                inline.push_badge(label, props.color, paint);
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
            UiNode::Box { props, children } => {
                let node = match props {
                    BoxProps::DefaultColumn(props)
                    | BoxProps::Directed(DirectedBox::Col(props)) => {
                        UiNode::Col { props, children }
                    }
                    BoxProps::Directed(DirectedBox::Row(mut props)) => {
                        // Boxes always use flex items, including anonymous runs of strings.
                        props.justify.get_or_insert_default();
                        UiNode::Row { props, children }
                    }
                };
                flow.finish_line(inline);
                flow.blocks.push(Self::flex_item(node, &paint));
                return;
            }
            UiNode::Col { props, children } => {
                flow.finish_line(inline);
                let children = Self::flex_items(children, &paint);
                flow.blocks.push(Self::col(children, props.gap));
                return;
            }
            UiNode::Row { props, children } => {
                flow.line.extend(Self::text(std::mem::take(inline)));
                if let Some(color) = props.background {
                    paint.background = Some(color);
                }
                let children = if props.justify.is_some()
                    || children
                        .iter()
                        .flatten()
                        .any(|n| matches!(n, UiNode::Spacer { .. }))
                {
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
                if let Some(max_lines) = props.max_lines {
                    paint.break_long_words = true;
                    flow.finish_line(inline);
                    let node = UiNode::Text {
                        props: Default::default(),
                        children,
                    };
                    if let Some(content) = Self::from_node(node, paint.clone()) {
                        paint.href = None;
                        flow.blocks.push(Self {
                            widths: Widths {
                                minimum: content.breakable_minimum(),
                                preferred: content.widths.preferred,
                            },
                            stretches: false,
                            kind: NodeKind::Clamped {
                                content: Box::new(content),
                                max_lines,
                                marker: paint,
                            },
                        });
                    }
                    return;
                }
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
