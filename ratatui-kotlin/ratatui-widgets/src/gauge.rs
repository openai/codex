//! The [`Gauge`] widget is used to display a horizontal progress bar.
use alloc::format;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Color, Style, Styled};
use ratatui_core::symbols;
use ratatui_core::text::{Line, Span};
use ratatui_core::widgets::Widget;

use crate::block::{Block, BlockExt};
#[cfg(not(feature = "std"))]
use crate::polyfills::F64Polyfills;

/// A widget to display a progress bar.
///
/// A `Gauge` renders a bar filled according to the value given to [`Gauge::percent`] or
/// [`Gauge::ratio`]. The bar width and height are defined by the [`Rect`] it is
/// [rendered](Widget::render) in.
///
/// The associated label is always centered horizontally and vertically. If not set with
/// [`Gauge::label`], the label is the percentage of the bar filled.
///
/// You might want to have a higher precision bar using [`Gauge::use_unicode`].
///
/// This can be useful to indicate the progression of a task, like a download.
///
/// # Example
///
/// ```
/// use ratatui::style::{Style, Stylize};
/// use ratatui::widgets::{Block, Gauge};
///
/// Gauge::default()
///     .block(Block::bordered().title("Progress"))
///     .gauge_style(Style::new().white().on_black().italic())
///     .percent(20);
/// ```
///
/// # See also
///
/// - [`LineGauge`] for a thin progress bar
#[expect(clippy::struct_field_names)] // gauge_style needs to be differentiated to style
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Gauge<'a> {
    block: Option<Block<'a>>,
    ratio: f64,
    label: Option<Span<'a>>,
    use_unicode: bool,
    style: Style,
    gauge_style: Style,
}

impl<'a> Gauge<'a> {
    /// Surrounds the `Gauge` with a [`Block`].
    ///
    /// The gauge is rendered in the inner portion of the block once space for borders and padding
    /// is reserved. Styles set on the block do **not** affect the bar itself.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets the bar progression from a percentage.
    ///
    /// # Panics
    ///
    /// This method panics if `percent` is **not** between 0 and 100 inclusively.
    ///
    /// # See also
    ///
    /// See [`Gauge::ratio`] to set from a float.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn percent(mut self, percent: u16) -> Self {
        assert!(
            percent <= 100,
            "Percentage should be between 0 and 100 inclusively."
        );
        self.ratio = f64::from(percent) / 100.0;
        self
    }

    /// Sets the bar progression from a ratio (float).
    ///
    /// `ratio` is the ratio between filled bar over empty bar (i.e. `3/4` completion is `0.75`).
    /// This is more easily seen as a floating point percentage (e.g. 42% = `0.42`).
    ///
    /// # Panics
    ///
    /// This method panics if `ratio` is **not** between 0 and 1 inclusively.
    ///
    /// # See also
    ///
    /// See [`Gauge::percent`] to set from a percentage.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn ratio(mut self, ratio: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&ratio),
            "Ratio should be between 0 and 1 inclusively."
        );
        self.ratio = ratio;
        self
    }

    /// Sets the label to display in the center of the bar.
    ///
    /// For a left-aligned label, see [`LineGauge`].
    /// If the label is not defined, it is the percentage filled.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn label<T>(mut self, label: T) -> Self
    where
        T: Into<Span<'a>>,
    {
        self.label = Some(label.into());
        self
    }

    /// Sets the widget style.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This will style the block (if any non-styled) and background of the widget (everything
    /// except the bar itself). [`Block`] style set with [`Gauge::block`] takes precedence.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the style of the bar.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn gauge_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.gauge_style = style.into();
        self
    }

    /// Sets whether to use unicode characters to display the progress bar.
    ///
    /// This enables the use of
    /// [unicode block characters](https://en.wikipedia.org/wiki/Block_Elements).
    /// This is useful to display a higher precision bar (8 extra fractional parts per cell).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn use_unicode(mut self, unicode: bool) -> Self {
        self.use_unicode = unicode;
        self
    }
}

impl Widget for Gauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Gauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        self.block.as_ref().render(area, buf);
        let inner = self.block.inner_if_some(area);
        self.render_gauge(inner, buf);
    }
}

