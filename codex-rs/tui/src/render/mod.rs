use ratatui::layout::Rect;

pub mod highlight;
pub mod line_utils;
pub mod renderable;

pub struct Insets {
    pub left: u16,
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
}

impl Insets {
    pub fn tlbr(top: u16, left: u16, bottom: u16, right: u16) -> Self {
        Self {
            top,
            left,
            bottom,
            right,
        }
    }

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
    fn inset(&self, insets: Insets) -> Rect;
}

impl RectExt for Rect {
    fn inset(&self, insets: Insets) -> Rect {
        Rect {
            x: self.x + insets.left,
            y: self.y + insets.top,
            width: self.width - insets.left - insets.right,
            height: self.height - insets.top - insets.bottom,
        }
    }
}
