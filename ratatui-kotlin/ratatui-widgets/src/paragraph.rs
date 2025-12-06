//! The [`Paragraph`] widget and related types allows displaying a block of text with optional
//! wrapping, alignment, and block styling.
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Position, Rect};
use ratatui_core::style::{Style, Styled};
use ratatui_core::text::{Line, StyledGrapheme, Text};
use ratatui_core::widgets::Widget;
use unicode_width::UnicodeWidthStr;

use crate::block::{Block, BlockExt};
use crate::reflow::{LineComposer, LineTruncator, WordWrapper, WrappedLine};

/// A widget to display some text.
///
/// It is used to display a block of text. The text can be styled and aligned. It can also be
/// wrapped to the next line if it is too long to fit in the given area.
///
/// The text can be any type that can be converted into a [`Text`]. By default, the text is styled
/// with [`Style::default()`], not wrapped, and aligned to the left.
///
/// The text can be wrapped to the next line if it is too long to fit in the given area. The
/// wrapping can be configured with the [`wrap`] method. For more complex wrapping, consider using
/// the [Textwrap crate].
///
/// The text can be aligned to the left, right, or center. The alignment can be configured with the
/// [`alignment`] method or with the [`left_aligned`], [`right_aligned`], and [`centered`] methods.
///
/// The text can be scrolled to show a specific part of the text. The scroll offset can be set with
/// the [`scroll`] method.
///
/// The text can be surrounded by a [`Block`] with a title and borders. The block can be configured
/// with the [`block`] method.
///
/// The style of the text can be set with the [`style`] method. This style will be applied to the
/// entire widget, including the block if one is present. Any style set on the block or text will be
/// added to this style. See the [`Style`] type for more information on how styles are combined.
///
/// Note: If neither wrapping or a block is needed, consider rendering the [`Text`], [`Line`], or
/// [`Span`] widgets directly.
///
/// [Textwrap crate]: https://crates.io/crates/textwrap
/// [`wrap`]: Self::wrap
/// [`alignment`]: Self::alignment
/// [`left_aligned`]: Self::left_aligned
/// [`right_aligned`]: Self::right_aligned
/// [`centered`]: Self::centered
/// [`scroll`]: Self::scroll
/// [`block`]: Self::block
/// [`style`]: Self::style
///
/// # Example
///
/// ```
/// use ratatui::layout::Alignment;
/// use ratatui::style::{Style, Stylize};
/// use ratatui::text::{Line, Span};
/// use ratatui::widgets::{Block, Paragraph, Wrap};
///
/// let text = vec![
///     Line::from(vec![
///         Span::raw("First"),
///         Span::styled("line", Style::new().green().italic()),
///         ".".into(),
///     ]),
///     Line::from("Second line".red()),
///     "Third line".into(),
/// ];
/// Paragraph::new(text)
///     .block(Block::bordered().title("Paragraph"))
///     .style(Style::new().white().on_black())
///     .alignment(Alignment::Center)
///     .wrap(Wrap { trim: true });
/// ```
///
/// [`Span`]: ratatui_core::text::Span
#[derive(Debug, Default, Clone, Eq, PartialEq, Hash)]
pub struct Paragraph<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// How to wrap the text
    wrap: Option<Wrap>,
    /// The text to display
    text: Text<'a>,
    /// Scroll
    scroll: Position,
    /// Alignment of the text
    alignment: Alignment,
}

/// Describes how to wrap text across lines.
///
/// ## Examples
///
/// ```
/// use ratatui::text::Text;
/// use ratatui::widgets::{Paragraph, Wrap};
///
/// let bullet_points = Text::from(
///     r#"Some indented points:
///     - First thing goes here and is long so that it wraps
///     - Here is another point that is long enough to wrap"#,
/// );
///
/// // With leading spaces trimmed (window width of 30 chars):
/// Paragraph::new(bullet_points.clone()).wrap(Wrap { trim: true });
/// // Some indented points:
/// // - First thing goes here and is
/// // long so that it wraps
/// // - Here is another point that
/// // is long enough to wrap
///
/// // But without trimming, indentation is preserved:
/// Paragraph::new(bullet_points).wrap(Wrap { trim: false });
/// // Some indented points:
/// //     - First thing goes here
/// // and is long so that it wraps
/// //     - Here is another point
/// // that is long enough to wrap
/// ```
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
pub struct Wrap {
    /// Should leading whitespace be trimmed
    pub trim: bool,
}

