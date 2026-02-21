use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt as _;

pub trait Renderable {
    fn render(&self, area: Rect, buf: &mut Buffer);
    fn desired_height(&self, width: u16) -> u16;
    fn cursor_pos(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}

pub enum RenderableItem<'a> {
    Owned(Box<dyn Renderable + 'a>),
    Borrowed(&'a dyn Renderable),
}

impl<'a> Renderable for RenderableItem<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        match self {
            RenderableItem::Owned(child) => child.render(area, buf),
            RenderableItem::Borrowed(child) => child.render(area, buf),
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        match self {
            RenderableItem::Owned(child) => child.desired_height(width),
            RenderableItem::Borrowed(child) => child.desired_height(width),
        }
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        match self {
            RenderableItem::Owned(child) => child.cursor_pos(area),
            RenderableItem::Borrowed(child) => child.cursor_pos(area),
        }
    }
}

impl<'a> From<Box<dyn Renderable + 'a>> for RenderableItem<'a> {
    fn from(value: Box<dyn Renderable + 'a>) -> Self {
        RenderableItem::Owned(value)
    }
}

impl<'a, R> From<R> for Box<dyn Renderable + 'a>
where
    R: Renderable + 'a,
{
    fn from(value: R) -> Self {
        Box::new(value)
    }
}

impl Renderable for () {
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
    fn desired_height(&self, _width: u16) -> u16 {
        0
    }
}

impl Renderable for &str {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

impl Renderable for String {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

impl<'a> Renderable for Span<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

impl<'a> Renderable for Line<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        WidgetRef::render_ref(self, area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

impl<'a> Renderable for Paragraph<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.line_count(width) as u16
    }
}

impl<R: Renderable> Renderable for Option<R> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        if let Some(renderable) = self {
            renderable.render(area, buf);
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        if let Some(renderable) = self {
            renderable.desired_height(width)
        } else {
            0
        }
    }
}

impl<R: Renderable> Renderable for Arc<R> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_ref().render(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.as_ref().desired_height(width)
    }
}

pub struct ColumnRenderable<'a> {
    children: Vec<RenderableItem<'a>>,
}

impl Renderable for ColumnRenderable<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut y = area.y;
        for child in &self.children {
            let child_area = Rect::new(area.x, y, area.width, child.desired_height(area.width))
                .intersection(area);
            if !child_area.is_empty() {
                child.render(child_area, buf);
            }
            y += child_area.height;
        }
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.children
            .iter()
            .map(|child| child.desired_height(width))
            .sum()
    }

    /// Returns the cursor position of the first child that has a cursor position, offset by the
    /// child's position in the column.
    ///
    /// It is generally assumed that either zero or one child will have a cursor position.
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let mut y = area.y;
        for child in &self.children {
            let child_area = Rect::new(area.x, y, area.width, child.desired_height(area.width))
                .intersection(area);
            if !child_area.is_empty()
                && let Some((px, py)) = child.cursor_pos(child_area)
            {
                return Some((px, py));
            }
            y += child_area.height;
        }
        None
    }
}

impl<'a> ColumnRenderable<'a> {
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    pub fn with<I, T>(children: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RenderableItem<'a>>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    pub fn push(&mut self, child: impl Into<Box<dyn Renderable + 'a>>) {
        self.children.push(RenderableItem::Owned(child.into()));
    }

