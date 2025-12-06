//! The [`Sparkline`] widget is used to display a sparkline over one or more lines.

use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cmp::min;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::{Style, Styled};
use ratatui_core::symbols;
use ratatui_core::widgets::Widget;
use strum::{Display, EnumString};

use crate::block::{Block, BlockExt};

/// Widget to render a sparkline over one or more lines.
///
/// Each bar in a `Sparkline` represents a value from the provided dataset. The height of the bar
/// is determined by the value in the dataset.
///
/// You can create a `Sparkline` using [`Sparkline::default`].
///
/// The data is set using [`Sparkline::data`]. The data can be a slice of `u64`, `Option<u64>`, or a
/// [`SparklineBar`].  For the `Option<u64>` and [`SparklineBar`] cases, a data point with a value
/// of `None` is interpreted an as the _absence_ of a value.
///
/// `Sparkline` can be styled either using [`Sparkline::style`] or preferably using the methods
/// provided by the [`Stylize`](ratatui_core::style::Stylize) trait.  The style may be set for the
/// entire widget or for individual bars by setting individual [`SparklineBar::style`].
///
/// The bars are rendered using a set of symbols. The default set is [`symbols::bar::NINE_LEVELS`].
/// You can change the set using [`Sparkline::bar_set`].
///
/// If the data provided is a slice of `u64` or `Option<u64>`, the bars will be styled with the
/// style of the sparkline. If the data is a slice of [`SparklineBar`], the bars will be
/// styled with the style of the sparkline combined with the style provided in the [`SparklineBar`]
/// if it is set, otherwise the sparkline style will be used.
///
/// Absent values and will be rendered with the style set by [`Sparkline::absent_value_style`] and
/// the symbol set by [`Sparkline::absent_value_symbol`].
///
/// # Setter methods
///
/// - [`Sparkline::block`] wraps the sparkline in a [`Block`]
/// - [`Sparkline::data`] defines the dataset, you'll almost always want to use it
/// - [`Sparkline::max`] sets the maximum value of bars
/// - [`Sparkline::direction`] sets the render direction
///
/// # Examples
///
/// ```
/// use ratatui::style::{Color, Style, Stylize};
/// use ratatui::symbols;
/// use ratatui::widgets::{Block, RenderDirection, Sparkline};
///
/// Sparkline::default()
///     .block(Block::bordered().title("Sparkline"))
///     .data(&[0, 2, 3, 4, 1, 4, 10])
///     .max(5)
///     .direction(RenderDirection::RightToLeft)
///     .style(Style::default().red().on_white())
///     .absent_value_style(Style::default().fg(Color::Red))
///     .absent_value_symbol(symbols::shade::FULL);
/// ```
#[derive(Debug, Default, Clone, Eq, PartialEq)]
pub struct Sparkline<'a> {
    /// A block to wrap the widget in
    block: Option<Block<'a>>,
    /// Widget style
    style: Style,
    /// Style of absent values
    absent_value_style: Style,
    /// The symbol to use for absent values
    absent_value_symbol: AbsentValueSymbol,
    /// A slice of the data to display
    data: Vec<SparklineBar>,
    /// The maximum value to take to compute the maximum bar height (if nothing is specified, the
    /// widget uses the max of the dataset)
    max: Option<u64>,
    /// A set of bar symbols used to represent the give data
    bar_set: symbols::bar::Set<'a>,
    /// The direction to render the sparkline, either from left to right, or from right to left
    direction: RenderDirection,
}

/// Defines the direction in which sparkline will be rendered.
///
/// See [`Sparkline::direction`].
#[derive(Debug, Default, Display, EnumString, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum RenderDirection {
    /// The first value is on the left, going to the right
    #[default]
    LeftToRight,
    /// The first value is on the right, going to the left
    RightToLeft,
}

