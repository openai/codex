//! Lightweight layout primitives for composing renderable widgets.
//!
//! The types in this module provide a small abstraction over `ratatui` widgets
//! so view code can combine renderable items, compute desired heights, and
//! surface cursor positions in a consistent way. These helpers do not own any
//! global state; they only wrap child renderables and perform rectangle math.

use std::sync::Arc;

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::text::Span;
use ratatui::widgets::Paragraph;
use ratatui::widgets::WidgetRef;

use crate::render::Insets;
use crate::render::RectExt as _;

/// Rendering abstraction that pairs drawing with sizing and cursor placement.
pub trait Renderable {
    /// Draws the item into the provided buffer region.
    fn render(&self, area: Rect, buf: &mut Buffer);
    /// Returns the preferred height for the item at the given width.
    fn desired_height(&self, width: u16) -> u16;
    /// Returns the cursor position, if the item owns one.
    fn cursor_pos(&self, _area: Rect) -> Option<(u16, u16)> {
        None
    }
}

/// Owned or borrowed renderable value used by layout containers.
pub enum RenderableItem<'a> {
    /// Renderable stored by ownership.
    Owned(Box<dyn Renderable + 'a>),
    /// Renderable stored by shared reference.
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

/// Wraps an owned renderable in a `RenderableItem`.
impl<'a> From<Box<dyn Renderable + 'a>> for RenderableItem<'a> {
    fn from(value: Box<dyn Renderable + 'a>) -> Self {
        RenderableItem::Owned(value)
    }
}

/// Converts any renderable into a boxed trait object.
impl<'a, R> From<R> for Box<dyn Renderable + 'a>
where
    R: Renderable + 'a,
{
    fn from(value: R) -> Self {
        Box::new(value)
    }
}

/// Treats the unit type as an empty renderable.
impl Renderable for () {
    fn render(&self, _area: Rect, _buf: &mut Buffer) {}
    fn desired_height(&self, _width: u16) -> u16 {
        0
    }
}

/// Treats a string slice as a single-line renderable.
impl Renderable for &str {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

/// Treats an owned string as a single-line renderable.
impl Renderable for String {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

/// Treats a span as a single-line renderable.
impl<'a> Renderable for Span<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

/// Treats a line as a single-line renderable.
impl<'a> Renderable for Line<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        WidgetRef::render_ref(self, area, buf);
    }
    fn desired_height(&self, _width: u16) -> u16 {
        1
    }
}

/// Treats a paragraph as a renderable with wrapped line height.
impl<'a> Renderable for Paragraph<'a> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.render_ref(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.line_count(width) as u16
    }
}

/// Allows optional renderables to take zero height and skip rendering.
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

/// Delegates rendering and sizing through an `Arc`.
impl<R: Renderable> Renderable for Arc<R> {
    fn render(&self, area: Rect, buf: &mut Buffer) {
        self.as_ref().render(area, buf);
    }
    fn desired_height(&self, width: u16) -> u16 {
        self.as_ref().desired_height(width)
    }
}

/// Column layout that stacks renderables vertically.
pub struct ColumnRenderable<'a> {
    /// Ordered list of child renderables.
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
    /// Creates an empty column.
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    /// Creates a column from an iterator of renderable items.
    pub fn with<I, T>(children: I) -> Self
    where
        I: IntoIterator<Item = T>,
        T: Into<RenderableItem<'a>>,
    {
        Self {
            children: children.into_iter().map(Into::into).collect(),
        }
    }

    /// Pushes an owned renderable onto the end of the column.
    pub fn push(&mut self, child: impl Into<Box<dyn Renderable + 'a>>) {
        self.children.push(RenderableItem::Owned(child.into()));
    }

    #[allow(dead_code)]
    /// Pushes a borrowed renderable onto the end of the column.
    pub fn push_ref<R>(&mut self, child: &'a R)
    where
        R: Renderable + 'a,
    {
        self.children
            .push(RenderableItem::Borrowed(child as &'a dyn Renderable));
    }
}

/// A flex child entry that carries its flex factor alongside the renderable.
pub struct FlexChild<'a> {
    /// Flex factor used when distributing remaining space.
    flex: i32,
    /// Renderable for this child.
    child: RenderableItem<'a>,
}

/// Column layout that distributes remaining space to flexible children.
pub struct FlexRenderable<'a> {
    /// Children ordered from top to bottom.
    children: Vec<FlexChild<'a>>,
}

/// Lays out children in a column, with the ability to specify a flex factor for each child.
///
/// Children with flex factor > 0 will be allocated the remaining space after the non-flex children,
/// proportional to the flex factor.
impl<'a> FlexRenderable<'a> {
    /// Creates an empty flex column.
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    /// Pushes a child with the given flex factor.
    pub fn push(&mut self, flex: i32, child: impl Into<RenderableItem<'a>>) {
        self.children.push(FlexChild {
            flex,
            child: child.into(),
        });
    }

    /// Loosely inspired by Flutter's Flex widget.
    ///
    /// See <https://github.com/flutter/flutter/blob/3fd81edbf1e015221e143c92b2664f4371bdc04a/packages/flutter/lib/src/rendering/flex.dart#L1205-L1209>.
    fn allocate(&self, area: Rect) -> Vec<Rect> {
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
                    // Last flex child receives any remainder to avoid rounding loss.
                    let max_child_extent = if i == last_flex_child_idx {
                        free_space - allocated_flex_space
                    } else {
                        space_per_flex * *flex as u16
                    };
                    let child_size = child.desired_height(area.width).min(max_child_extent);
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
        self.allocate(area)
            .into_iter()
            .zip(self.children.iter())
            .for_each(|(rect, child)| {
                child.child.render(rect, buf);
            });
    }

    fn desired_height(&self, width: u16) -> u16 {
        self.allocate(Rect::new(0, 0, width, u16::MAX))
            .last()
            .map(|rect| rect.bottom())
            .unwrap_or(0)
    }

    fn cursor_pos(&self, area: Rect) -> Option<(u16, u16)> {
        self.allocate(area)
            .into_iter()
            .zip(self.children.iter())
            .find_map(|(rect, child)| child.child.cursor_pos(rect))
    }
}

/// Row layout that assigns fixed widths to each child.
pub struct RowRenderable<'a> {
    /// Children and their requested widths.
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
    /// Creates an empty row.
    pub fn new() -> Self {
        Self { children: vec![] }
    }

    /// Pushes an owned child with a fixed width.
    pub fn push(&mut self, width: u16, child: impl Into<Box<dyn Renderable>>) {
        self.children
            .push((width, RenderableItem::Owned(child.into())));
    }

    #[allow(dead_code)]
    /// Pushes a borrowed child with a fixed width.
    pub fn push_ref<R>(&mut self, width: u16, child: &'a R)
    where
        R: Renderable + 'a,
    {
        self.children
            .push((width, RenderableItem::Borrowed(child as &'a dyn Renderable)));
    }
}

/// Renderable that applies padding before delegating to a child.
pub struct InsetRenderable<'a> {
    /// Renderable child to inset.
    child: RenderableItem<'a>,
    /// Insets to apply around the child.
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
    /// Wraps a renderable in an inset container.
    pub fn new(child: impl Into<RenderableItem<'a>>, insets: Insets) -> Self {
        Self {
            child: child.into(),
            insets,
        }
    }
}

/// Extension helpers for wrapping renderables with layout adapters.
pub trait RenderableExt<'a> {
    /// Wraps the renderable in an inset container.
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