type Horizontal = u16;
type Vertical = u16;

impl<'a> Paragraph<'a> {
    /// Creates a new [`Paragraph`] widget with the given text.
    ///
    /// The `text` parameter can be a [`Text`] or any type that can be converted into a [`Text`]. By
    /// default, the text is styled with [`Style::default()`], not wrapped, and aligned to the left.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::text::{Line, Text};
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello, world!");
    /// let paragraph = Paragraph::new(String::from("Hello, world!"));
    /// let paragraph = Paragraph::new(Text::raw("Hello, world!"));
    /// let paragraph = Paragraph::new(Text::styled("Hello, world!", Style::default()));
    /// let paragraph = Paragraph::new(Line::from(vec!["Hello, ".into(), "world!".red()]));
    /// ```
    pub fn new<T>(text: T) -> Self
    where
        T: Into<Text<'a>>,
    {
        Self {
            block: None,
            style: Style::default(),
            wrap: None,
            text: text.into(),
            scroll: Position::ORIGIN,
            alignment: Alignment::Left,
        }
    }

    /// Surrounds the [`Paragraph`] widget with a [`Block`].
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::widgets::{Block, Paragraph};
    ///
    /// let paragraph = Paragraph::new("Hello, world!").block(Block::bordered().title("Paragraph"));
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets the style of the entire widget.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This applies to the entire widget, including the block if one is present. Any style set on
    /// the block or text will be added to this style.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::style::{Style, Stylize};
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello, world!").style(Style::new().red().on_white());
    /// ```
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the wrapping configuration for the widget.
    ///
    /// See [`Wrap`] for more information on the different options.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::widgets::{Paragraph, Wrap};
    ///
    /// let paragraph = Paragraph::new("Hello, world!").wrap(Wrap { trim: true });
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn wrap(mut self, wrap: Wrap) -> Self {
        self.wrap = Some(wrap);
        self
    }

    /// Set the scroll offset for the given paragraph
    ///
    /// The scroll offset is a tuple of (y, x) offset. The y offset is the number of lines to
    /// scroll, and the x offset is the number of characters to scroll. The scroll offset is applied
    /// after the text is wrapped and aligned.
    ///
    /// Note: the order of the tuple is (y, x) instead of (x, y), which is different from general
    /// convention across the crate.
    ///
    /// For more information about future scrolling design and concerns, see [RFC: Design of
    /// Scrollable Widgets](https://github.com/ratatui/ratatui/discussions/1924) on GitHub.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn scroll(mut self, offset: (Vertical, Horizontal)) -> Self {
        self.scroll = Position {
            x: offset.1,
            y: offset.0,
        };
        self
    }

    /// Set the text alignment for the given paragraph
    ///
    /// The alignment is a variant of the [`Alignment`] enum which can be one of Left, Right, or
    /// Center. If no alignment is specified, the text in a paragraph will be left-aligned.
    ///
    /// # Example
    ///
    /// ```rust
    /// use ratatui::layout::Alignment;
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello World").alignment(Alignment::Center);
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn alignment(mut self, alignment: Alignment) -> Self {
        self.alignment = alignment;
        self
    }

    /// Left-aligns the text in the given paragraph.
    ///
    /// Convenience shortcut for `Paragraph::alignment(Alignment::Left)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello World").left_aligned();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn left_aligned(self) -> Self {
        self.alignment(Alignment::Left)
    }

    /// Center-aligns the text in the given paragraph.
    ///
    /// Convenience shortcut for `Paragraph::alignment(Alignment::Center)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello World").centered();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn centered(self) -> Self {
        self.alignment(Alignment::Center)
    }

    /// Right-aligns the text in the given paragraph.
    ///
    /// Convenience shortcut for `Paragraph::alignment(Alignment::Right)`.
    ///
    /// # Examples
    ///
    /// ```rust
    /// use ratatui::widgets::Paragraph;
    ///
    /// let paragraph = Paragraph::new("Hello World").right_aligned();
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn right_aligned(self) -> Self {
        self.alignment(Alignment::Right)
    }

    /// Calculates the number of lines needed to fully render.
    ///
    /// Given a max line width, this method calculates the number of lines that a paragraph will
    /// need in order to be fully rendered. For paragraphs that do not use wrapping, this count is
    /// simply the number of lines present in the paragraph.
    ///
    /// This method will also account for the [`Block`] if one is set through [`Self::block`].
    ///
    /// Note: The design for text wrapping is not stable and might affect this API.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ratatui::{widgets::{Paragraph, Wrap}};
    ///
    /// let paragraph = Paragraph::new("Hello World")
    ///     .wrap(Wrap { trim: false });
    /// assert_eq!(paragraph.line_count(20), 1);
    /// assert_eq!(paragraph.line_count(10), 2);
    /// ```
    #[instability::unstable(
        feature = "rendered-line-info",
        issue = "https://github.com/ratatui/ratatui/issues/293"
    )]
    pub fn line_count(&self, width: u16) -> usize {
        if width < 1 {
            return 0;
        }

        let (top, bottom) = self
            .block
            .as_ref()
            .map(Block::vertical_space)
            .unwrap_or_default();

        let count = if let Some(Wrap { trim }) = self.wrap {
            let styled = self.text.iter().map(|line| {
                let graphemes = line
                    .spans
                    .iter()
                    .flat_map(|span| span.styled_graphemes(self.style));
                let alignment = line.alignment.unwrap_or(self.alignment);
                (graphemes, alignment)
            });
            let mut line_composer = WordWrapper::new(styled, width, trim);
            let mut count = 0;
            while line_composer.next_line().is_some() {
                count += 1;
            }
            count
        } else {
            self.text.height()
        };

        count
            .saturating_add(top as usize)
            .saturating_add(bottom as usize)
    }

    /// Calculates the shortest line width needed to avoid any word being wrapped or truncated.
    ///
    /// Accounts for the [`Block`] if a block is set through [`Self::block`].
    ///
    /// Note: The design for text wrapping is not stable and might affect this API.
    ///
    /// # Example
    ///
    /// ```ignore
    /// use ratatui::{widgets::Paragraph};
    ///
    /// let paragraph = Paragraph::new("Hello World");
    /// assert_eq!(paragraph.line_width(), 11);
    ///
    /// let paragraph = Paragraph::new("Hello World\nhi\nHello World!!!");
    /// assert_eq!(paragraph.line_width(), 14);
    /// ```
    #[instability::unstable(
        feature = "rendered-line-info",
        issue = "https://github.com/ratatui/ratatui/issues/293"
    )]
    pub fn line_width(&self) -> usize {
        let width = self.text.iter().map(Line::width).max().unwrap_or_default();
        let (left, right) = self
            .block
            .as_ref()
            .map(Block::horizontal_space)
            .unwrap_or_default();

        width
            .saturating_add(left as usize)
            .saturating_add(right as usize)
    }
}