impl Gauge<'_> {
    fn render_gauge(&self, gauge_area: Rect, buf: &mut Buffer) {
        if gauge_area.is_empty() {
            return;
        }

        buf.set_style(gauge_area, self.gauge_style);

        // compute label value and its position
        // label is put at the center of the gauge_area
        let default_label = Span::raw(format!("{}%", f64::round(self.ratio * 100.0)));
        let label = self.label.as_ref().unwrap_or(&default_label);
        let clamped_label_width = gauge_area.width.min(label.width() as u16);
        let label_col = gauge_area.left() + (gauge_area.width - clamped_label_width) / 2;
        let label_row = gauge_area.top() + gauge_area.height / 2;

        // the gauge will be filled proportionally to the ratio
        let filled_width = f64::from(gauge_area.width) * self.ratio;
        let end = if self.use_unicode {
            gauge_area.left() + filled_width.floor() as u16
        } else {
            gauge_area.left() + filled_width.round() as u16
        };
        for y in gauge_area.top()..gauge_area.bottom() {
            // render the filled area (left to end)
            for x in gauge_area.left()..end {
                // Use full block for the filled part of the gauge and spaces for the part that is
                // covered by the label. Note that the background and foreground colors are swapped
                // for the label part, otherwise the gauge will be inverted
                if x < label_col || x > label_col + clamped_label_width || y != label_row {
                    buf[(x, y)]
                        .set_symbol(symbols::block::FULL)
                        .set_fg(self.gauge_style.fg.unwrap_or(Color::Reset))
                        .set_bg(self.gauge_style.bg.unwrap_or(Color::Reset));
                } else {
                    buf[(x, y)]
                        .set_symbol(" ")
                        .set_fg(self.gauge_style.bg.unwrap_or(Color::Reset))
                        .set_bg(self.gauge_style.fg.unwrap_or(Color::Reset));
                }
            }
            if self.use_unicode && self.ratio < 1.0 {
                buf[(end, y)].set_symbol(get_unicode_block(filled_width % 1.0));
            }
        }
        // render the label
        buf.set_span(label_col, label_row, label, clamped_label_width);
    }
}

fn get_unicode_block<'a>(frac: f64) -> &'a str {
    match (frac * 8.0).round() as u16 {
        1 => symbols::block::ONE_EIGHTH,
        2 => symbols::block::ONE_QUARTER,
        3 => symbols::block::THREE_EIGHTHS,
        4 => symbols::block::HALF,
        5 => symbols::block::FIVE_EIGHTHS,
        6 => symbols::block::THREE_QUARTERS,
        7 => symbols::block::SEVEN_EIGHTHS,
        8 => symbols::block::FULL,
        _ => " ",
    }
}

/// A compact widget to display a progress bar over a single thin line.
///
/// This can be useful to indicate the progression of a task, like a download.
///
/// A `LineGauge` renders a line filled with symbols defined by [`LineGauge::filled_symbol`] and
/// [`LineGauge::unfilled_symbol`] according to the value given to [`LineGauge::ratio`].
/// Unlike [`Gauge`], only the width can be defined by the [rendering](Widget::render) [`Rect`]. The
/// height is always 1.
///
/// The associated label is always left-aligned. If not set with [`LineGauge::label`], the label is
/// the percentage of the bar filled.
///
/// You can also set the symbols used to draw the bar with [`LineGauge::line_set`].
///
/// To style the gauge line use [`LineGauge::filled_style`] and [`LineGauge::unfilled_style`] which
/// let you pick a color for foreground (i.e. line) and background of the filled and unfilled part
/// of gauge respectively.
///
/// # Examples:
///
/// ```
/// use ratatui::style::{Style, Stylize};
/// use ratatui::symbols;
/// use ratatui::widgets::{Block, LineGauge};
///
/// LineGauge::default()
///     .block(Block::bordered().title("Progress"))
///     .filled_style(Style::new().white().on_black().bold())
///     .filled_symbol(symbols::line::THICK_HORIZONTAL)
///     .ratio(0.4);
/// ```
///
/// # See also
///
/// - [`Gauge`] for bigger, higher precision and more configurable progress bar
#[derive(Debug, Clone, PartialEq)]
pub struct LineGauge<'a> {
    block: Option<Block<'a>>,
    ratio: f64,
    label: Option<Line<'a>>,
    style: Style,
    filled_symbol: &'a str,
    unfilled_symbol: &'a str,
    filled_style: Style,
    unfilled_style: Style,
}

