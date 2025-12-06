//! The [`RatatuiLogo`] widget renders the Ratatui logo.
use indoc::indoc;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::text::Text;
use ratatui_core::widgets::Widget;

/// A widget that renders the Ratatui logo
///
/// The Ratatui logo takes up two lines of text and comes in two sizes: `Tiny` and `Small`. This may
/// be used in an application's help or about screen to show that it is powered by Ratatui.
///
/// # Examples
///
/// The [Ratatui-logo] example demonstrates how to use the `RatatuiLogo` widget. This can be run by
/// cloning the Ratatui repository and then running the following command with an optional size
/// argument:
///
/// ```shell
/// cargo run --example logo [size]
/// ```
///
/// [Ratatui-logo]: https://github.com/ratatui/ratatui/blob/main/ratatui-widgets/examples/logo.rs
///
/// ## Tiny (default, 2x15 characters)
///
/// ```
/// use ratatui::widgets::RatatuiLogo;
///
/// # fn draw(frame: &mut ratatui::Frame) {
/// frame.render_widget(RatatuiLogo::tiny(), frame.area());
/// # }
/// ```
///
/// Renders:
///
/// ```text
/// ▛▚▗▀▖▜▘▞▚▝▛▐ ▌▌
/// ▛▚▐▀▌▐ ▛▜ ▌▝▄▘▌
/// ```
///
/// ## Small (2x27 characters)
///
/// ```
/// use ratatui::widgets::RatatuiLogo;
///
/// # fn draw(frame: &mut ratatui::Frame) {
/// frame.render_widget(RatatuiLogo::small(), frame.area());
/// # }
/// ```
///
/// Renders:
///
/// ```text
/// █▀▀▄ ▄▀▀▄▝▜▛▘▄▀▀▄▝▜▛▘█  █ █
/// █▀▀▄ █▀▀█ ▐▌ █▀▀█ ▐▌ ▀▄▄▀ █
/// ```
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RatatuiLogo {
    size: Size,
}

/// The size of the logo
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Size {
    /// A tiny logo
    ///
    /// The default size of the logo (2x15 characters)
    ///
    /// ```text
    /// ▛▚▗▀▖▜▘▞▚▝▛▐ ▌▌
    /// ▛▚▐▀▌▐ ▛▜ ▌▝▄▘▌
    /// ```
    #[default]
    Tiny,
    /// A small logo
    ///
    /// A slightly larger version of the logo (2x27 characters)
    ///
    /// ```text
    /// █▀▀▄ ▄▀▀▄▝▜▛▘▄▀▀▄▝▜▛▘█  █ █
    /// █▀▀▄ █▀▀█ ▐▌ █▀▀█ ▐▌ ▀▄▄▀ █
    /// ```
    Small,
}

impl RatatuiLogo {
    /// Create a new Ratatui logo widget
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::widgets::{RatatuiLogo, RatatuiLogoSize};
    ///
    /// let logo = RatatuiLogo::new(RatatuiLogoSize::Tiny);
    /// ```
    pub const fn new(size: Size) -> Self {
        Self { size }
    }

    /// Set the size of the logo
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::widgets::{RatatuiLogo, RatatuiLogoSize};
    ///
    /// let logo = RatatuiLogo::default().size(RatatuiLogoSize::Small);
    /// ```
    #[must_use]
    pub const fn size(self, size: Size) -> Self {
        let _ = self;
        Self { size }
    }

    /// Create a new Ratatui logo widget with a tiny size
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::widgets::RatatuiLogo;
    ///
    /// let logo = RatatuiLogo::tiny();
    /// ```
    pub const fn tiny() -> Self {
        Self::new(Size::Tiny)
    }

    /// Create a new Ratatui logo widget with a small size
    ///
    /// # Examples
    ///
    /// ```
    /// use ratatui::widgets::RatatuiLogo;
    ///
    /// let logo = RatatuiLogo::small();
    /// ```
    pub const fn small() -> Self {
        Self::new(Size::Small)
    }
}

impl Widget for RatatuiLogo {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let logo = self.size.as_str();
        Text::raw(logo).render(area, buf);
    }
}

impl Size {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Tiny => Self::tiny(),
            Self::Small => Self::small(),
        }
    }

    const fn tiny() -> &'static str {
        indoc! {"
            ▛▚▗▀▖▜▘▞▚▝▛▐ ▌▌
            ▛▚▐▀▌▐ ▛▜ ▌▝▄▘▌
        "}
    }

    const fn small() -> &'static str {
        indoc! {"
            █▀▀▄ ▄▀▀▄▝▜▛▘▄▀▀▄▝▜▛▘█  █ █
            █▀▀▄ █▀▀█ ▐▌ █▀▀█ ▐▌ ▀▄▄▀ █
        "}
    }
}

#[cfg(test)]
mod tests {
    use rstest::rstest;

    use super::*;

    #[rstest]
    #[case::tiny(Size::Tiny)]
    #[case::small(Size::Small)]
    fn new_size(#[case] size: Size) {
        let logo = RatatuiLogo::new(size);
        assert_eq!(logo.size, size);
    }

    #[test]
    fn default_logo_is_tiny() {
        let logo = RatatuiLogo::default();
        assert_eq!(logo.size, Size::Tiny);
    }

    #[test]
    fn set_logo_size_to_small() {
        let logo = RatatuiLogo::default().size(Size::Small);
        assert_eq!(logo.size, Size::Small);
    }

    #[test]
    fn tiny_logo_constant() {
        let logo = RatatuiLogo::tiny();
        assert_eq!(logo.size, Size::Tiny);
    }

    #[test]
    fn small_logo_constant() {
        let logo = RatatuiLogo::small();
        assert_eq!(logo.size, Size::Small);
    }

    #[test]
    #[rustfmt::skip]
    fn render_tiny() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 15, 2));
        RatatuiLogo::tiny().render(buf.area, &mut buf);
        assert_eq!(
            buf,
            Buffer::with_lines([
                "▛▚▗▀▖▜▘▞▚▝▛▐ ▌▌",
                "▛▚▐▀▌▐ ▛▜ ▌▝▄▘▌",
            ])
        );
    }

    #[test]
    #[rustfmt::skip]
    fn render_small() {
        let mut buf = Buffer::empty(Rect::new(0, 0, 27, 2));
        RatatuiLogo::small().render(buf.area, &mut buf);
        assert_eq!(
            buf,
            Buffer::with_lines([
                "█▀▀▄ ▄▀▀▄▝▜▛▘▄▀▀▄▝▜▛▘█  █ █",
                "█▀▀▄ █▀▀█ ▐▌ █▀▀█ ▐▌ ▀▄▄▀ █",
            ])
        );
    }

    #[rstest]
    #[case::tiny(Size::Tiny, Buffer::with_lines(["▛"]))]
    #[case::small(Size::Small, Buffer::with_lines(["█"]))]
    fn render_in_minimal_buffer(#[case] size: Size, #[case] expected: Buffer) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let logo = RatatuiLogo::new(size);
        // This should not panic, even if the buffer is too small to render the logo.
        logo.render(buffer.area, &mut buffer);
        assert_eq!(buffer, expected);
    }

    #[rstest]
    #[case::tiny(Size::Tiny)]
    #[case::small(Size::Small)]
    fn render_in_zero_size_buffer(#[case] size: Size) {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let logo = RatatuiLogo::new(size);
        // This should not panic, even if the buffer has zero size.
        logo.render(buffer.area, &mut buffer);
    }
}