impl Widget for Paragraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Paragraph<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let area = area.intersection(buf.area);
        buf.set_style(area, self.style);
        self.block.as_ref().render(area, buf);
        let inner = self.block.inner_if_some(area);
        self.render_paragraph(inner, buf);
    }
}

impl Paragraph<'_> {
    fn render_paragraph(&self, text_area: Rect, buf: &mut Buffer) {
        if text_area.is_empty() {
            return;
        }

        buf.set_style(text_area, self.style);
        let styled = self.text.iter().map(|line| {
            let graphemes = line.styled_graphemes(self.text.style);
            let alignment = line.alignment.unwrap_or(self.alignment);
            (graphemes, alignment)
        });

        if let Some(Wrap { trim }) = self.wrap {
            let mut line_composer = WordWrapper::new(styled, text_area.width, trim);
            // compute the lines iteratively until we reach the desired scroll offset.
            for _ in 0..self.scroll.y {
                if line_composer.next_line().is_none() {
                    return;
                }
            }
            render_lines(line_composer, text_area, buf);
        } else {
            // avoid unnecessary work by skipping directly to the relevant line before rendering
            let lines = styled.skip(self.scroll.y as usize);
            let mut line_composer = LineTruncator::new(lines, text_area.width);
            line_composer.set_horizontal_offset(self.scroll.x);
            render_lines(line_composer, text_area, buf);
        }
    }
}

fn render_lines<'a, C: LineComposer<'a>>(mut composer: C, area: Rect, buf: &mut Buffer) {
    let mut y = 0;
    while let Some(ref wrapped) = composer.next_line() {
        render_line(wrapped, area, buf, y);
        y += 1;
        if y >= area.height {
            break;
        }
    }
}

