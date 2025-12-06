#![cfg(feature = "widget-calendar")]
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::Style;
use ratatui::widgets::Widget;
use ratatui::widgets::calendar::{CalendarEventStore, Monthly};
use time::{Date, Month};

#[track_caller]
fn test_render<W: Widget>(widget: W, width: u16, height: u16, expected: &Buffer) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_widget(widget, f.area()))
        .unwrap();
    terminal.backend().assert_buffer(expected);
}

#[test]
fn days_layout() {
    let c = Monthly::new(
        Date::from_calendar_date(2023, Month::January, 1).unwrap(),
        CalendarEventStore::default(),
    );
    let expected = Buffer::with_lines([
        "  1  2  3  4  5  6  7",
        "  8  9 10 11 12 13 14",
        " 15 16 17 18 19 20 21",
        " 22 23 24 25 26 27 28",
        " 29 30 31",
    ]);
    test_render(c, 21, 5, &expected);
}

#[test]
fn days_layout_show_surrounding() {
    let c = Monthly::new(
        Date::from_calendar_date(2023, Month::December, 1).unwrap(),
        CalendarEventStore::default(),
    )
    .show_surrounding(Style::default());
    let expected = Buffer::with_lines([
        " 26 27 28 29 30  1  2",
        "  3  4  5  6  7  8  9",
        " 10 11 12 13 14 15 16",
        " 17 18 19 20 21 22 23",
        " 24 25 26 27 28 29 30",
        " 31  1  2  3  4  5  6",
    ]);
    test_render(c, 21, 6, &expected);
}

#[test]
fn show_month_header() {
    let c = Monthly::new(
        Date::from_calendar_date(2023, Month::January, 1).unwrap(),
        CalendarEventStore::default(),
    )
    .show_month_header(Style::default());
    let expected = Buffer::with_lines([
        "    January 2023     ",
        "  1  2  3  4  5  6  7",
        "  8  9 10 11 12 13 14",
        " 15 16 17 18 19 20 21",
        " 22 23 24 25 26 27 28",
        " 29 30 31",
    ]);
    test_render(c, 21, 6, &expected);
}

#[test]
fn show_weekdays_header() {
    let c = Monthly::new(
        Date::from_calendar_date(2023, Month::January, 1).unwrap(),
        CalendarEventStore::default(),
    )
    .show_weekdays_header(Style::default());
    let expected = Buffer::with_lines([
        " Su Mo Tu We Th Fr Sa",
        "  1  2  3  4  5  6  7",
        "  8  9 10 11 12 13 14",
        " 15 16 17 18 19 20 21",
        " 22 23 24 25 26 27 28",
        " 29 30 31",
    ]);
    test_render(c, 21, 6, &expected);
}

#[test]
fn show_combo() {
    let c = Monthly::new(
        Date::from_calendar_date(2023, Month::January, 1).unwrap(),
        CalendarEventStore::default(),
    )
    .show_weekdays_header(Style::default())
    .show_month_header(Style::default())
    .show_surrounding(Style::default());
    let expected = Buffer::with_lines([
        "    January 2023     ",
        " Su Mo Tu We Th Fr Sa",
        "  1  2  3  4  5  6  7",
        "  8  9 10 11 12 13 14",
        " 15 16 17 18 19 20 21",
        " 22 23 24 25 26 27 28",
        " 29 30 31  1  2  3  4",
    ]);
    test_render(c, 21, 7, &expected);
}
