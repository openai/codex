use ratatui::layout::Rect;

pub mod adapter_ratatui;
pub mod highlight;
pub mod line_utils;
pub mod model;
pub mod renderable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Insets {
    left: u16,
    top: u16,
    right: u16,
    bottom: u16,
}

impl Insets {
    /// Creates insets using top, left, bottom, right values.
    ///
    /// # Arguments
    /// - `top` (u16): Insets for the top edge.
    /// - `left` (u16): Insets for the left edge.
    /// - `bottom` (u16): Insets for the bottom edge.
    /// - `right` (u16): Insets for the right edge.
    ///
    /// # Returns
    /// - `Insets`: Insets instance with specified edges.
    pub fn tlbr(top: u16, left: u16, bottom: u16, right: u16) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

    /// Creates vertical and horizontal insets.
    ///
    /// # Arguments
    /// - `v` (u16): Insets for top and bottom edges.
    /// - `h` (u16): Insets for left and right edges.
    ///
    /// # Returns
    /// - `Insets`: Insets instance with vertical and horizontal values.
    pub fn vh(v: u16, h: u16) -> Self {
        Self {
            top: v,
            left: h,
            bottom: v,
            right: h,
        }
    }
}

pub trait RectExt {
    /// Returns a rectangle inset by the provided insets.
    ///
    /// # Arguments
    /// - `insets` (Insets): Insets to apply.
    ///
    /// # Returns
    /// - `Rect`: New inset rectangle.
    fn inset(&self, insets: Insets) -> Rect;
}

impl RectExt for Rect {
    /// Returns a rectangle inset by the provided insets.
    ///
    /// # Arguments
    /// - `insets` (Insets): Insets to apply.
    ///
    /// # Returns
    /// - `Rect`: New inset rectangle.
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
