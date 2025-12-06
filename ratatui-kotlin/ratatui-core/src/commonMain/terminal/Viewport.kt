use core::fmt;

use crate::layout::Rect;

/// Represents the viewport of the terminal. The viewport is the area of the terminal that is
/// currently visible to the user. It can be either fullscreen, inline or fixed.
///
/// When the viewport is fullscreen, the whole terminal is used to draw the application.
///
/// When the viewport is inline, it is drawn inline with the rest of the terminal. The height of
/// the viewport is fixed, but the width is the same as the terminal width.
///
/// When the viewport is fixed, it is drawn in a fixed area of the terminal. The area is specified
/// by a [`Rect`].
///
/// See [`Terminal::with_options`] for more information.
///
/// [`Terminal::with_options`]: crate::terminal::Terminal::with_options
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub enum Viewport {
    /// The viewport is fullscreen
    #[default]
    Fullscreen,
    /// The viewport is inline with the rest of the terminal.
    ///
    /// The viewport's height is fixed and specified in number of lines. The width is the same as
    /// the terminal's width. The viewport is drawn below the cursor position.
    Inline(u16),
    /// The viewport is drawn in a fixed area of the terminal. The area is specified by a [`Rect`].
    Fixed(Rect),
}

impl fmt::Display for Viewport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Fullscreen => write!(f, "Fullscreen"),
            Self::Inline(height) => write!(f, "Inline({height})"),
            Self::Fixed(area) => write!(f, "Fixed({area})"),
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::ToString;

    use super::*;

    #[test]
    fn viewport_to_string() {
        assert_eq!(Viewport::Fullscreen.to_string(), "Fullscreen");
        assert_eq!(Viewport::Inline(5).to_string(), "Inline(5)");
        assert_eq!(
            Viewport::Fixed(Rect::new(0, 0, 5, 5)).to_string(),
            "Fixed(5x5+0+0)"
        );
    }
}