fn render_line(wrapped: &WrappedLine<'_, '_>, area: Rect, buf: &mut Buffer, y: u16) {
    let mut x = get_line_offset(wrapped.width, area.width, wrapped.alignment);
    for StyledGrapheme { symbol, style } in wrapped.graphemes {
        let width = symbol.width();
        if width == 0 {
            continue;
        }
        // Make sure to overwrite any previous character with a space (rather than a zero-width)
        let symbol = if symbol.is_empty() { " " } else { symbol };
        let position = Position::new(area.left() + x, area.top() + y);
        buf[position].set_symbol(symbol).set_style(*style);
        x += u16::try_from(width).unwrap_or(u16::MAX);
    }
}

const fn get_line_offset(line_width: u16, text_area_width: u16, alignment: Alignment) -> u16 {
    match alignment {
        Alignment::Center => (text_area_width / 2).saturating_sub(line_width / 2),
        Alignment::Right => text_area_width.saturating_sub(line_width),
        Alignment::Left => 0,
    }
}

impl Styled for Paragraph<'_> {
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
    use alloc::vec;

    use ratatui_core::buffer::Buffer;
    use ratatui_core::layout::{Alignment, Rect};
    use ratatui_core::style::{Color, Modifier, Style, Stylize};
    use ratatui_core::text::{Line, Span, Text};
    use ratatui_core::widgets::Widget;
    use rstest::rstest;

    use super::*;
    use crate::block::TitlePosition;
    use crate::borders::Borders;

    /// Tests the [`Paragraph`] widget against the expected [`Buffer`] by rendering it onto an equal
    /// area and comparing the rendered and expected content.
    /// This can be used for easy testing of varying configured paragraphs with the same expected
    /// buffer or any other test case really.
    #[track_caller]
    fn test_case(paragraph: &Paragraph, expected: &Buffer) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, expected.area.width, expected.area.height));
        paragraph.render(buffer.area, &mut buffer);
        assert_eq!(buffer, *expected);
    }

    #[test]
    fn zero_width_char_at_end_of_line() {
        let line = "foo\u{200B}";
        for paragraph in [
            Paragraph::new(line),
            Paragraph::new(line).wrap(Wrap { trim: false }),
            Paragraph::new(line).wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::with_lines(["foo"]));
            test_case(&paragraph, &Buffer::with_lines(["foo   "]));
            test_case(&paragraph, &Buffer::with_lines(["foo   ", "      "]));
            test_case(&paragraph, &Buffer::with_lines(["foo", "   "]));
        }
    }

    #[test]
    fn test_render_empty_paragraph() {
        for paragraph in [
            Paragraph::new(""),
            Paragraph::new("").wrap(Wrap { trim: false }),
            Paragraph::new("").wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::with_lines([" "]));
            test_case(&paragraph, &Buffer::with_lines(["          "]));
            test_case(&paragraph, &Buffer::with_lines(["     "; 10]));
            test_case(&paragraph, &Buffer::with_lines([" ", " "]));
        }
    }

    #[test]
    fn test_render_single_line_paragraph() {
        let text = "Hello, world!";
        for paragraph in [
            Paragraph::new(text),
            Paragraph::new(text).wrap(Wrap { trim: false }),
            Paragraph::new(text).wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::with_lines(["Hello, world!  "]));
            test_case(&paragraph, &Buffer::with_lines(["Hello, world!"]));
            test_case(
                &paragraph,
                &Buffer::with_lines(["Hello, world!  ", "               "]),
            );
            test_case(
                &paragraph,
                &Buffer::with_lines(["Hello, world!", "             "]),
            );
        }
    }

    #[test]
    fn test_render_multi_line_paragraph() {
        let text = "This is a\nmultiline\nparagraph.";
        for paragraph in [
            Paragraph::new(text),
            Paragraph::new(text).wrap(Wrap { trim: false }),
            Paragraph::new(text).wrap(Wrap { trim: true }),
        ] {
            test_case(
                &paragraph,
                &Buffer::with_lines(["This is a ", "multiline ", "paragraph."]),
            );
            test_case(
                &paragraph,
                &Buffer::with_lines(["This is a      ", "multiline      ", "paragraph.     "]),
            );
            test_case(
                &paragraph,
                &Buffer::with_lines([
                    "This is a      ",
                    "multiline      ",
                    "paragraph.     ",
                    "               ",
                    "               ",
                ]),
            );
        }
    }

    #[test]
    fn test_render_paragraph_with_block() {
        // We use the slightly unconventional "worlds" instead of "world" here to make sure when we
        // can truncate this without triggering the typos linter.
        let text = "Hello, worlds!";
        let truncated_paragraph = Paragraph::new(text).block(Block::bordered().title("Title"));
        let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
        let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            #[rustfmt::skip]
            test_case(
                paragraph,
                &Buffer::with_lines([
                    "┌Title─────────┐",
                    "│Hello, worlds!│",
                    "└──────────────┘",
                ]),
            );
            test_case(
                paragraph,
                &Buffer::with_lines([
                    "┌Title───────────┐",
                    "│Hello, worlds!  │",
                    "└────────────────┘",
                ]),
            );
            test_case(
                paragraph,
                &Buffer::with_lines([
                    "┌Title────────────┐",
                    "│Hello, worlds!   │",
                    "│                 │",
                    "└─────────────────┘",
                ]),
            );
        }

        test_case(
            &truncated_paragraph,
            &Buffer::with_lines([
                "┌Title───────┐",
                "│Hello, world│",
                "│            │",
                "└────────────┘",
            ]),
        );
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines([
                "┌Title──────┐",
                "│Hello,     │",
                "│worlds!    │",
                "└───────────┘",
            ]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines([
                "┌Title──────┐",
                "│Hello,     │",
                "│worlds!    │",
                "└───────────┘",
            ]),
        );
    }

    #[test]
    fn test_render_line_styled() {
        let l0 = Line::raw("unformatted");
        let l1 = Line::styled("bold text", Style::new().bold());
        let l2 = Line::styled("cyan text", Style::new().cyan());
        let l3 = Line::styled("dim text", Style::new().dim());
        let paragraph = Paragraph::new(vec![l0, l1, l2, l3]);

        let mut expected =
            Buffer::with_lines(["unformatted", "bold text", "cyan text", "dim text"]);
        expected.set_style(Rect::new(0, 1, 9, 1), Style::new().bold());
        expected.set_style(Rect::new(0, 2, 9, 1), Style::new().cyan());
        expected.set_style(Rect::new(0, 3, 8, 1), Style::new().dim());

        test_case(&paragraph, &expected);
    }

    #[test]
    fn test_render_line_spans_styled() {
        let l0 = Line::default().spans([
            Span::styled("bold", Style::new().bold()),
            Span::raw(" and "),
            Span::styled("cyan", Style::new().cyan()),
        ]);
        let l1 = Line::default().spans([Span::raw("unformatted")]);
        let paragraph = Paragraph::new(vec![l0, l1]);

        let mut expected = Buffer::with_lines(["bold and cyan", "unformatted"]);
        expected.set_style(Rect::new(0, 0, 4, 1), Style::new().bold());
        expected.set_style(Rect::new(9, 0, 4, 1), Style::new().cyan());

        test_case(&paragraph, &expected);
    }

    #[test]
    fn test_render_paragraph_with_block_with_bottom_title_and_border() {
        let block = Block::new()
            .borders(Borders::BOTTOM)
            .title_position(TitlePosition::Bottom)
            .title("Title");
        let paragraph = Paragraph::new("Hello, world!").block(block);
        test_case(
            &paragraph,
            &Buffer::with_lines(["Hello, world!  ", "Title──────────"]),
        );
    }

    #[test]
    fn test_render_paragraph_with_word_wrap() {
        let text = "This is a long line of text that should wrap      and contains a superultramegagigalong word.";
        let wrapped_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines([
                "This is a long line",
                "of text that should",
                "wrap      and      ",
                "contains a         ",
                "superultramegagigal",
                "ong word.          ",
            ]),
        );
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines([
                "This is a   ",
                "long line of",
                "text that   ",
                "should wrap ",
                "    and     ",
                "contains a  ",
                "superultrame",
                "gagigalong  ",
                "word.       ",
            ]),
        );

        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines([
                "This is a long line",
                "of text that should",
                "wrap      and      ",
                "contains a         ",
                "superultramegagigal",
                "ong word.          ",
            ]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines([
                "This is a   ",
                "long line of",
                "text that   ",
                "should wrap ",
                "and contains",
                "a           ",
                "superultrame",
                "gagigalong  ",
                "word.       ",
            ]),
        );
    }

    #[test]
    fn test_render_wrapped_paragraph_with_whitespace_only_line() {
        let text: Text = ["A", "  ", "B", "  a", "C"]
            .into_iter()
            .map(Line::from)
            .collect();
        let paragraph = Paragraph::new(text.clone()).wrap(Wrap { trim: false });
        let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

        test_case(
            &paragraph,
            &Buffer::with_lines(["A", "  ", "B", "  a", "C"]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines(["A", "", "B", "a", "C"]),
        );
    }

    #[test]
    fn test_render_paragraph_with_line_truncation() {
        let text = "This is a long line of text that should be truncated.";
        let truncated_paragraph = Paragraph::new(text);

        test_case(
            &truncated_paragraph,
            &Buffer::with_lines(["This is a long line of"]),
        );
        test_case(
            &truncated_paragraph,
            &Buffer::with_lines(["This is a long line of te"]),
        );
        test_case(
            &truncated_paragraph,
            &Buffer::with_lines(["This is a long line of "]),
        );
        test_case(
            &truncated_paragraph.clone().scroll((0, 2)),
            &Buffer::with_lines(["is is a long line of te"]),
        );
    }

    #[test]
    fn test_render_paragraph_with_left_alignment() {
        let text = "Hello, world!";
        let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Left);
        let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
        let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            test_case(paragraph, &Buffer::with_lines(["Hello, world!  "]));
            test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
        }

        test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines(["Hello,    ", "world!    "]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines(["Hello,    ", "world!    "]),
        );
    }

    #[test]
    fn test_render_paragraph_with_center_alignment() {
        let text = "Hello, world!";
        let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Center);
        let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
        let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            test_case(paragraph, &Buffer::with_lines([" Hello, world! "]));
            test_case(paragraph, &Buffer::with_lines(["  Hello, world! "]));
            test_case(paragraph, &Buffer::with_lines(["  Hello, world!  "]));
            test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
        }

        test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines(["  Hello,  ", "  world!  "]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines(["  Hello,  ", "  world!  "]),
        );
    }

    #[test]
    fn test_render_paragraph_with_right_alignment() {
        let text = "Hello, world!";
        let truncated_paragraph = Paragraph::new(text).alignment(Alignment::Right);
        let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
        let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            test_case(paragraph, &Buffer::with_lines(["  Hello, world!"]));
            test_case(paragraph, &Buffer::with_lines(["Hello, world!"]));
        }

        test_case(&truncated_paragraph, &Buffer::with_lines(["Hello, wor"]));
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines(["    Hello,", "    world!"]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines(["    Hello,", "    world!"]),
        );
    }

    #[test]
    fn test_render_paragraph_with_scroll_offset() {
        let text = "This is a\ncool\nmultiline\nparagraph.";
        let truncated_paragraph = Paragraph::new(text).scroll((2, 0));
        let wrapped_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: false });
        let trimmed_paragraph = truncated_paragraph.clone().wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            test_case(
                paragraph,
                &Buffer::with_lines(["multiline   ", "paragraph.  ", "            "]),
            );
            test_case(paragraph, &Buffer::with_lines(["multiline   "]));
        }

        test_case(
            &truncated_paragraph.clone().scroll((2, 4)),
            &Buffer::with_lines(["iline   ", "graph.  "]),
        );
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines(["cool   ", "multili", "ne     "]),
        );
    }

    #[test]
    fn test_render_paragraph_with_zero_width_area() {
        let text = "Hello, world!";
        let area = Rect::new(0, 0, 0, 3);

        for paragraph in [
            Paragraph::new(text),
            Paragraph::new(text).wrap(Wrap { trim: false }),
            Paragraph::new(text).wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::empty(area));
            test_case(&paragraph.clone().scroll((2, 4)), &Buffer::empty(area));
        }
    }

    #[test]
    fn test_render_paragraph_with_zero_height_area() {
        let text = "Hello, world!";
        let area = Rect::new(0, 0, 10, 0);

        for paragraph in [
            Paragraph::new(text),
            Paragraph::new(text).wrap(Wrap { trim: false }),
            Paragraph::new(text).wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::empty(area));
            test_case(&paragraph.clone().scroll((2, 4)), &Buffer::empty(area));
        }
    }

    #[test]
    fn test_render_paragraph_with_styled_text() {
        let text = Line::from(vec![
            Span::styled("Hello, ", Style::default().fg(Color::Red)),
            Span::styled("world!", Style::default().fg(Color::Blue)),
        ]);

        let mut expected_buffer = Buffer::with_lines(["Hello, world!"]);
        expected_buffer.set_style(
            Rect::new(0, 0, 7, 1),
            Style::default().fg(Color::Red).bg(Color::Green),
        );
        expected_buffer.set_style(
            Rect::new(7, 0, 6, 1),
            Style::default().fg(Color::Blue).bg(Color::Green),
        );

        for paragraph in [
            Paragraph::new(text.clone()),
            Paragraph::new(text.clone()).wrap(Wrap { trim: false }),
            Paragraph::new(text.clone()).wrap(Wrap { trim: true }),
        ] {
            test_case(
                &paragraph.style(Style::default().bg(Color::Green)),
                &expected_buffer,
            );
        }
    }

    #[test]
    fn test_render_paragraph_with_special_characters() {
        let text = "Hello, <world>!";
        for paragraph in [
            Paragraph::new(text),
            Paragraph::new(text).wrap(Wrap { trim: false }),
            Paragraph::new(text).wrap(Wrap { trim: true }),
        ] {
            test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!"]));
            test_case(&paragraph, &Buffer::with_lines(["Hello, <world>!     "]));
            test_case(
                &paragraph,
                &Buffer::with_lines(["Hello, <world>!     ", "                    "]),
            );
            test_case(
                &paragraph,
                &Buffer::with_lines(["Hello, <world>!", "               "]),
            );
        }
    }

    #[test]
    fn test_render_paragraph_with_unicode_characters() {
        let text = "こんにちは, 世界! 😃";
        let truncated_paragraph = Paragraph::new(text);
        let wrapped_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        let trimmed_paragraph = Paragraph::new(text).wrap(Wrap { trim: true });

        for paragraph in [&truncated_paragraph, &wrapped_paragraph, &trimmed_paragraph] {
            test_case(paragraph, &Buffer::with_lines(["こんにちは, 世界! 😃"]));
            test_case(
                paragraph,
                &Buffer::with_lines(["こんにちは, 世界! 😃     "]),
            );
        }

        test_case(
            &truncated_paragraph,
            &Buffer::with_lines(["こんにちは, 世 "]),
        );
        test_case(
            &wrapped_paragraph,
            &Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]),
        );
        test_case(
            &trimmed_paragraph,
            &Buffer::with_lines(["こんにちは,    ", "世界! 😃      "]),
        );
    }

    #[test]
    fn can_be_stylized() {
        assert_eq!(
            Paragraph::new("").black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn widgets_paragraph_count_rendered_lines() {
        let paragraph = Paragraph::new("Hello World");
        assert_eq!(paragraph.line_count(20), 1);
        assert_eq!(paragraph.line_count(10), 1);
        let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_count(20), 1);
        assert_eq!(paragraph.line_count(10), 2);
        let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_count(20), 1);
        assert_eq!(paragraph.line_count(10), 2);

        let text = "Hello World ".repeat(100);
        let paragraph = Paragraph::new(text.trim());
        assert_eq!(paragraph.line_count(11), 1);
        assert_eq!(paragraph.line_count(6), 1);
        let paragraph = paragraph.wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_count(11), 100);
        assert_eq!(paragraph.line_count(6), 200);
        let paragraph = paragraph.wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_count(11), 100);
        assert_eq!(paragraph.line_count(6), 200);
    }

    #[test]
    fn widgets_paragraph_rendered_line_count_accounts_block() {
        let block = Block::new();
        let paragraph = Paragraph::new("Hello World").block(block);
        assert_eq!(paragraph.line_count(20), 1);
        assert_eq!(paragraph.line_count(10), 1);

        let block = Block::new().borders(Borders::TOP);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(20), 2);
        assert_eq!(paragraph.line_count(10), 2);

        let block = Block::new().borders(Borders::BOTTOM);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(20), 2);
        assert_eq!(paragraph.line_count(10), 2);

        let block = Block::new().borders(Borders::TOP | Borders::BOTTOM);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(20), 3);
        assert_eq!(paragraph.line_count(10), 3);

        let block = Block::bordered();
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(20), 3);
        assert_eq!(paragraph.line_count(10), 3);

        let block = Block::bordered();
        let paragraph = paragraph.block(block).wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_count(20), 3);
        assert_eq!(paragraph.line_count(10), 4);

        let block = Block::bordered();
        let paragraph = paragraph.block(block).wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_count(20), 3);
        assert_eq!(paragraph.line_count(10), 4);

        let text = "Hello World ".repeat(100);
        let block = Block::new();
        let paragraph = Paragraph::new(text.trim()).block(block);
        assert_eq!(paragraph.line_count(11), 1);

        let block = Block::bordered();
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(11), 3);
        assert_eq!(paragraph.line_count(6), 3);

        let block = Block::new().borders(Borders::TOP);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(11), 2);
        assert_eq!(paragraph.line_count(6), 2);

        let block = Block::new().borders(Borders::BOTTOM);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(11), 2);
        assert_eq!(paragraph.line_count(6), 2);

        let block = Block::new().borders(Borders::LEFT | Borders::RIGHT);
        let paragraph = paragraph.block(block);
        assert_eq!(paragraph.line_count(11), 1);
        assert_eq!(paragraph.line_count(6), 1);
    }

    #[test]
    fn widgets_paragraph_line_width() {
        let paragraph = Paragraph::new("Hello World");
        assert_eq!(paragraph.line_width(), 11);
        let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_width(), 11);
        let paragraph = Paragraph::new("Hello World").wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_width(), 11);

        let text = "Hello World ".repeat(100);
        let paragraph = Paragraph::new(text);
        assert_eq!(paragraph.line_width(), 1200);
        let paragraph = paragraph.wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_width(), 1200);
        let paragraph = paragraph.wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_width(), 1200);
    }

    #[test]
    fn widgets_paragraph_line_width_accounts_for_block() {
        let block = Block::bordered();
        let paragraph = Paragraph::new("Hello World").block(block);
        assert_eq!(paragraph.line_width(), 13);

        let block = Block::new().borders(Borders::LEFT);
        let paragraph = Paragraph::new("Hello World").block(block);
        assert_eq!(paragraph.line_width(), 12);

        let block = Block::new().borders(Borders::LEFT);
        let paragraph = Paragraph::new("Hello World")
            .block(block)
            .wrap(Wrap { trim: true });
        assert_eq!(paragraph.line_width(), 12);

        let block = Block::new().borders(Borders::LEFT);
        let paragraph = Paragraph::new("Hello World")
            .block(block)
            .wrap(Wrap { trim: false });
        assert_eq!(paragraph.line_width(), 12);
    }

    #[test]
    fn left_aligned() {
        let p = Paragraph::new("Hello, world!").left_aligned();
        assert_eq!(p.alignment, Alignment::Left);
    }

    #[test]
    fn centered() {
        let p = Paragraph::new("Hello, world!").centered();
        assert_eq!(p.alignment, Alignment::Center);
    }

    #[test]
    fn right_aligned() {
        let p = Paragraph::new("Hello, world!").right_aligned();
        assert_eq!(p.alignment, Alignment::Right);
    }

    /// Regression test for <https://github.com/ratatui/ratatui/issues/990>
    ///
    /// This test ensures that paragraphs with a block and styled text are rendered correctly.
    /// It has been simplified from the original issue but tests the same functionality.
    #[test]
    fn paragraph_block_text_style() {
        let text = Text::styled("Styled text", Color::Green);
        let paragraph = Paragraph::new(text).block(Block::bordered());

        let mut buf = Buffer::empty(Rect::new(0, 0, 20, 3));
        paragraph.render(Rect::new(0, 0, 20, 3), &mut buf);

        let mut expected = Buffer::with_lines([
            "┌──────────────────┐",
            "│Styled text       │",
            "└──────────────────┘",
        ]);
        expected.set_style(Rect::new(1, 1, 11, 1), Style::default().fg(Color::Green));
        assert_eq!(buf, expected);
    }

    #[rstest]
    #[case::bottom(Rect::new(0, 5, 15, 1))]
    #[case::right(Rect::new(20, 0, 15, 1))]
    #[case::bottom_right(Rect::new(20, 5, 15, 1))]
    fn test_render_paragraph_out_of_bounds(#[case] area: Rect) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 3));
        Paragraph::new("Beyond the pale").render(area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(vec!["          "; 3]));
    }

    #[test]
    fn partial_out_of_bounds() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 15, 3));
        Paragraph::new("Hello World").render(Rect::new(10, 0, 10, 3), &mut buffer);
        assert_eq!(
            buffer,
            Buffer::with_lines(vec![
                "          Hello",
                "               ",
                "               ",
            ])
        );
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let paragraph = Paragraph::new("Lorem ipsum");
        // This should not panic, even if the buffer is too small to render the paragraph.
        paragraph.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["L"]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let paragraph = Paragraph::new("Lorem ipsum");
        // This should not panic, even if the buffer has zero size.
        paragraph.render(buffer.area, &mut buffer);
    }
}
