//! A Ratatui example that demonstrates how to render calendar with different styles.
//!
//! Marks the holidays and seasons on the calendar.
//!
//! This example runs with the Ratatui library code in the branch that you are currently reading.
//! See the [`latest`] branch for the code which works with the most recent Ratatui release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
//! [`BarChart`]: https://docs.rs/ratatui/latest/ratatui/widgets/struct.BarChart.html

use std::fmt;

use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::layout::{Constraint, Layout, Margin, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use ratatui::{DefaultTerminal, Frame};
use time::ext::NumericalDuration;
use time::{Date, Month, OffsetDateTime};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(run)
}

/// Run the application.
fn run(terminal: &mut DefaultTerminal) -> Result<()> {
    let mut selected_date = OffsetDateTime::now_local()?.date();
    let mut calendar_style = StyledCalendar::Default;
    loop {
        terminal.draw(|frame| render(frame, calendar_style, selected_date))?;
        if let Some(key) = event::read()?.as_key_press_event() {
            match key.code {
                KeyCode::Char('q') => break Ok(()),
                KeyCode::Char('s') => calendar_style = calendar_style.next(),
                KeyCode::Char('n') | KeyCode::Tab => selected_date = next_month(selected_date),
                KeyCode::Char('p') | KeyCode::BackTab => selected_date = prev_month(selected_date),
                KeyCode::Char('h') | KeyCode::Left => selected_date -= 1.days(),
                KeyCode::Char('j') | KeyCode::Down => selected_date += 1.weeks(),
                KeyCode::Char('k') | KeyCode::Up => selected_date -= 1.weeks(),
                KeyCode::Char('l') | KeyCode::Right => selected_date += 1.days(),
                _ => {}
            }
        }
    }
}

fn next_month(date: Date) -> Date {
    if date.month() == Month::December {
        date.replace_month(Month::January)
            .unwrap()
            .replace_year(date.year() + 1)
            .unwrap()
    } else {
        date.replace_month(date.month().next()).unwrap()
    }
}

fn prev_month(date: Date) -> Date {
    if date.month() == Month::January {
        date.replace_month(Month::December)
            .unwrap()
            .replace_year(date.year() - 1)
            .unwrap()
    } else {
        date.replace_month(date.month().previous()).unwrap()
    }
}

/// Render the UI with a calendar.
fn render(frame: &mut Frame, calendar_style: StyledCalendar, selected_date: Date) {
    let header = Text::from_iter([
        Line::from("Calendar Example".bold()),
        Line::from(
            "<q> Quit | <s> Change Style | <n> Next Month | <p> Previous Month, <hjkl> Move",
        ),
        Line::from(format!(
            "Current date: {selected_date} | Current style: {calendar_style}"
        )),
    ]);

    let [text_area, area] = frame.area().layout(&Layout::vertical([
        Constraint::Length(header.height() as u16),
        Constraint::Fill(1),
    ]));
    frame.render_widget(header.centered(), text_area);
    calendar_style
        .render_year(frame, area, selected_date)
        .unwrap();
}

#[derive(Debug, Clone, Copy)]
enum StyledCalendar {
    Default,
    Surrounding,
    WeekdaysHeader,
    SurroundingAndWeekdaysHeader,
    MonthHeader,
    MonthAndWeekdaysHeader,
}

impl StyledCalendar {
    // Cycle through the different styles.
    const fn next(self) -> Self {
        match self {
            Self::Default => Self::Surrounding,
            Self::Surrounding => Self::WeekdaysHeader,
            Self::WeekdaysHeader => Self::SurroundingAndWeekdaysHeader,
            Self::SurroundingAndWeekdaysHeader => Self::MonthHeader,
            Self::MonthHeader => Self::MonthAndWeekdaysHeader,
            Self::MonthAndWeekdaysHeader => Self::Default,
        }
    }
}

impl fmt::Display for StyledCalendar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default => write!(f, "Default"),
            Self::Surrounding => write!(f, "Show Surrounding"),
            Self::WeekdaysHeader => write!(f, "Show Weekdays Header"),
            Self::SurroundingAndWeekdaysHeader => write!(f, "Show Surrounding and Weekdays Header"),
            Self::MonthHeader => write!(f, "Show Month Header"),
            Self::MonthAndWeekdaysHeader => write!(f, "Show Month Header and Weekdays Header"),
        }
    }
}

