//! A simple calendar widget. `(feature: widget-calendar)`
//!
//!
//!
//! The [`Monthly`] widget will display a calendar for the month provided in `display_date`. Days
//! are styled using the default style unless:
//! * `show_surrounding` is set, then days not in the `display_date` month will use that style.
//! * a style is returned by the [`DateStyler`] for the day
//!
//! [`Monthly`] has several controls for what should be displayed
use alloc::format;
use alloc::vec::Vec;

use hashbrown::HashMap;
use ratatui_core::buffer::Buffer;
use ratatui_core::layout::{Alignment, Constraint, Layout, Rect};
use ratatui_core::style::Style;
use ratatui_core::text::{Line, Span};
use ratatui_core::widgets::Widget;
use time::{Date, Duration};

use crate::block::{Block, BlockExt};

/// Display a month calendar for the month containing `display_date`
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct Monthly<'a, DS: DateStyler> {
    display_date: Date,
    events: DS,
    show_surrounding: Option<Style>,
    show_weekday: Option<Style>,
    show_month: Option<Style>,
    default_style: Style,
    block: Option<Block<'a>>,
}

impl<'a, DS: DateStyler> Monthly<'a, DS> {
    /// Construct a calendar for the `display_date` and highlight the `events`
    pub const fn new(display_date: Date, events: DS) -> Self {
        Self {
            display_date,
            events,
            show_surrounding: None,
            show_weekday: None,
            show_month: None,
            default_style: Style::new(),
            block: None,
        }
    }

    /// Fill the calendar slots for days not in the current month also, this causes each line to be
    /// completely filled. If there is an event style for a date, this style will be patched with
    /// the event's style
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_surrounding<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_surrounding = Some(style.into());
        self
    }

    /// Display a header containing weekday abbreviations
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_weekdays_header<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_weekday = Some(style.into());
        self
    }

    /// Display a header containing the month and year
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn show_month_header<S: Into<Style>>(mut self, style: S) -> Self {
        self.show_month = Some(style.into());
        self
    }

    /// How to render otherwise unstyled dates
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn default_style<S: Into<Style>>(mut self, style: S) -> Self {
        self.default_style = style.into();
        self
    }

    /// Render the calendar within a [Block]
    #[must_use = "method moves the value of self and returns the modified value"]
    pub fn block(mut self, block: Block<'a>) -> Self {
        self.block = Some(block);
        self
    }

    /// Return the width required to render the calendar.
    #[must_use]
    pub fn width(&self) -> u16 {
        const DAYS_PER_WEEK: u16 = 7;
        const GUTTER_WIDTH: u16 = 1;
        const DAY_WIDTH: u16 = 2;

        let mut width = DAYS_PER_WEEK * (GUTTER_WIDTH + DAY_WIDTH);
        if let Some(block) = &self.block {
            let (left, right) = block.horizontal_space();
            width = width.saturating_add(left).saturating_add(right);
        }
        width
    }

    /// Return the height required to render the calendar.
    #[must_use]
    pub fn height(&self) -> u16 {
        let mut height = u16::from(sunday_based_weeks(self.display_date))
            .saturating_add(u16::from(self.show_month.is_some()))
            .saturating_add(u16::from(self.show_weekday.is_some()));

        if let Some(block) = &self.block {
            let (top, bottom) = block.vertical_space();
            height = height.saturating_add(top).saturating_add(bottom);
        }

        height
    }

    /// Return a style with only the background from the default style
    const fn default_bg(&self) -> Style {
        match self.default_style.bg {
            None => Style::new(),
            Some(c) => Style::new().bg(c),
        }
    }

    /// All logic to style a date goes here.
    fn format_date(&self, date: Date) -> Span<'_> {
        if date.month() == self.display_date.month() {
            Span::styled(
                format!("{:2?}", date.day()),
                self.default_style.patch(self.events.get_style(date)),
            )
        } else {
            match self.show_surrounding {
                None => Span::styled("  ", self.default_bg()),
                Some(s) => {
                    let style = self
                        .default_style
                        .patch(s)
                        .patch(self.events.get_style(date));
                    Span::styled(format!("{:2?}", date.day()), style)
                }
            }
        }
    }
}

impl<DS: DateStyler> Widget for Monthly<'_, DS> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Widget::render(&self, area, buf);
    }
}

impl<DS: DateStyler> Widget for &Monthly<'_, DS> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        self.block.as_ref().render(area, buf);
        let inner = self.block.inner_if_some(area);
        self.render_monthly(inner, buf);
    }
}

