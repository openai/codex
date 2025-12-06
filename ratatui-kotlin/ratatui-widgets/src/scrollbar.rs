//! The [`Scrollbar`] widget is used to display a scrollbar alongside other widgets.
#![warn(clippy::pedantic)]
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::module_name_repetitions
)]

use core::iter;

use ratatui_core::buffer::Buffer;
use ratatui_core::layout::Rect;
use ratatui_core::style::Style;
use ratatui_core::symbols::scrollbar::{DOUBLE_HORIZONTAL, DOUBLE_VERTICAL, Set};
use ratatui_core::widgets::StatefulWidget;
use strum::{Display, EnumString};
use unicode_width::UnicodeWidthStr;

#[cfg(not(feature = "std"))]
use crate::polyfills::F64Polyfills;

/// A widget to display a scrollbar
///
/// The following components of the scrollbar are customizable in symbol and style. Note the
/// scrollbar is represented horizontally but it can also be set vertically (which is actually the
/// default).
///
/// ```text
/// <--▮------->
/// ^  ^   ^   ^
/// │  │   │   └ end
/// │  │   └──── track
/// │  └──────── thumb
/// └─────────── begin
/// ```
///
/// # Important
///
/// You must specify the [`ScrollbarState::content_length`] before rendering the `Scrollbar`, or
/// else the `Scrollbar` will render blank.
///
/// # Examples
///
/// ```rust
/// use ratatui::Frame;
/// use ratatui::layout::{Margin, Rect};
/// use ratatui::text::Line;
/// use ratatui::widgets::{
///     Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, StatefulWidget,
/// };
///
/// # fn render_paragraph_with_scrollbar(frame: &mut Frame, area: Rect) {
/// let vertical_scroll = 0; // from app state
///
/// let items = vec![
///     Line::from("Item 1"),
///     Line::from("Item 2"),
///     Line::from("Item 3"),
/// ];
/// let paragraph = Paragraph::new(items.clone())
///     .scroll((vertical_scroll as u16, 0))
///     .block(Block::new().borders(Borders::RIGHT)); // to show a background for the scrollbar
///
/// let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
///     .begin_symbol(Some("↑"))
///     .end_symbol(Some("↓"));
///
/// let mut scrollbar_state = ScrollbarState::new(items.len()).position(vertical_scroll);
///
/// let area = frame.area();
/// // Note we render the paragraph
/// frame.render_widget(paragraph, area);
/// // and the scrollbar, those are separate widgets
/// frame.render_stateful_widget(
///     scrollbar,
///     area.inner(Margin {
///         // using an inner vertical margin of 1 unit makes the scrollbar inside the block
///         vertical: 1,
///         horizontal: 0,
///     }),
///     &mut scrollbar_state,
/// );
/// # }
/// ```
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Scrollbar<'a> {
    orientation: ScrollbarOrientation,
    thumb_style: Style,
    thumb_symbol: &'a str,
    track_style: Style,
    track_symbol: Option<&'a str>,
    begin_symbol: Option<&'a str>,
    begin_style: Style,
    end_symbol: Option<&'a str>,
    end_style: Style,
}

/// This is the position of the scrollbar around a given area.
///
/// ```plain
///           HorizontalTop
///             ┌───────┐
/// VerticalLeft│       │VerticalRight
///             └───────┘
///          HorizontalBottom
/// ```
#[derive(Debug, Default, Display, EnumString, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ScrollbarOrientation {
    /// Positions the scrollbar on the right, scrolling vertically
    #[default]
    VerticalRight,
    /// Positions the scrollbar on the left, scrolling vertically
    VerticalLeft,
    /// Positions the scrollbar on the bottom, scrolling horizontally
    HorizontalBottom,
    /// Positions the scrollbar on the top, scrolling horizontally
    HorizontalTop,
}

