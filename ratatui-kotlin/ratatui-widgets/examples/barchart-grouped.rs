//! # [Ratatui] `BarChart` example with grouped bars
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

use core::iter::zip;

use color_eyre::Result;
use crossterm::event;
use ratatui::Frame;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Bar, BarChart, BarGroup};

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

/// Render the UI with a barchart on the left and right side.
fn render(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let horizontal = Layout::horizontal([Constraint::Fill(1); 2]).spacing(1);
    let [top, main] = frame.area().layout(&vertical);
    let [left, right] = main.layout(&horizontal);

    let title = Line::from_iter([
        Span::from("BarChart Widget (Grouped)").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);
    render_barchart(frame, left, Direction::Vertical, 6);
    render_barchart(frame, right, Direction::Horizontal, 1);
}

/// Render a barchart with grouped bars.
fn render_barchart(frame: &mut Frame, area: Rect, direction: Direction, bar_width: u16) {
    let companies = [
        ("BITE", Color::Blue),
        ("TART", Color::White),
        ("BAKE", Color::LightRed),
    ];
    let revenues = [
        ("Jan", [8500, 6500, 7000]),
        ("Feb", [9000, 7500, 8500]),
        ("Mar", [9500, 4500, 8200]),
        ("Apr", [6300, 4000, 5000]),
    ];

    let mut barchart = BarChart::default()
        .bar_gap(0)
        .bar_width(bar_width)
        .group_gap(2)
        .direction(direction);

    for (period, values) in revenues {
        let bars: Vec<_> = zip(companies, values)
            .map(|((label, color), value)| bar(label, value, color))
            .collect();
        let label = Line::from(period).centered();
        let group = BarGroup::new(bars).label(label);
        barchart = barchart.data(group);
    }

    frame.render_widget(barchart, area);
}

/// Return a bar with the given label, value, and color.
fn bar(label: &str, value: u64, color: Color) -> Bar<'_> {
    Bar::default()
        .label(label)
        .value(value)
        .text_value(format!("{:.1}M", value as f64 / 1000.))
        .style(color)
        .value_style((Color::Black, color))
}