impl<DS: DateStyler> Monthly<'_, DS> {
    fn render_monthly(&self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::vertical([
            Constraint::Length(self.show_month.is_some().into()),
            Constraint::Length(self.show_weekday.is_some().into()),
            Constraint::Fill(1),
        ]);
        let [month_header, days_header, days_area] = layout.areas(area);

        // Draw the month name and year
        if let Some(style) = self.show_month {
            Line::styled(
                format!("{} {}", self.display_date.month(), self.display_date.year()),
                style,
            )
            .alignment(Alignment::Center)
            .render(month_header, buf);
        }

        // Draw days of week
        if let Some(style) = self.show_weekday {
            Span::styled(" Su Mo Tu We Th Fr Sa", style).render(days_header, buf);
        }

        // Set the start of the calendar to the Sunday before the 1st (or the sunday of the first)
        let first_of_month = self.display_date.replace_day(1).unwrap();
        let offset = Duration::days(first_of_month.weekday().number_days_from_sunday().into());
        let mut curr_day = first_of_month - offset;

        let mut y = days_area.y;
        // go through all the weeks containing a day in the target month.
        while curr_day.month() != self.display_date.month().next() {
            let mut spans = Vec::with_capacity(14);
            for i in 0..7 {
                // Draw the gutter. Do it here so we can avoid worrying about
                // styling the ' ' in the format_date method
                if i == 0 {
                    spans.push(Span::styled(" ", Style::default()));
                } else {
                    spans.push(Span::styled(" ", self.default_bg()));
                }
                spans.push(self.format_date(curr_day));
                curr_day += Duration::DAY;
            }
            if buf.area.height > y {
                buf.set_line(days_area.x, y, &spans.into(), area.width);
            }
            y += 1;
        }
    }
}

/// Compute how many Sunday-based week rows are needed to render `display_date`.
///
/// Mirrors the rendering logic by taking the difference between the first and last day
/// Sunday-based week numbers (inclusive).
fn sunday_based_weeks(display_date: Date) -> u8 {
    let first_of_month = display_date
        .replace_day(1)
        .expect("valid first day of month");
    let last_of_month = first_of_month
        .replace_day(first_of_month.month().length(first_of_month.year()))
        .expect("valid last of month");
    let first_week = first_of_month.sunday_based_week();
    let last_week = last_of_month.sunday_based_week();
    last_week.saturating_sub(first_week) + 1
}

/// Provides a method for styling a given date. [Monthly] is generic on this trait, so any type
/// that implements this trait can be used.
pub trait DateStyler {
    /// Given a date, return a style for that date
    fn get_style(&self, date: Date) -> Style;
}

/// A simple `DateStyler` based on a [`HashMap`]
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct CalendarEventStore(pub HashMap<Date, Style>);

impl CalendarEventStore {
    /// Construct a store that has the current date styled.
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    #[cfg(feature = "std")]
    pub fn today<S: Into<Style>>(style: S) -> Self {
        use time::OffsetDateTime;
        let mut res = Self::default();
        res.add(
            OffsetDateTime::now_local()
                .unwrap_or_else(|_| OffsetDateTime::now_utc())
                .date(),
            style.into(),
        );
        res
    }

    /// Add a date and style to the store
    ///
    /// `style` accepts any type that is convertible to [`Style`] (e.g. [`Style`], [`Color`], or
    /// your own type that implements [`Into<Style>`]).
    ///
    /// [`Color`]: ratatui_core::style::Color
    pub fn add<S: Into<Style>>(&mut self, date: Date, style: S) {
        // to simplify style nonsense, last write wins
        let _ = self.0.insert(date, style.into());
    }

    /// Helper for trait impls
    fn lookup_style(&self, date: Date) -> Style {
        self.0.get(&date).copied().unwrap_or_default()
    }
}

impl DateStyler for CalendarEventStore {
    fn get_style(&self, date: Date) -> Style {
        self.lookup_style(date)
    }
}

impl DateStyler for &CalendarEventStore {
    fn get_style(&self, date: Date) -> Style {
        self.lookup_style(date)
    }
}

impl Default for CalendarEventStore {
    fn default() -> Self {
        Self(HashMap::with_capacity(4))
    }
}

#[cfg(test)]
mod tests {
    use ratatui_core::style::{Color, Style};
    use time::Month;

    use super::*;
    use crate::block::{Block, Padding};

    #[test]
    fn event_store() {
        let a = (
            Date::from_calendar_date(2023, Month::January, 1).unwrap(),
            Style::default(),
        );
        let b = (
            Date::from_calendar_date(2023, Month::January, 2).unwrap(),
            Style::default().bg(Color::Red).fg(Color::Blue),
        );
        let mut s = CalendarEventStore::default();
        s.add(b.0, b.1);

        assert_eq!(
            s.get_style(a.0),
            a.1,
            "Date not added to the styler should look up as Style::default()"
        );
        assert_eq!(
            s.get_style(b.0),
            b.1,
            "Date added to styler should return the provided style"
        );
    }

    #[test]
    fn test_today() {
        CalendarEventStore::today(Style::default());
    }