    #[allow(dead_code)]
    pub fn push_ref<R>(&mut self, child: &'a R)
    where
        R: Renderable + 'a,
    {
        self.children
            .push(RenderableItem::Borrowed(child as &'a dyn Renderable));
    }
}

pub struct FlexChild<'a> {
    flex: i32,
    child: RenderableItem<'a>,
}

pub struct FlexRenderable<'a> {
    children: Vec<FlexChild<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FlexAllocationMode {
    /// Allocate flex children to their intrinsic height, capped by available extent.
    ///
    /// This is used by `desired_height()` so we keep content-based sizing semantics.
    FitContent,
    /// Allocate flex children to fill their assigned extent.
    ///
    /// This is used during render/cursor layout to keep fixed children anchored.
    FillAssignedExtent,
}

/// Lays out children in a column, with the ability to specify a flex factor for each child.
///
/// Children with flex factor > 0 will be allocated the remaining space after the non-flex children,
/// proportional to the flex factor.
impl<'a> FlexRenderable<'a> {
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    pub fn push(&mut self, flex: i32, child: impl Into<RenderableItem<'a>>) {
        self.children.push(FlexChild {
            flex,
            child: child.into(),
        });
    }

    /// Loosely inspired by Flutter's Flex widget.
    ///
    /// Ref https://github.com/flutter/flutter/blob/3fd81edbf1e015221e143c92b2664f4371bdc04a/packages/flutter/lib/src/rendering/flex.dart#L1205-L1209
    fn allocate(&self, area: Rect, mode: FlexAllocationMode) -> Vec<Rect> {
        let mut allocated_rects = Vec::with_capacity(self.children.len());
        let mut child_sizes = vec![0; self.children.len()];
        let mut allocated_size = 0;
        let mut total_flex = 0;

        // 1. Allocate space to non-flex children.
        let max_size = area.height;
        let mut last_flex_child_idx = 0;
        for (i, FlexChild { flex, child }) in self.children.iter().enumerate() {
            if *flex > 0 {
                total_flex += flex;
                last_flex_child_idx = i;
            } else {
                child_sizes[i] = child
                    .desired_height(area.width)
                    .min(max_size.saturating_sub(allocated_size));
                allocated_size += child_sizes[i];
            }
        }
        let free_space = max_size.saturating_sub(allocated_size);
        // 2. Allocate space to flex children, proportional to their flex factor.
        let mut allocated_flex_space = 0;
        if total_flex > 0 {
            let space_per_flex = free_space / total_flex as u16;
            for (i, FlexChild { flex, child }) in self.children.iter().enumerate() {
                if *flex > 0 {
                    // Last flex child gets all the remaining space, to prevent a rounding error
                    // from not allocating all the space.
                    let max_child_extent = if i == last_flex_child_idx {
                        free_space - allocated_flex_space
                    } else {
                        space_per_flex * *flex as u16
                    };
                    let child_size = match mode {
                        FlexAllocationMode::FitContent => {
                            child.desired_height(area.width).min(max_child_extent)
                        }
                        FlexAllocationMode::FillAssignedExtent => max_child_extent,
                    };
                    child_sizes[i] = child_size;
                    allocated_flex_space += child_size;
                }
            }
        }

        let mut y = area.y;
        for size in child_sizes {
            let child_area = Rect::new(area.x, y, area.width, size);
            allocated_rects.push(child_area);
            y += child_area.height;
        }
        allocated_rects
    }
}

impl<'a> Renderable for FlexRenderable<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.allocate(area, FlexAllocationMode::FillAssignedExtent)
            .into_iter()
            .zip(self.children.iter())
            .for_each(|(rect, child)| {
                child.child.render(rect, buf);
            });
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.allocate(
            Rect::new(0, 0, width, u16::MAX),
            FlexAllocationMode::FitContent,
        )
        .last()
        .map(|rect| rect.bottom())
        .unwrap_or(0)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.allocate(area, FlexAllocationMode::FillAssignedExtent)
            .into_iter()
            .zip(self.children.iter())
            .find_map(|(rect, child)| child.child.cursor_pos(rect))
    }
}

pub struct RowRenderable<'a> {
    children: Vec<(u16, RenderableItem<'a>)>,
}

