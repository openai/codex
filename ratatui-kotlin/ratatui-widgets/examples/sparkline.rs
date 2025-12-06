//! # [Ratatui] `Sparkline` example
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

use core::time::Duration;

use color_eyre::Result;
use crossterm::event;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{RenderDirection, Sparkline};
use ratatui::{Frame, symbols};

fn main() -> Result<()> {
    color_eyre::install()?;
    let frame_timeout = Duration::from_secs_f64(1.0 / 60.0); // run at 60 FPS
    ratatui::run(|terminal| {
        loop {
            terminal.draw(render)?;
            if event::poll(frame_timeout)? && event::read()?.is_key_press() {
                break Ok(());
            }
        }
    })
}

/// Render the UI with various sparklines.
fn render(frame: &mut Frame) {
    let constraints = [
        Constraint::Length(1),
        Constraint::Max(2),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ];
    let layout = Layout::vertical(constraints).spacing(1);
    let [top, first, second, _] = frame.area().layout(&layout);

    let title = Line::from_iter([
        Span::from("Sparkline Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_sparkline(frame, first);
    render_sin_wave(frame, second);
}

/// Render a sparkline with some sample data.
pub fn render_sparkline(frame: &mut Frame, area: Rect) {
    let data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10].repeat(area.width.into());
    let sparkline = Sparkline::default()
        .data(&data)
        .max(10)
        .direction(RenderDirection::LeftToRight)
        .style(Color::Cyan);

    frame.render_widget(sparkline, area);
}

/// Render a sin wave based on the current frame count.
pub fn render_sin_wave(frame: &mut Frame, area: Rect) {
    let phase_shift = frame.count() as f64 * 0.2;
    let data: Vec<u64> = (0..area.width)
        .map(|v| {
            let angle = f64::from(v) * 0.5 + phase_shift;
            ((angle.sin() * 3.0 + 3.0) * 10.0).round() as u64
        })
        .collect();

    let sparkline = Sparkline::default()
        .data(&data)
        .max(100)
        .direction(RenderDirection::RightToLeft)
        .style(Style::default().magenta().on_black())
        .absent_value_style(Color::Red)
        .absent_value_symbol(symbols::shade::FULL);

    frame.render_widget(sparkline, area);
}