    #[test]
    fn render_in_minimal_buffer() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 1, 1));
        let calendar = Monthly::new(
            Date::from_calendar_date(1984, Month::January, 1).unwrap(),
            CalendarEventStore::default(),
        );
        // This should not panic, even if the buffer is too small to render the calendar.
        calendar.render(buffer.area, &mut buffer);
        assert_eq!(buffer, Buffer::with_lines([" "]));
    }

    #[test]
    fn render_in_zero_size_buffer() {
        let mut buffer = Buffer::empty(Rect::ZERO);
        let calendar = Monthly::new(
            Date::from_calendar_date(1984, Month::January, 1).unwrap(),
            CalendarEventStore::default(),
        );
        // This should not panic, even if the buffer has zero size.
        calendar.render(buffer.area, &mut buffer);
    }

    #[test]
    fn calendar_width_reflects_grid_layout() {
        let date = Date::from_calendar_date(2023, Month::January, 1).unwrap();
        let calendar = Monthly::new(date, CalendarEventStore::default());
        assert_eq!(calendar.width(), 21);
    }

    #[test]
    fn calendar_height_counts_weeks_and_headers() {
        let date = Date::from_calendar_date(2015, Month::February, 1).unwrap();
        let base_calendar = Monthly::new(date, CalendarEventStore::default());
        assert_eq!(base_calendar.height(), 4);

        let decorated_calendar = Monthly::new(date, CalendarEventStore::default())
            .show_month_header(Style::default())
            .show_weekdays_header(Style::default());
        assert_eq!(decorated_calendar.height(), 6);
    }

    #[test]
    fn calendar_dimensions_examples() {
        // Feb 2015 starts Sunday and spans 4 rows.
        let feb_2015 = Date::from_calendar_date(2015, Month::February, 1).unwrap();
        let cal = Monthly::new(feb_2015, CalendarEventStore::default());
        assert_eq!(cal.width(), 21, "4w base width");
        assert_eq!(cal.height(), 4, "Feb 2015 rows");

        let cal = Monthly::new(feb_2015, CalendarEventStore::default())
            .show_month_header(Style::default())
            .show_weekdays_header(Style::default());
        assert_eq!(cal.height(), 6, "Headers add 2 rows");

        let block = Block::bordered().padding(Padding::new(2, 3, 1, 2));
        let cal = Monthly::new(feb_2015, CalendarEventStore::default()).block(block);
        assert_eq!(cal.width(), 28, "Padding widens width");
        assert_eq!(cal.height(), 9, "Padding grows height");

        // Feb 2024 starts Thursday and spans 5 rows.
        let feb_2024 = Date::from_calendar_date(2024, Month::February, 1).unwrap();
        let cal = Monthly::new(feb_2024, CalendarEventStore::default());
        assert_eq!(cal.width(), 21, "5w base width");
        assert_eq!(cal.height(), 5, "Feb 2024 rows");

        let cal = Monthly::new(feb_2024, CalendarEventStore::default())
            .show_month_header(Style::default())
            .show_weekdays_header(Style::default());
        assert_eq!(cal.height(), 7, "Headers add 2 rows (5w)");

        let cal = Monthly::new(feb_2024, CalendarEventStore::default()).block(Block::bordered());
        assert_eq!(cal.width(), 23, "Border adds 2 cols");
        assert_eq!(cal.height(), 7, "Border adds 2 rows");

        // Apr 2023 starts Saturday and spans 6 rows.
        let apr_2023 = Date::from_calendar_date(2023, Month::April, 1).unwrap();
        let cal = Monthly::new(apr_2023, CalendarEventStore::default());
        assert_eq!(cal.width(), 21, "6w base width");
        assert_eq!(cal.height(), 6, "Apr 2023 rows");

        let cal = Monthly::new(apr_2023, CalendarEventStore::default())
            .show_month_header(Style::default())
            .show_weekdays_header(Style::default());
        assert_eq!(cal.height(), 8, "Headers add 2 rows (6w)");

        let block = Block::bordered().padding(Padding::symmetric(1, 1));
        let cal = Monthly::new(apr_2023, CalendarEventStore::default()).block(block);
        assert_eq!(cal.width(), 25, "Symmetric padding width");
        assert_eq!(cal.height(), 10, "Symmetric padding height");
    }

    #[test]
    fn sunday_based_weeks_shapes() {
        let sunday_start =
            Date::from_calendar_date(2015, Month::February, 11).expect("valid test date");
        let saturday_start =
            Date::from_calendar_date(2023, Month::April, 9).expect("valid test date");
        let leap_year =
            Date::from_calendar_date(2024, Month::February, 29).expect("valid test date");

        assert_eq!(sunday_based_weeks(sunday_start), 4);
        assert_eq!(sunday_based_weeks(saturday_start), 6);
        assert_eq!(sunday_based_weeks(leap_year), 5);
    }
}