impl Default for LineGauge<'_> {
    fn default() -> Self {
        Self {
            block: None,
            ratio: 0.0,
            label: None,
            style: Style::default(),
            filled_symbol: symbols::line::HORIZONTAL,
            unfilled_symbol: symbols::line::HORIZONTAL,
            filled_style: Style::default(),
            unfilled_style: Style::default(),
        }
    }
}

impl<'a> LineGauge<'a> {
    /// Surrounds the `LineGauge` with a [`Block`].
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Sets the bar progression from a ratio (float).
    ///
    /// `ratio` is the ratio between filled bar over empty bar (i.e. `3/4` completion is `0.75`).
    /// This is more easily seen as a floating point percentage (e.g. 42% = `0.42`).
    ///
    /// # Panics
    ///
    /// This method panics if `ratio` is **not** between 0 and 1 inclusively.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn ratio(mut self, ratio: f64) -> Self {
        assert!(
            (0.0..=1.0).contains(&ratio),
            "Ratio should be between 0 and 1 inclusively."
        );
        self.ratio = ratio;
        self
    }

    /// Sets the characters to use for the line.
    ///
    /// # See also
    ///
    /// See [`symbols::line::Set`] for more information. Predefined sets are also available, see
    /// [`NORMAL`](symbols::line::NORMAL), [`DOUBLE`](symbols::line::DOUBLE) and
    /// [`THICK`](symbols::line::THICK).
    #[must_use = "method moves the value of self and returns the modified value"]
    #[deprecated(
        since = "0.30.0",
        note = "use `filled_symbol()` and `unfilled_symbol()` instead"
    )]
    pub const fn line_set(mut self, set: symbols::line::Set<'a>) -> Self {
        self.filled_symbol = set.horizontal;
        self.unfilled_symbol = set.horizontal;
        self
    }

    /// Sets the symbol for the filled part of the gauge.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn filled_symbol(mut self, symbol: &'a str) -> Self {
        self.filled_symbol = symbol;
        self
    }

    /// Sets the symbol for the unfilled part of the gauge.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn unfilled_symbol(mut self, symbol: &'a str) -> Self {
        self.unfilled_symbol = symbol;
        self
    }

    /// Sets the label to display.
    ///
    /// With `LineGauge`, labels are only on the left, see [`Gauge`] for a centered label.
    /// If the label is not defined, it is the percentage filled.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn label<T>(mut self, label: T) -> Self
    where
        T: Into<Line<'a>>,
    {
        self.label = Some(label.into());
        self
    }

    /// Sets the widget style.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This will style everything except the bar itself, so basically the block (if any) and
    /// background.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the style of the bar.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    #[deprecated(since = "0.27.0", note = "use `filled_style()` instead")]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn gauge_style<S: Into<Style>>(mut self, style: S) -> Self {
        let style: Style = style.into();

        // maintain backward compatibility, which used the background color of the style as the
        // unfilled part of the gauge and the foreground color as the filled part of the gauge
        let filled_color = style.fg.unwrap_or(Color::Reset);
        let unfilled_color = style.bg.unwrap_or(Color::Reset);
        self.filled_style = style.fg(filled_color).bg(Color::Reset);
        self.unfilled_style = style.fg(unfilled_color).bg(Color::Reset);
        self
    }

    /// Sets the style of filled part of the bar.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn filled_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.filled_style = style.into();
        self
    }

    /// Sets the style of the unfilled part of the bar.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn unfilled_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.unfilled_style = style.into();
        self
    }
}

impl Widget for LineGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &LineGauge<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        buf.set_style(area, self.style);
        self.block.as_ref().render(area, buf);
        let gauge_area = self.block.inner_if_some(area);
        if gauge_area.is_empty() {
            return;
        }

        let ratio = self.ratio;
        let default_label = Line::from(format!("{:3.0}%", ratio * 100.0));
        let label = self.label.as_ref().unwrap_or(&default_label);
        let (col, row) = buf.set_line(gauge_area.left(), gauge_area.top(), label, gauge_area.width);
        let start = col + 1;
        if start >= gauge_area.right() {
            return;
        }

        let end = start
            + (f64::from(gauge_area.right().saturating_sub(start)) * self.ratio).floor() as u16;
        for col in start..end {
            buf[(col, row)]
                .set_symbol(self.filled_symbol)
                .set_style(self.filled_style);
        }
        for col in end..gauge_area.right() {
            buf[(col, row)]
                .set_symbol(self.unfilled_symbol)
                .set_style(self.unfilled_style);
        }
    }
}

