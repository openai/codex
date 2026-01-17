//! Rendering helpers shared across TUI widgets.
//!
//! This module groups small utilities used by rendering code, such as inset
//! calculations and line helpers. It deliberately avoids owning any state; the
//! helpers operate on `ratatui` primitives and are used by multiple widgets.

use ratatui::layout::Rect;

pub mod highlight;
pub mod line_utils;
pub mod renderable;

/// Insets to apply around a rectangular region.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insets {
    /// Left edge to remove from the rectangle.
    left: u16,
    /// Top edge to remove from the rectangle.
    top: u16,
    /// Right edge to remove from the rectangle.
    right: u16,
    /// Bottom edge to remove from the rectangle.
    bottom: u16,
}

impl Insets {
    /// Builds insets from top/left/bottom/right values.
    pub fn tlbr(top: u16, left: u16, bottom: u16, right: u16) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    /// Builds symmetric vertical/horizontal insets.
    pub fn vh(v: u16, h: u16) -> Self {
        Self {
            top: v,
            left: h,
            bottom: v,
            right: h,
        }
    }
}

/// Extension helpers for `ratatui` rectangles.
pub trait RectExt {
    /// Returns a new rectangle shrunk by the provided insets.
    fn inset(&self, insets: Insets) -> Rect;
}

impl RectExt for Rect {
    /// Shrinks a rectangle by applying inset padding on each side.
    fn inset(&self, insets: Insets) -> Rect {
        let horizontal = insets.left.saturating_add(insets.right);
        let vertical = insets.top.saturating_add(insets.bottom);
        Rect {
            x: self.x.saturating_add(insets.left),
            y: self.y.saturating_add(insets.top),
            width: self.width.saturating_sub(horizontal),
            height: self.height.saturating_sub(vertical),
        }
    }
}