impl<'a> Sparkline<'a> {
    /// Wraps the sparkline with the given `block`.
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
    /// The foreground corresponds to the bars while the background is everything else.
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }

    /// Sets the style to use for absent values.
    ///
    /// Absent values are values in the dataset that are `None`.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// The foreground corresponds to the bars while the background is everything else.
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn absent_value_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.absent_value_style = style.into();
        self
    }

    /// Sets the symbol to use for absent values.
    ///
    /// Absent values are values in the dataset that are `None`.
    ///
    /// The default is [`symbols::shade::EMPTY`].
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn absent_value_symbol(mut self, symbol: impl Into<String>) -> Self {
        self.absent_value_symbol = AbsentValueSymbol(symbol.into());
        self
    }

    /// Sets the dataset for the sparkline.
    ///
    /// Each item in the dataset is a bar in the sparkline. The height of the bar is determined by
    /// the value in the dataset.
    ///
    /// The data can be a slice of `u64`, `Option<u64>`, or a [`SparklineBar`].  For the
    /// `Option<u64>` and [`SparklineBar`] cases, a data point with a value of `None` is
    /// interpreted an as the _absence_ of a value.
    ///
    /// If the data provided is a slice of `u64` or `Option<u64>`, the bars will be styled with the
    /// style of the sparkline. If the data is a slice of [`SparklineBar`], the bars will be
    /// styled with the style of the sparkline combined with the style provided in the
    /// [`SparklineBar`] if it is set, otherwise the sparkline style will be used.
    ///
    /// Absent values and will be rendered with the style set by [`Sparkline::absent_value_style`]
    /// and the symbol set by [`Sparkline::absent_value_symbol`].
    ///
    /// # Examples
    ///
    /// Create a `Sparkline` from a slice of `u64`:
    ///
    /// ```
    /// use ratatui::Frame;
    /// use ratatui::layout::Rect;
    /// use ratatui::widgets::Sparkline;
    ///
    /// # fn ui(frame: &mut Frame) {
    /// # let area = Rect::default();
    /// let sparkline = Sparkline::default().data(&[1, 2, 3]);
    /// frame.render_widget(sparkline, area);
    /// # }
    /// ```
    ///
    /// Create a `Sparkline` from a slice of `Option<u64>` such that some bars are absent:
    ///
    /// ```
    /// # use ratatui::{prelude::*, widgets::*};
    /// # fn ui(frame: &mut Frame) {
    /// # let area = Rect::default();
    /// let data = vec![Some(1), None, Some(3)];
    /// let sparkline = Sparkline::default().data(data);
    /// frame.render_widget(sparkline, area);
    /// # }
    /// ```
    ///
    /// Create a [`Sparkline`] from a a Vec of [`SparklineBar`] such that some bars are styled:
    ///
    /// ```
    /// # use ratatui::{prelude::*, widgets::*};
    /// # fn ui(frame: &mut Frame) {
    /// # let area = Rect::default();
    /// let data = vec![
    ///     SparklineBar::from(1).style(Some(Style::default().fg(Color::Red))),
    ///     SparklineBar::from(2),
    ///     SparklineBar::from(3).style(Some(Style::default().fg(Color::Blue))),
    /// ];
    /// let sparkline = Sparkline::default().data(data);
    /// frame.render_widget(sparkline, area);
    /// # }
    /// ```
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn data<T>(mut self, data: T) -> Self
    where
        T: IntoIterator,
        T::Item: Into<SparklineBar>,
    {
        self.data = data.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the maximum value of bars.
    ///
    /// Every bar will be scaled accordingly. If no max is given, this will be the max in the
    /// dataset.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn max(mut self, max: u64) -> Self {
        self.max = Some(max);
        self
    }

    /// Sets the characters used to display the bars.
    ///
    /// Can be [`symbols::bar::THREE_LEVELS`], [`symbols::bar::NINE_LEVELS`] (default) or a custom
    /// [`Set`](symbols::bar::Set).
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn bar_set(mut self, bar_set: symbols::bar::Set<'a>) -> Self {
        self.bar_set = bar_set;
        self
    }

    /// Sets the direction of the sparkline.
    ///
    /// [`RenderDirection::LeftToRight`] by default.
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn direction(mut self, direction: RenderDirection) -> Self {
        self.direction = direction;
        self
    }
}

/// An bar in a `Sparkline`.
///
/// The height of the bar is determined by the value and a value of `None` is interpreted as the
/// _absence_ of a value, as distinct from a value of `Some(0)`.
#[derive(Debug, Default, Copy, Clone, Eq, PartialEq)]
pub struct SparklineBar {
    /// The value of the bar.
    ///
    /// If `None`, the bar is absent.
    value: Option<u64>,
    /// The style of the bar.
    ///
    /// If `None`, the bar will use the style of the sparkline.
    style: Option<Style>,
}