impl Styled for Gauge<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Styled for LineGauge<'_> {
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
    use ratatui_core::style::{Color, Modifier, Style, Stylize};
    use ratatui_core::symbols;

    use super::*;

    #[test]
    #[should_panic = "Percentage should be between 0 and 100 inclusively"]
    fn gauge_invalid_percentage() {
        let _ = Gauge::default().percent(110);
    }

    #[test]
    #[should_panic = "Ratio should be between 0 and 1 inclusively"]
    fn gauge_invalid_ratio_upper_bound() {
        let _ = Gauge::default().ratio(1.1);
    }

    #[test]
    #[should_panic = "Ratio should be between 0 and 1 inclusively"]
    fn gauge_invalid_ratio_lower_bound() {
        let _ = Gauge::default().ratio(-0.5);
    }

    #[test]
    fn gauge_can_be_stylized() {
        assert_eq!(
            Gauge::default().black().on_white().bold().not_dim().style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[test]
    fn line_gauge_can_be_stylized() {
        assert_eq!(
            LineGauge::default()
                .black()
                .on_white()
                .bold()
                .not_dim()
                .style,
            Style::default()
                .fg(Color::Black)
                .bg(Color::White)
                .add_modifier(Modifier::BOLD)
                .remove_modifier(Modifier::DIM)
        );
    }

    #[expect(deprecated)]
    #[test]
    fn line_gauge_can_be_stylized_with_deprecated_gauge_style() {
        let gauge =
            LineGauge::default().gauge_style(Style::default().fg(Color::Red).bg(Color::Blue));

        assert_eq!(
            gauge.filled_style,
            Style::default().fg(Color::Red).bg(Color::Reset)
        );

        assert_eq!(
            gauge.unfilled_style,
            Style::default().fg(Color::Blue).bg(Color::Reset)
        );
    }

    #[test]
    fn line_gauge_set_filled_symbol() {
        assert_eq!(LineGauge::default().filled_symbol("▰").filled_symbol, "▰");
    }

    #[test]
    fn line_gauge_set_unfilled_symbol() {
        assert_eq!(
            LineGauge::default().unfilled_symbol("▱").unfilled_symbol,
            "▱"
        );
    }

    #[expect(deprecated)]
    #[test]
    fn line_gauge_deprecated_line_set() {
        let gauge = LineGauge::default().line_set(symbols::line::DOUBLE);
        assert_eq!(gauge.filled_symbol, symbols::line::DOUBLE.horizontal);
        assert_eq!(gauge.unfilled_symbol, symbols::line::DOUBLE.horizontal);
    }

    #[test]
    fn line_gauge_default() {
        assert_eq!(
            LineGauge::default(),
            LineGauge {
                block: None,
                ratio: 0.0,
                label: None,
                style: Style::default(),
                filled_symbol: symbols::line::HORIZONTAL,
                unfilled_symbol: symbols::line::HORIZONTAL,
                filled_style: Style::default(),
                unfilled_style: Style::default()
            }
        );
    }

    #[test]
    fn render_in_minimal_buffer_gauge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let gauge = Gauge::default().percent(50);
        // This should not panic, even if the buffer is too small to render the gauge.
        gauge.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["5"]));
    }

    #[test]
    fn render_in_minimal_buffer_line_gauge() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let line_gauge = LineGauge::default().ratio(0.5);
        // This should not panic, even if the buffer is too small to render the line gauge.
        line_gauge.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[test]
    fn render_in_zero_size_buffer_gauge() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let gauge = Gauge::default().percent(50);
        // This should not panic, even if the buffer has zero size.
        gauge.render(buffer.area, &mut buffer);
    }

    #[test]
    fn render_in_zero_size_buffer_line_gauge() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let line_gauge = LineGauge::default().ratio(0.5);
        // This should not panic, even if the buffer has zero size.
        line_gauge.render(buffer.area, &mut buffer);
    }
}