/// A struct representing the state of a Scrollbar widget.
///
/// # Important
///
/// It's essential to set the `content_length` field when using this struct. This field
/// represents the total length of the scrollable content. The default value is zero
/// which will result in the Scrollbar not rendering.
///
/// For example, in the following list, assume there are 4 bullet points:
///
/// - the `content_length` is 4
/// - the `position` is 0
/// - the `viewport_content_length` is 2
///
/// ```text
/// ┌───────────────┐
/// │1. this is a   █
/// │   single item █
/// │2. this is a   ║
/// │   second item ║
/// └───────────────┘
/// ```
///
/// If you don't have multi-line content, you can leave the `viewport_content_length` set to the
/// default and it'll use the track size as a `viewport_content_length`.
#[derive(Debug, Default, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct ScrollbarState {
    /// The total length of the scrollable content.
    content_length: usize,
    /// The current position within the scrollable content.
    position: usize,
    /// The length of content in current viewport.
    ///
    /// FIXME: this should be `Option<usize>`, but it will break serialization to change it.
    viewport_content_length: usize,
}

/// An enum representing a scrolling direction.
///
/// This is used with [`ScrollbarState::scroll`].
///
/// It is useful for example when you want to store in which direction to scroll.
#[derive(Debug, Default, Display, EnumString, Clone, Copy, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum ScrollDirection {
    /// Forward scroll direction, usually corresponds to scrolling downwards or rightwards.
    #[default]
    Forward,
    /// Backward scroll direction, usually corresponds to scrolling upwards or leftwards.
    Backward,
}

impl Default for Scrollbar<'_> {
    fn default() -> Self {
        Self::new(ScrollbarOrientation::default())
    }
}

impl<'a> Scrollbar<'a> {
    /// Creates a new scrollbar with the given orientation.
    ///
    /// Most of the time you'll want [`ScrollbarOrientation::VerticalRight`] or
    /// [`ScrollbarOrientation::HorizontalBottom`]. See [`ScrollbarOrientation`] for more options.
    #[must_use = "creates the Scrollbar"]
    pub const fn new(orientation: ScrollbarOrientation) -> Self {
        let symbols = if orientation.is_vertical() {
            DOUBLE_VERTICAL
        } else {
            DOUBLE_HORIZONTAL
        };
        Self::new_with_symbols(orientation, &symbols)
    }

    /// Creates a new scrollbar with the given orientation and symbol set.
    #[must_use = "creates the Scrollbar"]
    const fn new_with_symbols(orientation: ScrollbarOrientation, symbols: &Set<'a>) -> Self {
        Self {
            orientation,
            thumb_symbol: symbols.thumb,
            thumb_style: Style::new(),
            track_symbol: Some(symbols.track),
            track_style: Style::new(),
            begin_symbol: Some(symbols.begin),
            begin_style: Style::new(),
            end_symbol: Some(symbols.end),
            end_style: Style::new(),
        }
    }