impl SparklineBar {
    /// Sets the style of the bar.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// If not set, the default style of the sparkline will be used.
    ///
    /// As well as the style of the sparkline, each [`SparklineBar`] may optionally set its own
    /// style.  If set, the style of the bar will be the style of the sparkline combined with
    /// the style of the bar.
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Option<Style>>>(mut self, style: S) -> Self {
        self.style = style.into();
        self
    }
}

impl From<Option<u64>> for SparklineBar {
    fn from(value: Option<u64>) -> Self {
        Self { value, style: None }
    }
}

impl From<u64> for SparklineBar {
    fn from(value: u64) -> Self {
        Self {
            value: Some(value),
            style: None,
        }
    }
}

impl From<&u64> for SparklineBar {
    fn from(value: &u64) -> Self {
        Self {
            value: Some(*value),
            style: None,
        }
    }
}

impl From<&Option<u64>> for SparklineBar {
    fn from(value: &Option<u64>) -> Self {
        Self {
            value: *value,
            style: None,
        }
    }
}

impl Styled for Sparkline<'_> {
    type Item = Self;

    fn style(&self) -> Style {
        self.style
    }

    fn set_style<S: Into<Style>>(self, style: S) -> Self::Item {
        self.style(style)
    }
}

impl Widget for Sparkline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl Widget for &Sparkline<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.block.as_ref().render(area, buf);
        let inner = self.block.inner_if_some(area);
        self.render_sparkline(inner, buf);
    }
}

/// A newtype wrapper for the symbol to use for absent values.
#[derive(Debug, Clone, Eq, PartialEq)]
struct AbsentValueSymbol(String);

impl Default for AbsentValueSymbol {
    fn default() -> Self {
        Self(symbols::shade::EMPTY.to_string())
    }
}

impl Sparkline<'_> {
    fn render_sparkline(&self, spark_area: Rect, buf: &mut Buffer) {
        if spark_area.is_empty() {
            return;
        }
        // determine the maximum height across all bars
        let max_height = self
            .max
            .unwrap_or_else(|| self.data.iter().filter_map(|s| s.value).max().unwrap_or(1));

        // determine the maximum index to render
        let max_index = min(spark_area.width as usize, self.data.len());

        // render each item in the data
        for (i, item) in self.data.iter().take(max_index).enumerate() {
            let x = match self.direction {
                RenderDirection::LeftToRight => spark_area.left() + i as u16,
                RenderDirection::RightToLeft => spark_area.right() - i as u16 - 1,
            };

            // determine the height, symbol and style to use for the item
            //
            // if the item is not absent:
            // - the height is the value of the item scaled to the height of the spark area
            // - the symbol is determined by the scaled height
            // - the style is the style of the item, if one is set
            //
            // otherwise:
            // - the height is the total height of the spark area
            // - the symbol is the absent value symbol
            // - the style is the absent value style
            let (mut height, symbol, style) = match item {
                SparklineBar {
                    value: Some(value),
                    style,
                } => {
                    let height = if max_height == 0 {
                        0
                    } else {
                        *value * u64::from(spark_area.height) * 8 / max_height
                    };
                    (height, None, *style)
                }
                _ => (
                    u64::from(spark_area.height) * 8,
                    Some(self.absent_value_symbol.0.as_str()),
                    Some(self.absent_value_style),
                ),
            };

            // render the item from top to bottom
            //
            // if the symbol is set it will be used for the entire height of the bar, otherwise the
            // symbol will be determined by the _remaining_ height.
            //
            // if the style is set it will be used for the entire height of the bar, otherwise the
            // sparkline style will be used.
            for j in (0..spark_area.height).rev() {
                let symbol = symbol.unwrap_or_else(|| self.symbol_for_height(height));
                if height > 8 {
                    height -= 8;
                } else {
                    height = 0;
                }
                buf[(x, spark_area.top() + j)]
                    .set_symbol(symbol)
                    .set_style(self.style.patch(style.unwrap_or_default()));
            }
        }
    }

    const fn symbol_for_height(&self, height: u64) -> &str {
        match height {
            0 => self.bar_set.empty,
            1 => self.bar_set.one_eighth,
            2 => self.bar_set.one_quarter,
            3 => self.bar_set.three_eighths,
            4 => self.bar_set.half,
            5 => self.bar_set.five_eighths,
            6 => self.bar_set.three_quarters,
            7 => self.bar_set.seven_eighths,
            _ => self.bar_set.full,
        }
    }
}

