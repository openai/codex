use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Style, Styled};
use ratatui_core::text::Text;
use ratatui_core::widgets::Widget;

/// A [`Cell`] contains the [`Text`] to be displayed in a [`Row`] of a [`Table`].
///
/// You can apply a [`Style`] to the [`Cell`] using [`Cell::style`]. This will set the style for the
/// entire area of the cell. Any [`Style`] set on the [`Text`] content will be combined with the
/// [`Style`] of the [`Cell`] by adding the [`Style`] of the [`Text`] content to the [`Style`] of
/// the [`Cell`]. Styles set on the text content will only affect the content.
///
/// You can use [`Text::alignment`] when creating a cell to align its content.
///
/// # Examples
///
/// You can create a `Cell` from anything that can be converted to a [`Text`].
///
/// ```rust
/// use std::borrow::Cow;
///
/// use ratatui::style::Stylize;
/// use ratatui::text::{Line, Span, Text};
/// use ratatui::widgets::Cell;
///
/// Cell::from("simple string");
/// Cell::from(Span::from("span"));
/// Cell::from(Line::from(vec![
///     Span::from("a vec of "),
///     Span::from("spans").bold(),
/// ]));
/// Cell::from(Text::from("a text"));
/// Cell::from(Text::from(Cow::Borrowed("hello")));
/// ```
///
/// `Cell` implements [`Styled`] which means you can use style shorthands from the [`Stylize`] trait
/// to set the style of the cell concisely.
///
/// ```rust
/// use ratatui::style::Stylize;
/// use ratatui::widgets::Cell;
///
/// Cell::new("Cell 1").red().italic();
/// ```
///
/// [`Row`]: super::Row
/// [`Table`]: super::Table
/// [`Stylize`]: ratatui_core::style::Stylize
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Cell<'a> {
    content: Text<'a>,
    style: Style,
}

impl<'a> Cell<'a> {
    /// Creates a new [`Cell`]
    ///
    /// The `content` parameter accepts any value that can be converted into a [`Text`].
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::text::{Line, Span, Text};
    /// use ratatui::widgets::Cell;
    ///
    /// Cell::new("simple string");
    /// Cell::new(Span::from("span"));
    /// Cell::new(Line::from(vec![
    ///     Span::raw("a vec of "),
    ///     Span::from("spans").bold(),
    /// ]));
    /// Cell::new(Text::from("a text"));
    /// ```
    pub fn new<T>(content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }

    /// Set the content of the [`Cell`]
    ///
    /// The `content` parameter accepts any value that can be converted into a [`Text`].
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::text::{Line, Span, Text};
    /// use ratatui::widgets::Cell;
    ///
    /// Cell::default().content("simple string");
    /// Cell::default().content(Span::from("span"));
    /// Cell::default().content(Line::from(vec![
    ///     Span::raw("a vec of "),
    ///     Span::from("spans").bold(),
    /// ]));
    /// Cell::default().content(Text::from("a text"));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn content<T>(mut self, content: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        self.content = content.into();
        self
    }

    /// Set the `Style` of this cell
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This `Style` will override the `Style` of the [`Row`] and can be overridden by the `Style`
    /// of the [`Text`] content.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::widgets::Cell;
    ///
    /// Cell::new("Cell 1").style(Style::new().red().italic());
    /// ```
    ///
    /// `Cell` also implements the [`Styled`] trait, which means you can use style shorthands from
    /// the [`Stylize`] trait to set the style of the widget more concisely.
    ///
    /// ```rust
    /// use ratatui::style::Stylize;
    /// use ratatui::widgets::Cell;
    ///
    /// Cell::new("Cell 1").red().italic();
    /// ```
    ///
    /// [`Row`]: super::Row
    /// [`Color`]: ratatui_core::style::Color
    /// [`Stylize`]: ratatui_core::style::Stylize
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl Cell<'_> {
    pub(crate) fn render(&self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        Widget::render(&self.content, area, buf);
    }
}

impl<'a, T> From<T> for Cell<'a>
where
    T: Into<Text<'a>>,
{
    fn from(content: T) -> Self {
        Self {
            content: content.into(),
            style: Style::default(),
        }
    }
}

impl Styled for Cell<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::{Color, Modifier, Stylize};

    use super::*;

    #[test]
    fn new() {
        let cell = Cell::new("");
        assert_eq!(cell.content, Text::from(""));
    }

    #[test]
    fn content() {
        let cell = Cell::default().content("");
        assert_eq!(cell.content, Text::from(""));
    }

    #[test]
    fn style() {
        let style = Style::default().red().italic();
        let cell = Cell::default().style(style);
        assert_eq!(cell.style, style);
    }

    #[test]
    fn stylize() {
        assert_eq!(
            Cell::from("").black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }
}