    /// Sets the position of the scrollbar.
    ///
    /// The orientation of the scrollbar is the position it will take around a [`Rect`]. See
    /// [`ScrollbarOrientation`] for more details.
    ///
    /// Resets the symbols to [`DOUBLE_VERTICAL`] or [`DOUBLE_HORIZONTAL`] based on orientation.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn orientation(mut self, orientation: ScrollbarOrientation) -> Self {
        self.orientation = orientation;
        let symbols = if self.orientation.is_vertical() {
            DOUBLE_VERTICAL
        } else {
            DOUBLE_HORIZONTAL
        };
        self.symbols(symbols)
    }

    /// Sets the orientation and symbols for the scrollbar from a [`Set`].
    ///
    /// This has the same effect as calling [`Scrollbar::orientation`] and then
    /// [`Scrollbar::symbols`]. See those for more details.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn orientation_and_symbol(
        mut self,
        orientation: ScrollbarOrientation,
        symbols: Set<'a>,
    ) -> Self {
        self.orientation = orientation;
        self.symbols(symbols)
    }

    /// Sets the symbol that represents the thumb of the scrollbar.
    ///
    /// The thumb is the handle representing the progression on the scrollbar. See [`Scrollbar`]
    /// for a visual example of what this represents.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn thumb_symbol(mut self, thumb_symbol: &'a str) -> Self {
        self.thumb_symbol = thumb_symbol;
        self
    }

    /// Sets the style on the scrollbar thumb.
    ///
    /// The thumb is the handle representing the progression on the scrollbar. See [`Scrollbar`]
    /// for a visual example of what this represents.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn thumb_style<S: Into<Style>>(mut self, thumb_style: S) -> Self {
        self.thumb_style = thumb_style.into();
        self
    }

    /// Sets the symbol that represents the track of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn track_symbol(mut self, track_symbol: Option<&'a str>) -> Self {
        self.track_symbol = track_symbol;
        self
    }

    /// Sets the style that is used for the track of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn track_style<S: Into<Style>>(mut self, track_style: S) -> Self {
        self.track_style = track_style.into();
        self
    }

    /// Sets the symbol that represents the beginning of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn begin_symbol(mut self, begin_symbol: Option<&'a str>) -> Self {
        self.begin_symbol = begin_symbol;
        self
    }

    /// Sets the style that is used for the beginning of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn begin_style<S: Into<Style>>(mut self, begin_style: S) -> Self {
        self.begin_style = begin_style.into();
        self
    }

    /// Sets the symbol that represents the end of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn end_symbol(mut self, end_symbol: Option<&'a str>) -> Self {
        self.end_symbol = end_symbol;
        self
    }

    /// Sets the style that is used for the end of the scrollbar.
    ///
    /// See [`Scrollbar`] for a visual example of what this represents.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn end_style<S: Into<Style>>(mut self, end_style: S) -> Self {
        self.end_style = end_style.into();
        self
    }

    /// Sets the symbols used for the various parts of the scrollbar from a [`Set`].
    ///
    /// ```text
    /// <--▮------->
    /// ^  ^   ^   ^
    /// │  │   │   └ end
    /// │  │   └──── track
    /// │  └──────── thumb
    /// └─────────── begin
    /// ```
    ///
    /// Only sets `begin_symbol`, `end_symbol` and `track_symbol` if they already contain a value.
    /// If they were set to `None` explicitly, this function will respect that choice. Use their
    /// respective setters to change their value.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[expect(clippy::needless_pass_by_value)] // Breaking change
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn symbols(mut self, symbols: Set<'a>) -> Self {
        self.thumb_symbol = symbols.thumb;
        if self.track_symbol.is_some() {
            self.track_symbol = Some(symbols.track);
        }
        if self.begin_symbol.is_some() {
            self.begin_symbol = Some(symbols.begin);
        }
        if self.end_symbol.is_some() {
            self.end_symbol = Some(symbols.end);
        }
        self
    }

    /// Sets the style used for the various parts of the scrollbar from a [`Style`].
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// ```text
    /// <--▮------->
    /// ^  ^   ^   ^
    /// │  │   │   └ end
    /// │  │   └──── track
    /// │  └──────── thumb
    /// └─────────── begin
    /// ```
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn style<S: Into<Style>>(mut self, style: S) -> Self {
        let style = style.into();
        self.track_style = style;
        self.thumb_style = style;
        self.begin_style = style;
        self.end_style = style;
        self
    }
}

impl ScrollbarState {
    /// Constructs a new [`ScrollbarState`] with the specified content length.
    ///
    /// `content_length` is the total number of element, that can be scrolled. See
    /// [`ScrollbarState`] for more details.
    #[must_use = "creates the ScrollbarState"]
    pub const fn new(content_length: usize) -> Self {
        Self {
            content_length,
            position: 0,
            viewport_content_length: 0,
        }
    }

    /// Sets the scroll position of the scrollbar.
    ///
    /// This represents the number of scrolled items.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn position(mut self, position: usize) -> Self {
        self.position = position;
        self
    }

    /// Sets the length of the scrollable content.
    ///
    /// This is the number of scrollable items. If items have a length of one, then this is the
    /// same as the number of scrollable cells.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn content_length(mut self, content_length: usize) -> Self {
        self.content_length = content_length;
        self
    }

    /// Sets the items' size.
    ///
    /// This is a fluent setter method which must be chained or used as it consumes self
    #[must_use = "method moves the value of self and returns the modified value"]
    pub const fn viewport_content_length(mut self, viewport_content_length: usize) -> Self {
        self.viewport_content_length = viewport_content_length;
        self
    }

    /// Decrements the scroll position by one, ensuring it doesn't go below zero.
    pub const fn prev(&mut self) {
        self.position = self.position.saturating_sub(1);
    }

    /// Increments the scroll position by one, ensuring it doesn't exceed the length of the content.
    pub fn next(&mut self) {
        self.position = self
            .position
            .saturating_add(1)
            .min(self.content_length.saturating_sub(1));
    }

    /// Sets the scroll position to the start of the scrollable content.
    pub const fn first(&mut self) {
        self.position = 0;
    }

    /// Sets the scroll position to the end of the scrollable content.
    pub const fn last(&mut self) {
        self.position = self.content_length.saturating_sub(1);
    }

    /// Changes the scroll position based on the provided [`ScrollDirection`].
    pub fn scroll(&mut self, direction: ScrollDirection) {
        match direction {
            ScrollDirection::Forward => {
                self.next();
            }
            ScrollDirection::Backward => {
                self.prev();
            }
        }
    }

    /// Returns the current position within the scrollable content.
    #[must_use = "returns the current position within the scrollable content"]
    pub const fn get_position(&self) -> usize {
        self.position
    }
}

