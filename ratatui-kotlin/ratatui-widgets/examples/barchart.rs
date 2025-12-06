//! # [Ratatui] `BarChart` example
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
use ratatui::style::Stylize;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart};

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

/// Render the UI with a title and two barcharts.
fn render(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let horizontal = Layout::horizontal([Constraint::Length(28), Constraint::Fill(1)]).spacing(1);
    let [top, main] = frame.area().layout(&vertical);
    let [left, right] = main.layout(&horizontal);

    let title = Line::from_iter([
        Span::from("BarChart Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);
    render_vertical_barchart(frame, left);
    render_horizontal_barchart(frame, right);
}

/// Render a horizontal barchart with some sample data.
fn render_horizontal_barchart(frame: &mut Frame, area: Rect) {
    let bars = vec![
        Bar::with_label("Red", 30).red(),
        Bar::with_label("Blue", 20).blue(),
        Bar::with_label("Green", 15).green(),
        Bar::with_label("Yellow", 10).yellow(),
    ];
    let chart = BarChart::horizontal(bars).bar_width(3);
    frame.render_widget(chart, area);
}

/// Render a vertical barchart with some sample data.
fn render_vertical_barchart(frame: &mut Frame, area: Rect) {
    let bars = vec![
        Bar::with_label("Red", 30).red(),
        Bar::with_label("Blue", 20).blue(),
        Bar::with_label("Green", 15).green(),
        Bar::with_label("Yellow", 10).yellow(),
    ];
    let chart = BarChart::vertical(bars).bar_width(6);
    frame.render_widget(chart, area);
}
