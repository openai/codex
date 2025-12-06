use crate::style::{Style, Styled};

const NBSP: &str = "\u{00a0}";
const ZWSP: &str = "\u{200b}";

/// A grapheme associated to a style.
/// Note that, although `StyledGrapheme` is the smallest divisible unit of text,
/// it actually is not a member of the text type hierarchy (`Text` -> `Line` -> `Span`).
/// It is a separate type used mostly for rendering purposes. A `Span` consists of components that
/// can be split into `StyledGrapheme`s, but it does not contain a collection of `StyledGrapheme`s.
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct StyledGrapheme<'a> {
    pub symbol: &'a str,
    pub style: Style,
}

impl<'a> StyledGrapheme<'a> {
    /// Creates a new `StyledGrapheme` with the given symbol and style.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: crate::style::Color
    pub fn new<S: Into<Style>>(symbol: &'a str, style: S) -> Self {
        Self {
            symbol,
            style: style.into(),
        }
    }

    pub fn is_whitespace(&self) -> bool {
        let symbol = self.symbol;
        symbol == ZWSP || symbol.chars().all(char::is_whitespace) && symbol != NBSP
    }
}

impl Styled for StyledGrapheme<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(mut self, style: S) -> Self::Item {
        self.style = style.into();
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::Stylize;

    #[test]
    fn new() {
        let style = Style::new().yellow();
        let sg = StyledGrapheme::new("a", style);
        assert_eq!(sg.symbol, "a");
        assert_eq!(sg.style, style);
    }

    #[test]
    fn style() {
        let style = Style::new().yellow();
        let sg = StyledGrapheme::new("a", style);
        assert_eq!(sg.style(), style);
    }

    #[test]
    fn set_style() {
        let style = Style::new().yellow().on_red();
        let style2 = Style::new().green();
        let sg = StyledGrapheme::new("a", style).set_style(style2);
        assert_eq!(sg.style, style2);
    }

    #[test]
    fn stylize() {
        let style = Style::new().yellow().on_red();
        let sg = StyledGrapheme::new("a", style).green();
        assert_eq!(sg.style, Style::new().green().on_red());
    }
}