impl StatefulWidget for Scrollbar<'_> {
    type State = ScrollbarState;

    fn render(self, area: Rect, buf: &mut Buffer, state: &mut Self::State) {
        if state.content_length == 0 || self.track_length_excluding_arrow_heads(area) == 0 {
            return;
        }

        if let Some(area) = self.scrollbar_area(area) {
            let areas = area.columns().flat_map(Rect::rows);
            let bar_symbols = self.bar_symbols(area, state);
            for (area, bar) in areas.zip(bar_symbols) {
                if let Some((symbol, style)) = bar {
                    buf.set_string(area.x, area.y, symbol, style);
                }
            }
        }
    }
}

impl Scrollbar<'_> {
    /// Returns an iterator over the symbols and styles of the scrollbar.
    fn bar_symbols(
        &self,
        area: Rect,
        state: &ScrollbarState,
    ) -> impl Iterator<Item = Option<(&str, Style)>> {
        let (track_start_len, thumb_len, track_end_len) = self.part_lengths(area, state);

        let begin = self.begin_symbol.map(|s| Some((s, self.begin_style)));
        let track = Some(self.track_symbol.map(|s| (s, self.track_style)));
        let thumb = Some(Some((self.thumb_symbol, self.thumb_style)));
        let end = self.end_symbol.map(|s| Some((s, self.end_style)));

        // `<`
        iter::once(begin)
            // `<═══`
            .chain(iter::repeat_n(track, track_start_len))
            // `<═══█████`
            .chain(iter::repeat_n(thumb, thumb_len))
            // `<═══█████═══════`
            .chain(iter::repeat_n(track, track_end_len))
            // `<═══█████═══════>`
            .chain(iter::once(end))
            .flatten()
    }

    /// Returns the lengths of the parts of a scrollbar
    ///
    /// The scrollbar has 3 parts of note:
    /// - `<═══█████═══════>`: full scrollbar
    /// - ` ═══             `: track start
    /// - `    █████        `: thumb
    /// - `         ═══════ `: track end
    ///
    /// This method returns the length of the start, thumb, and end as a tuple.
    fn part_lengths(&self, area: Rect, state: &ScrollbarState) -> (usize, usize, usize) {
        let track_length = f64::from(self.track_length_excluding_arrow_heads(area));
        let viewport_length = self.viewport_length(state, area) as f64;

        // Ensure that the position of the thumb is within the bounds of the content taking into
        // account the content and viewport length. When the last line of the content is at the top
        // of the viewport, the thumb should be at the bottom of the track.
        let max_position = state.content_length.saturating_sub(1) as f64;
        let start_position = (state.position as f64).clamp(0.0, max_position);
        let max_viewport_position = max_position + viewport_length;
        let end_position = start_position + viewport_length;

        // Calculate the start and end positions of the thumb. The size will be proportional to the
        // viewport length compared to the total amount of possible visible rows.
        let thumb_start = start_position * track_length / max_viewport_position;
        let thumb_end = end_position * track_length / max_viewport_position;

        // Make sure that the thumb is at least 1 cell long by ensuring that the start of the thumb
        // is less than the track_len. We use the positions instead of the sizes and use nearest
        // integer instead of floor / ceil to avoid problems caused by rounding errors.
        let thumb_start = thumb_start.round().clamp(0.0, track_length - 1.0) as usize;
        let thumb_end = thumb_end.round().clamp(0.0, track_length) as usize;

        let thumb_length = thumb_end.saturating_sub(thumb_start).max(1);
        let track_end_length = (track_length as usize).saturating_sub(thumb_start + thumb_length);

        (thumb_start, thumb_length, track_end_length)
    }

    fn scrollbar_area(&self, area: Rect) -> Option<Rect> {
        match self.orientation {
            ScrollbarOrientation::VerticalLeft => area.columns().next(),
            ScrollbarOrientation::VerticalRight => area.columns().next_back(),
            ScrollbarOrientation::HorizontalTop => area.rows().next(),
            ScrollbarOrientation::HorizontalBottom => area.rows().next_back(),
        }
    }

    /// Calculates length of the track excluding the arrow heads
    ///
    /// ```plain
    ///        ┌────────── track_length
    ///  vvvvvvvvvvvvvvv
    /// <═══█████═══════>
    /// ```
    fn track_length_excluding_arrow_heads(&self, area: Rect) -> u16 {
        let start_len = self.begin_symbol.map_or(0, |s| s.width() as u16);
        let end_len = self.end_symbol.map_or(0, |s| s.width() as u16);
        let arrows_len = start_len.saturating_add(end_len);
        if self.orientation.is_vertical() {
            area.height.saturating_sub(arrows_len)
        } else {
            area.width.saturating_sub(arrows_len)
        }
    }

    const fn viewport_length(&self, state: &ScrollbarState, area: Rect) -> usize {
        if state.viewport_content_length != 0 {
            state.viewport_content_length
        } else if self.orientation.is_vertical() {
            area.height as usize
        } else {
            area.width as usize
        }
    }
}

