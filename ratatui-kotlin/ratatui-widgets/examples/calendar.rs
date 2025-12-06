//! # [Ratatui] `Calendar` example
//!
//! The latest version of this example is available in the [widget examples] folder in the
//! repository.
//!
//! Please note that the examples are designed to be run against the `main` branch of the Github
//! repository. This means that you may not be able to compile with the latest release version on
//! crates.io, or the one that you have installed locally.
//!
//! See the [examples readme] for more information on finding examples that match the version of the
//! library you are using.
//!
//! [Ratatui]: https://github.com/ratatui/ratatui
//! [widget examples]: https://github.com/ratatui/ratatui/blob/main/ratatui-widgets/examples
//! [examples readme]: https://github.com/ratatui/ratatui/blob/main/examples/README.md

use color_eyre::Result;
use crossterm::event;
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Padding};
use ratatui_widgets::calendar::{CalendarEventStore, Monthly};
use time::{Date, Month, OffsetDateTime};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        loop {
            terminal.draw(render)?;
            if event::read()?.is_key_press() {
                break Ok(());
            }
        }
    })
}

/// Render the UI with 2 monthly calendars side by side.
fn render(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let horizontal = Layout::horizontal([Constraint::Percentage(50); 2]).spacing(1);
    let [top, main] = frame.area().layout(&vertical);
    let [left, right] = main.layout(&horizontal);

    let title = Line::from_iter([
        Span::from("Calendar Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_current_month(frame, left);
    render_styled_month(frame, right);
}

/// Render the current month calendar.
fn render_current_month(frame: &mut Frame, area: Rect) {
    let date = OffsetDateTime::now_utc().date();

    let monthly = Monthly::new(
        date,
        CalendarEventStore::today(Style::default().red().bold()),
    )
    .block(Block::new().padding(Padding::new(0, 0, 2, 0)))
    .show_month_header(Modifier::BOLD)
    .show_weekdays_header(Modifier::ITALIC);
    frame.render_widget(monthly, area);
}

/// Render an arbitrary month with more styles.
fn render_styled_month(frame: &mut Frame, area: Rect) {
    // Release date of the movie Ratatouille.
    let date = Date::from_calendar_date(2007, Month::June, 29).unwrap();

    let mut event_store = CalendarEventStore::today(Style::default().red().bold());
    event_store.add(date, Style::default().blue().italic());

    let monthly = Monthly::new(date, event_store)
        .show_surrounding(Modifier::DIM)
        .show_month_header(Modifier::BOLD)
        .show_weekdays_header(Style::default().bold().green())
        .default_style(Style::default().bold().bg(Color::Rgb(50, 50, 50)));
    frame.render_widget(monthly, area);
}