impl Renderable for RowRenderable<'_> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        let mut x = area.x;
        for (width, child) in &self.children {
            let available_width = area.width.saturating_sub(x - area.x);
            let child_area = Rect::new(x, area.y, (*width).min(available_width), area.height);
            if child_area.is_empty() {
                break;
            }
            child.render(child_area, buf);
            x = x.saturating_add(*width);
        }
    }
    fn desired_height(&self, width: u16) -> u16 {
        let mut max_height = 0;
        let mut width_remaining = width;
        for (child_width, child) in &self.children {
            let w = (*child_width).min(width_remaining);
            if w == 0 {
                break;
            }
            let height = child.desired_height(w);
            if height > max_height {
                max_height = height;
            }
            width_remaining = width_remaining.saturating_sub(w);
        }
        max_height
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        let mut x = area.x;
        for (width, child) in &self.children {
            let available_width = area.width.saturating_sub(x - area.x);
            let child_area = Rect::new(x, area.y, (*width).min(available_width), area.height);
            if !child_area.is_empty()
                && let Some(pos) = child.cursor_pos(child_area)
            {
                return Some(pos);
            }
            x = x.saturating_add(*width);
        }
        None
    }
}

impl<'a> RowRenderable<'a> {
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    pub fn push(&mut self, width: u16, child: impl Into<Box<dyn Renderable>>) {
        self.children
            .push((width, RenderableItem::Owned(child.into())));
    }

    #[allow(dead_code)]
    pub fn push_ref<R>(&mut self, width: u16, child: &'a R)
    where
        R: Renderable + 'a,
    {
        self.children
            .push((width, RenderableItem::Borrowed(child as &'a dyn Renderable)));
    }
}

pub struct InsetRenderable<'a> {
    child: RenderableItem<'a>,
    insets: Insets,
}

impl<'a> Renderable for InsetRenderable<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.child.render(area.inset(self.insets), buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.child
            .desired_height(width - self.insets.left - self.insets.right)
            + self.insets.top
            + self.insets.bottom
    }
    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.child.cursor_pos(area.inset(self.insets))
    }
}

impl<'a> InsetRenderable<'a> {
    pub fn new(child: impl Into<RenderableItem<'a>>, insets: Insets) -> Self {
        Self {
            child: child.into(),
            insets,
        }
    }
}

pub trait RenderableExt<'a> {
    fn inset(self, insets: Insets) -> RenderableItem<'a>;
}

impl<'a, R> RenderableExt<'a> for R
where
    R: Renderable + 'a,
{
    fn inset(self, insets: Insets) -> RenderableItem<'a> {
        let child: RenderableItem<'a> =
            RenderableItem::Owned(Box::new(self) as Box<dyn Renderable + 'a>);
        RenderableItem::Owned(Box::new(InsetRenderable { child, insets }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct SizedCursorRenderable {
        height: u16,
        has_cursor: bool,
    }

    impl Renderable for SizedCursorRenderable {
        fn render(&self, _area: Rect, _buf: &mut Buffer) {}

        fn desired_height(&self, _width: u16) -> u16 {
            self.height
        }

        fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
            self.has_cursor.then_some((area.x, area.y))
        }
    }

    #[test]
    fn flex_render_anchors_fixed_child_to_bottom() {
        let mut flex = FlexRenderable::new();
        flex.push(
            1,
            RenderableItem::Owned(Box::new(SizedCursorRenderable {
                height: 1,
                has_cursor: false,
            })),
        );
        flex.push(
            0,
            RenderableItem::Owned(Box::new(SizedCursorRenderable {
                height: 1,
                has_cursor: true,
            })),
        );

        let area = Rect::new(0, 0, 80, 10);
        assert_eq!(flex.cursor_pos(area), Some((0, 9)));
    }

    #[test]
    fn flex_desired_height_remains_content_based() {
        let mut flex = FlexRenderable::new();
        flex.push(
            1,
            RenderableItem::Owned(Box::new(SizedCursorRenderable {
                height: 3,
                has_cursor: false,
            })),
        );
        flex.push(
            0,
            RenderableItem::Owned(Box::new(SizedCursorRenderable {
                height: 2,
                has_cursor: false,
            })),
        );

        assert_eq!(flex.desired_height(80), 5);
    }
}