impl ScrollbarOrientation {
    /// Returns `true` if the scrollbar is vertical.
    #[must_use = "returns the requested kind of the scrollbar"]
    pub const fn is_vertical(&self) -> bool {
        matches!(self, Self::VerticalRight | Self::VerticalLeft)
    }

    /// Returns `true` if the scrollbar is horizontal.
    #[must_use = "returns the requested kind of the scrollbar"]
    pub const fn is_horizontal(&self) -> bool {
        matches!(self, Self::HorizontalBottom | Self::HorizontalTop)
    }
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::ToString;
    use core::str::FromStr;

    use ratatui_core::text::Text;
    use ratatui_core::widgets::Widget;
    use rstest::{fixture, rstest};
    use strum::ParseError;

    use super::*;

    #[test]
    fn scroll_direction_to_string() {
        assert_eq!(ScrollDirection::Forward.to_string(), "Forward");
        assert_eq!(ScrollDirection::Backward.to_string(), "Backward");
    }

    #[test]
    fn scroll_direction_from_str() {
        assert_eq!("Forward".parse(), Ok(ScrollDirection::Forward));
        assert_eq!("Backward".parse(), Ok(ScrollDirection::Backward));
        assert_eq!(
            ScrollDirection::from_str(""),
            Err(ParseError::VariantNotFound)
        );
    }

    #[test]
    fn scrollbar_orientation_to_string() {
        use ScrollbarOrientation::*;
        assert_eq!(VerticalRight.to_string(), "VerticalRight");
        assert_eq!(VerticalLeft.to_string(), "VerticalLeft");
        assert_eq!(HorizontalBottom.to_string(), "HorizontalBottom");
        assert_eq!(HorizontalTop.to_string(), "HorizontalTop");
    }

    #[test]
    fn scrollbar_orientation_from_str() {
        use ScrollbarOrientation::*;
        assert_eq!("VerticalRight".parse(), Ok(VerticalRight));
        assert_eq!("VerticalLeft".parse(), Ok(VerticalLeft));
        assert_eq!("HorizontalBottom".parse(), Ok(HorizontalBottom));
        assert_eq!("HorizontalTop".parse(), Ok(HorizontalTop));
        assert_eq!(
            ScrollbarOrientation::from_str(""),
            Err(ParseError::VariantNotFound)
        );
    }