impl StyledCalendar {
    fn render_year(self, frame: &mut Frame, area: Rect, date: Date) -> Result<()> {
        let events = events(date)?;

        let vertical = Layout::vertical([Constraint::Ratio(1, 3); 3]);
        let horizontal = &Layout::horizontal([Constraint::Ratio(1, 4); 4]);
        let areas = area
            .inner(Margin::new(1, 1))
            .layout_vec(&vertical)
            .into_iter()
            .flat_map(|row| row.layout_vec(horizontal));
        for (i, area) in areas.enumerate() {
            let month = date
                .replace_day(1)
                .unwrap()
                .replace_month(Month::try_from(i as u8 + 1).unwrap())
                .unwrap();
            self.render_month(frame, area, month, &events);
        }
        Ok(())
    }

    fn render_month(self, frame: &mut Frame, area: Rect, date: Date, events: &CalendarEventStore) {
        let calendar = match self {
            Self::Default => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default()),
            Self::Surrounding => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default())
                .show_surrounding(Style::new().dim()),
            Self::WeekdaysHeader => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default())
                .show_weekdays_header(Style::new().bold().green()),
            Self::SurroundingAndWeekdaysHeader => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default())
                .show_surrounding(Style::new().dim())
                .show_weekdays_header(Style::new().bold().green()),
            Self::MonthHeader => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default())
                .show_month_header(Style::new().bold().green()),
            Self::MonthAndWeekdaysHeader => Monthly::new(date, events)
                .default_style(Style::new().bold().bg(Color::Rgb(50, 50, 50)))
                .show_month_header(Style::default())
                .show_weekdays_header(Style::new().bold().dim().light_yellow()),
        };
        frame.render_widget(calendar, area);
    }
}

/// Makes a list of dates for the current year.
fn events(selected_date: Date) -> Result<CalendarEventStore> {
    const SELECTED: Style = Style::new()
        .fg(Color::White)
        .bg(Color::Red)
        .add_modifier(Modifier::BOLD);
    const HOLIDAY: Style = Style::new()
        .fg(Color::Red)
        .add_modifier(Modifier::UNDERLINED);
    const SEASON: Style = Style::new()
        .fg(Color::Green)
        .bg(Color::Black)
        .add_modifier(Modifier::UNDERLINED);

    let mut list = CalendarEventStore::today(
        Style::default()
            .add_modifier(Modifier::BOLD)
            .bg(Color::Blue),
    );
    let y = selected_date.year();

    // new year's
    list.add(Date::from_calendar_date(y, Month::January, 1)?, HOLIDAY);
    // next new_year's for December "show surrounding"
    list.add(Date::from_calendar_date(y + 1, Month::January, 1)?, HOLIDAY);
    // groundhog day
    list.add(Date::from_calendar_date(y, Month::February, 2)?, HOLIDAY);
    // april fool's
    list.add(Date::from_calendar_date(y, Month::April, 1)?, HOLIDAY);
    // earth day
    list.add(Date::from_calendar_date(y, Month::April, 22)?, HOLIDAY);
    // star wars day
    list.add(Date::from_calendar_date(y, Month::May, 4)?, HOLIDAY);
    // festivus
    list.add(Date::from_calendar_date(y, Month::December, 23)?, HOLIDAY);
    // new year's eve
    list.add(Date::from_calendar_date(y, Month::December, 31)?, HOLIDAY);

    // seasons
    // spring equinox
    list.add(Date::from_calendar_date(y, Month::March, 22)?, SEASON);
    // summer solstice
    list.add(Date::from_calendar_date(y, Month::June, 21)?, SEASON);
    // fall equinox
    list.add(Date::from_calendar_date(y, Month::September, 22)?, SEASON);
    // winter solstice
    list.add(Date::from_calendar_date(y, Month::December, 21)?, SEASON);

    // selected date
    list.add(selected_date, SELECTED);

    Ok(list)
}
