//! # [Ratatui] `Canvas` example
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
use ratatui::style::{Color, Stylize};
use ratatui::symbols::Marker;
use ratatui::text::{Line as TextLine, Span};
use ratatui::widgets::canvas::{Canvas, Line, Map, MapResolution, Rectangle};
use ratatui_widgets::canvas::Points;

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

/// Render the UI with a canvas widget.
fn render(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let horizontal = Layout::horizontal([Constraint::Percentage(100)]).spacing(1);
    let [top, main] = frame.area().layout(&vertical);
    let [area] = main.layout(&horizontal);

    let title = TextLine::from_iter([
        Span::from("Canvas Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_canvas(frame, area);
}

/// Renders the canvas widget with various shapes and a map.
pub fn render_canvas(frame: &mut Frame, area: Rect) {
    let canvas = Canvas::default()
        .x_bounds([-180.0, 180.0])
        .y_bounds([-90.0, 90.0])
        .marker(Marker::Braille)
        .paint(|ctx| {
            ctx.draw(&Map {
                resolution: MapResolution::High,
                color: Color::White,
            });
            ctx.layer();
            ctx.draw(&Line::new(0.0, 10.0, 10.0, 10.0, Color::Blue));
            ctx.draw(&Rectangle {
                x: 10.0,
                y: 20.0,
                width: 10.0,
                height: 10.0,
                color: Color::Green,
            });
            ctx.draw(&Points {
                coords: &[
                    (2.3522, 48.8566),    // Paris
                    (-122.3321, 47.6062), // Seattle
                    (-79.3837, 43.6511),  // Toronto
                    (32.8597, 39.9334),   // Ankara
                ],
                color: Color::Red,
            });
        });

    frame.render_widget(canvas, area);
}