    #[fixture]
    fn scrollbar_no_arrows() -> Scrollbar<'static> {
        Scrollbar::new(ScrollbarOrientation::HorizontalTop)
            .begin_symbol(None)
            .end_symbol(None)
            .track_symbol(Some("-"))
            .thumb_symbol("#")
    }

    #[rstest]
    #[case::area_2_position_0("#-", 0, 2)]
    #[case::area_2_position_1("-#", 1, 2)]
    fn render_scrollbar_simplest(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, expected.width() as u16, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("#####-----", 0, 10)]
    #[case::position_1("-#####----", 1, 10)]
    #[case::position_2("-#####----", 2, 10)]
    #[case::position_3("--#####---", 3, 10)]
    #[case::position_4("--#####---", 4, 10)]
    #[case::position_5("---#####--", 5, 10)]
    #[case::position_6("---#####--", 6, 10)]
    #[case::position_7("----#####-", 7, 10)]
    #[case::position_8("----#####-", 8, 10)]
    #[case::position_9("-----#####", 9, 10)]
    fn render_scrollbar_simple(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, expected.width() as u16, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("          ", 0, 0)]
    fn render_scrollbar_nobar(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let size = expected.width();
        let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::fullbar_position_0("##########", 0, 1)]
    #[case::almost_fullbar_position_0("#########-", 0, 2)]
    #[case::almost_fullbar_position_1("-#########", 1, 2)]
    fn render_scrollbar_fullbar(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let size = expected.width();
        let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("#########-", 0, 2)]
    #[case::position_1("-#########", 1, 2)]
    fn render_scrollbar_almost_fullbar(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let size = expected.width();
        let mut buffer = Buffer::empty(Rect::new(0, 0, size as u16, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("█████═════", 0, 10)]
    #[case::position_1("═█████════", 1, 10)]
    #[case::position_2("═█████════", 2, 10)]
    #[case::position_3("══█████═══", 3, 10)]
    #[case::position_4("══█████═══", 4, 10)]
    #[case::position_5("═══█████══", 5, 10)]
    #[case::position_6("═══█████══", 6, 10)]
    #[case::position_7("════█████═", 7, 10)]
    #[case::position_8("════█████═", 8, 10)]
    #[case::position_9("═════█████", 9, 10)]
    #[case::position_out_of_bounds("═════█████", 100, 10)]
    fn render_scrollbar_without_symbols(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(None)
            .end_symbol(None)
            .render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("█████     ", 0, 10)]
    #[case::position_1(" █████    ", 1, 10)]
    #[case::position_2(" █████    ", 2, 10)]
    #[case::position_3("  █████   ", 3, 10)]
    #[case::position_4("  █████   ", 4, 10)]
    #[case::position_5("   █████  ", 5, 10)]
    #[case::position_6("   █████  ", 6, 10)]
    #[case::position_7("    █████ ", 7, 10)]
    #[case::position_8("    █████ ", 8, 10)]
    #[case::position_9("     █████", 9, 10)]
    #[case::position_out_of_bounds("     █████", 100, 10)]
    fn render_scrollbar_without_track_symbols(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .track_symbol(None)
            .begin_symbol(None)
            .end_symbol(None)
            .render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("█████-----", 0, 10)]
    #[case::position_1("-█████----", 1, 10)]
    #[case::position_2("-█████----", 2, 10)]
    #[case::position_3("--█████---", 3, 10)]
    #[case::position_4("--█████---", 4, 10)]
    #[case::position_5("---█████--", 5, 10)]
    #[case::position_6("---█████--", 6, 10)]
    #[case::position_7("----█████-", 7, 10)]
    #[case::position_8("----█████-", 8, 10)]
    #[case::position_9("-----█████", 9, 10)]
    #[case::position_out_of_bounds("-----█████", 100, 10)]
    fn render_scrollbar_without_track_symbols_over_content(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let width = buffer.area.width as usize;
        let s = "";
        Text::from(format!("{s:-^width$}")).render(buffer.area, &mut buffer);
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .track_symbol(None)
            .begin_symbol(None)
            .end_symbol(None)
            .render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("<####---->", 0, 10)]
    #[case::position_1("<#####--->", 1, 10)]
    #[case::position_2("<-####--->", 2, 10)]
    #[case::position_3("<-####--->", 3, 10)]
    #[case::position_4("<--####-->", 4, 10)]
    #[case::position_5("<--####-->", 5, 10)]
    #[case::position_6("<---####->", 6, 10)]
    #[case::position_7("<---####->", 7, 10)]
    #[case::position_8("<---#####>", 8, 10)]
    #[case::position_9("<----####>", 9, 10)]
    #[case::position_one_out_of_bounds("<----####>", 10, 10)]
    #[case::position_few_out_of_bounds("<----####>", 15, 10)]
    #[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
    fn render_scrollbar_with_symbols(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalTop)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#")
            .render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::position_0("█████═════", 0, 10)]
    #[case::position_1("═█████════", 1, 10)]
    #[case::position_2("═█████════", 2, 10)]
    #[case::position_3("══█████═══", 3, 10)]
    #[case::position_4("══█████═══", 4, 10)]
    #[case::position_5("═══█████══", 5, 10)]
    #[case::position_6("═══█████══", 6, 10)]
    #[case::position_7("════█████═", 7, 10)]
    #[case::position_8("════█████═", 8, 10)]
    #[case::position_9("═════█████", 9, 10)]
    #[case::position_out_of_bounds("═════█████", 100, 10)]
    fn render_scrollbar_horizontal_bottom(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 2));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalBottom)
            .begin_symbol(None)
            .end_symbol(None)
            .render(buffer.area, &mut buffer, &mut state);
        let empty_string = " ".repeat(size as usize);
        assert_eq!(buffer, Buffer::with_lines([&empty_string, expected]));
    }

    #[rstest]
    #[case::position_0("█████═════", 0, 10)]
    #[case::position_1("═█████════", 1, 10)]
    #[case::position_2("═█████════", 2, 10)]
    #[case::position_3("══█████═══", 3, 10)]
    #[case::position_4("══█████═══", 4, 10)]
    #[case::position_5("═══█████══", 5, 10)]
    #[case::position_6("═══█████══", 6, 10)]
    #[case::position_7("════█████═", 7, 10)]
    #[case::position_8("════█████═", 8, 10)]
    #[case::position_9("═════█████", 9, 10)]
    #[case::position_out_of_bounds("═════█████", 100, 10)]
    fn render_scrollbar_horizontal_top(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 2));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::HorizontalTop)
            .begin_symbol(None)
            .end_symbol(None)
            .render(buffer.area, &mut buffer, &mut state);
        let empty_string = " ".repeat(size as usize);
        assert_eq!(buffer, Buffer::with_lines([expected, &empty_string]));
    }

    #[rstest]
    #[case::position_0("<####---->", 0, 10)]
    #[case::position_1("<#####--->", 1, 10)]
    #[case::position_2("<-####--->", 2, 10)]
    #[case::position_3("<-####--->", 3, 10)]
    #[case::position_4("<--####-->", 4, 10)]
    #[case::position_5("<--####-->", 5, 10)]
    #[case::position_6("<---####->", 6, 10)]
    #[case::position_7("<---####->", 7, 10)]
    #[case::position_8("<---#####>", 8, 10)]
    #[case::position_9("<----####>", 9, 10)]
    #[case::position_one_out_of_bounds("<----####>", 10, 10)]
    #[case::position_few_out_of_bounds("<----####>", 15, 10)]
    #[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
    fn render_scrollbar_vertical_left(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, 5, size));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::VerticalLeft)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#")
            .render(buffer.area, &mut buffer, &mut state);
        let bar = expected.chars().map(|c| format!("{c}    "));
        assert_eq!(buffer, Buffer::with_lines(bar));
    }

    #[rstest]
    #[case::position_0("<####---->", 0, 10)]
    #[case::position_1("<#####--->", 1, 10)]
    #[case::position_2("<-####--->", 2, 10)]
    #[case::position_3("<-####--->", 3, 10)]
    #[case::position_4("<--####-->", 4, 10)]
    #[case::position_5("<--####-->", 5, 10)]
    #[case::position_6("<---####->", 6, 10)]
    #[case::position_7("<---####->", 7, 10)]
    #[case::position_8("<---#####>", 8, 10)]
    #[case::position_9("<----####>", 9, 10)]
    #[case::position_one_out_of_bounds("<----####>", 10, 10)]
    #[case::position_few_out_of_bounds("<----####>", 15, 10)]
    #[case::position_very_many_out_of_bounds("<----####>", 500, 10)]
    fn render_scrollbar_vertical_right(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, 5, size));
        let mut state = ScrollbarState::new(content_length).position(position);
        Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#")
            .render(buffer.area, &mut buffer, &mut state);
        let bar = expected.chars().map(|c| format!("    {c}"));
        assert_eq!(buffer, Buffer::with_lines(bar));
    }

    #[rstest]
    #[case::position_0("##--------", 0, 10)]
    #[case::position_1("-##-------", 1, 10)]
    #[case::position_2("--##------", 2, 10)]
    #[case::position_3("---##-----", 3, 10)]
    #[case::position_4("----#-----", 4, 10)]
    #[case::position_5("-----#----", 5, 10)]
    #[case::position_6("-----##---", 6, 10)]
    #[case::position_7("------##--", 7, 10)]
    #[case::position_8("-------##-", 8, 10)]
    #[case::position_9("--------##", 9, 10)]
    #[case::position_one_out_of_bounds("--------##", 10, 10)]
    fn custom_viewport_length(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let mut state = ScrollbarState::new(content_length)
            .position(position)
            .viewport_content_length(2);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    /// Fixes <https://github.com/ratatui/ratatui/pull/959> which was a bug that would not
    /// render a thumb when the viewport was very small in comparison to the content length.
    #[rstest]
    #[case::position_0("#----", 0, 100)]
    #[case::position_10("#----", 10, 100)]
    #[case::position_20("-#---", 20, 100)]
    #[case::position_30("-#---", 30, 100)]
    #[case::position_40("--#--", 40, 100)]
    #[case::position_50("--#--", 50, 100)]
    #[case::position_60("---#-", 60, 100)]
    #[case::position_70("---#-", 70, 100)]
    #[case::position_80("----#", 80, 100)]
    #[case::position_90("----#", 90, 100)]
    #[case::position_one_out_of_bounds("----#", 100, 100)]
    fn thumb_visible_on_very_small_track(
        #[case] expected: &str,
        #[case] position: usize,
        #[case] content_length: usize,
        scrollbar_no_arrows: Scrollbar,
    ) {
        let size = expected.width() as u16;
        let mut buffer = Buffer::empty(Rect::new(0, 0, size, 1));
        let mut state = ScrollbarState::new(content_length)
            .position(position)
            .viewport_content_length(2);
        scrollbar_no_arrows.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([expected]));
    }

    #[rstest]
    #[case::scrollbar_height_0(10, 0)]
    #[case::scrollbar_width_0(0, 10)]
    fn do_not_render_with_empty_area(#[case] width: u16, #[case] height: u16) {
        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("<"))
            .end_symbol(Some(">"))
            .track_symbol(Some("-"))
            .thumb_symbol("#");
        let zero_width_area = Rect::new(0, 0, width, height);
        let mut buffer = Buffer::empty(Rect::new(0, 0, 10, 10));

        let mut state = ScrollbarState::new(10);
        scrollbar.render(zero_width_area, &mut buffer, &mut state);
    }

    #[rstest]
    #[case::vertical_left(ScrollbarOrientation::VerticalLeft)]
    #[case::vertical_right(ScrollbarOrientation::VerticalRight)]
    #[case::horizontal_top(ScrollbarOrientation::HorizontalTop)]
    #[case::horizontal_bottom(ScrollbarOrientation::HorizontalBottom)]
    fn render_in_minimal_buffer(#[case] orientation: ScrollbarOrientation) {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let scrollbar = Scrollbar::new(orientation);
        let mut state = ScrollbarState::new(10).position(5);
        // This should not panic, even if the buffer is too small to render the scrollbar.
        scrollbar.render(buffer.area, &mut buffer, &mut state);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[rstest]
    #[case::vertical_left(ScrollbarOrientation::VerticalLeft)]
    #[case::vertical_right(ScrollbarOrientation::VerticalRight)]
    #[case::horizontal_top(ScrollbarOrientation::HorizontalTop)]
    #[case::horizontal_bottom(ScrollbarOrientation::HorizontalBottom)]
    fn render_in_zero_size_buffer(#[case] orientation: ScrollbarOrientation) {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let scrollbar = Scrollbar::new(orientation);
        let mut state = ScrollbarState::new(10).position(5);
        // This should not panic, even if the buffer has zero size.
        scrollbar.render(buffer.area, &mut buffer, &mut state);
    }
}