#[cfg(test)]
mod tests {
    use alloc::vec;

    use ratatui_core::buffer::Cell;
    use ratatui_core::style::{Color, Modifier, Stylize};
    use strum::ParseError;

    use super::*;

    #[test]
    fn render_direction_to_string() {
        assert_eq!(RenderDirection::LeftToRight.to_string(), "LeftToRight");
        assert_eq!(RenderDirection::RightToLeft.to_string(), "RightToLeft");
    }

    #[test]
    fn render_direction_from_str() {
        assert_eq!(
            "LeftToRight".parse::<RenderDirection>(),
            Ok(RenderDirection::LeftToRight)
        );
        assert_eq!(
            "RightToLeft".parse::<RenderDirection>(),
            Ok(RenderDirection::RightToLeft)
        );
        assert_eq!(
            "".parse::<RenderDirection>(),
            Err(ParseError::VariantNotFound)
        );
    }

    #[test]
    fn it_can_be_created_from_vec_of_u64() {
        let data = vec![1_u64, 2, 3];
        let spark_data = Sparkline::default().data(data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(2),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    #[test]
    fn it_can_be_created_from_vec_of_option_u64() {
        let data = vec![Some(1_u64), None, Some(3)];
        let spark_data = Sparkline::default().data(data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(None),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    #[test]
    fn it_can_be_created_from_array_of_u64() {
        let data = [1_u64, 2, 3];
        let spark_data = Sparkline::default().data(data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(2),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    #[test]
    fn it_can_be_created_from_array_of_option_u64() {
        let data = [Some(1_u64), None, Some(3)];
        let spark_data = Sparkline::default().data(data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(None),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    #[test]
    fn it_can_be_created_from_slice_of_u64() {
        let data = vec![1_u64, 2, 3];
        let spark_data = Sparkline::default().data(&data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(2),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    #[test]
    fn it_can_be_created_from_slice_of_option_u64() {
        let data = vec![Some(1_u64), None, Some(3)];
        let spark_data = Sparkline::default().data(&data).data;
        let expected = vec![
            SparklineBar::from(1),
            SparklineBar::from(None),
            SparklineBar::from(3),
        ];
        assert_eq!(spark_data, expected);
    }

    // Helper function to render a sparkline to a buffer with a given width
    // filled with x symbols to make it easier to assert on the result
    fn render(widget: Sparkline<'_>, width: u16) -> Buffer {
        let area = Rect::new(0, 0, width, 1);
        let mut buffer = Buffer::filled(area, Cell::new("x"));
        widget.render(area, &mut buffer);
        buffer
    }

    #[test]
    fn it_does_not_panic_if_max_is_zero() {
        let widget = Sparkline::default().data([0, 0, 0]);
        let buffer = render(widget, 6);
        assert_eq!(buffer, Buffer::with_lines(["   xxx"]));
    }

    #[test]
    fn it_does_not_panic_if_max_is_set_to_zero() {
        // see https://github.com/rust-lang/rust-clippy/issues/13191
        let widget = Sparkline::default().data([0, 1, 2]).max(0);
        let buffer = render(widget, 6);
        assert_eq!(buffer, Buffer::with_lines(["   xxx"]));
    }

    #[test]
    fn it_draws() {
        let widget = Sparkline::default().data([0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let buffer = render(widget, 12);
        assert_eq!(buffer, Buffer::with_lines([" ▁▂▃▄▅▆▇█xxx"]));
    }

    #[test]
    fn it_draws_double_height() {
        let widget = Sparkline::default().data([0, 1, 2, 3, 4, 5, 6, 7, 8]);
        let area = Rect::new(0, 0, 12, 2);
        let mut buffer = Buffer::filled(area, Cell::new("x"));
        widget.render(area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines(["     ▂▄▆█xxx", " ▂▄▆█████xxx"]));
    }

    #[test]
    fn it_renders_left_to_right() {
        let widget = Sparkline::default()
            .data([0, 1, 2, 3, 4, 5, 6, 7, 8])
            .direction(RenderDirection::LeftToRight);
        let buffer = render(widget, 12);
        assert_eq!(buffer, Buffer::with_lines([" ▁▂▃▄▅▆▇█xxx"]));
    }

    #[test]
    fn it_renders_right_to_left() {
        let widget = Sparkline::default()
            .data([0, 1, 2, 3, 4, 5, 6, 7, 8])
            .direction(RenderDirection::RightToLeft);
        let buffer = render(widget, 12);
        assert_eq!(buffer, Buffer::with_lines(["xxx█▇▆▅▄▃▂▁ "]));
    }

    #[test]
    fn it_renders_with_absent_value_style() {
        let widget = Sparkline::default()
            .absent_value_style(Style::default().fg(Color::Red))
            .absent_value_symbol(symbols::shade::FULL)
            .data([
                None,
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
            ]);
        let buffer = render(widget, 12);
        let mut expected = Buffer::with_lines(["█▁▂▃▄▅▆▇█xxx"]);
        expected.set_style(Rect::new(0, 0, 1, 1), Style::default().fg(Color::Red));
        assert_eq!(buffer, expected);
    }

    #[test]
    fn it_renders_with_absent_value_style_double_height() {
        let widget = Sparkline::default()
            .absent_value_style(Style::default().fg(Color::Red))
            .absent_value_symbol(symbols::shade::FULL)
            .data([
                None,
                Some(1),
                Some(2),
                Some(3),
                Some(4),
                Some(5),
                Some(6),
                Some(7),
                Some(8),
            ]);
        let area = Rect::new(0, 0, 12, 2);
        let mut buffer = Buffer::filled(area, Cell::new("x"));
        widget.render(area, &mut buffer);
        let mut expected = Buffer::with_lines(["█    ▂▄▆█xxx", "█▂▄▆█████xxx"]);
        expected.set_style(Rect::new(0, 0, 1, 2), Style::default().fg(Color::Red));
        assert_eq!(buffer, expected);
    }

    #[test]
    fn it_renders_with_custom_absent_value_style() {
        let widget = Sparkline::default().absent_value_symbol('*').data([
            None,
            Some(1),
            Some(2),
            Some(3),
            Some(4),
            Some(5),
            Some(6),
            Some(7),
            Some(8),
        ]);
        let buffer = render(widget, 12);
        let expected = Buffer::with_lines(["*▁▂▃▄▅▆▇█xxx"]);
        assert_eq!(buffer, expected);
    }

    #[test]
    fn it_renders_with_custom_bar_styles() {
        let widget = Sparkline::default().data(vec![
            SparklineBar::from(Some(0)).style(Some(Style::default().fg(Color::Red))),
            SparklineBar::from(Some(1)).style(Some(Style::default().fg(Color::Red))),
            SparklineBar::from(Some(2)).style(Some(Style::default().fg(Color::Red))),
            SparklineBar::from(Some(3)).style(Some(Style::default().fg(Color::Green))),
            SparklineBar::from(Some(4)).style(Some(Style::default().fg(Color::Green))),
            SparklineBar::from(Some(5)).style(Some(Style::default().fg(Color::Green))),
            SparklineBar::from(Some(6)).style(Some(Style::default().fg(Color::Blue))),
            SparklineBar::from(Some(7)).style(Some(Style::default().fg(Color::Blue))),
            SparklineBar::from(Some(8)).style(Some(Style::default().fg(Color::Blue))),
        ]);
        let buffer = render(widget, 12);
        let mut expected = Buffer::with_lines([" ▁▂▃▄▅▆▇█xxx"]);
        expected.set_style(Rect::new(0, 0, 3, 1), Style::default().fg(Color::Red));
        expected.set_style(Rect::new(3, 0, 3, 1), Style::default().fg(Color::Green));
        expected.set_style(Rect::new(6, 0, 3, 1), Style::default().fg(Color::Blue));
        assert_eq!(buffer, expected);
    }

    #[test]
    fn can_be_stylized() {
        assert_eq!(
            Sparkline::default()
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

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let sparkline = Sparkline::default()
            .data([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            .max(10);
        // This should not panic, even if the buffer is too small to render the sparkline.
        sparkline.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let sparkline = Sparkline::default()
            .data([1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            .max(10);
        // This should not panic, even if the buffer has zero size.
        sparkline.render(buffer.area, &mut buffer);
    }
}
