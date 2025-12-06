//! An example of how to use [`WidgetRef`] to store heterogeneous widgets in a container.
//!
//! This example creates a `StackContainer` widget that can hold any number of widgets of
//! different types. It creates two widgets, `Greeting` and `Farewell`, and stores them in a
//! `StackContainer` with a vertical layout. The `StackContainer` widget renders each of its
//! child widgets in the order they were added.
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use std::iter::zip;

use color_eyre::Result;
use crossterm::event;
use ratatui::Frame;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::widgets::{Block, Paragraph, Widget, WidgetRef};

fn main() -> Result<()> {
    color_eyre::install()?;
    ratatui::run(|terminal| {
        loop {
            terminal.draw(render)?;
            if event::read()?.is_key_press() {
                return Ok(());
            }
        }
    })
}

fn render(frame: &mut Frame) {
    let container = StackContainer {
        direction: Direction::Vertical,
        widgets: vec![
            (Box::new(&Greeting), Constraint::Percentage(50)),
            (Box::new(&Farewell), Constraint::Percentage(50)),
        ],
    };
    frame.render_widget(&container, frame.area());
}

struct Greeting;

impl Widget for &Greeting {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Hello")
            .block(Block::bordered())
            .render(area, buf);
    }
}

struct Farewell;

impl Widget for &Farewell {
    fn render(self, area: Rect, buf: &mut Buffer) {
        Paragraph::new("Goodbye")
            .block(Block::bordered())
            .render(area, buf);
    }
}

struct StackContainer {
    direction: Direction,
    widgets: Vec<(Box<dyn WidgetRef>, Constraint)>,
}

impl Widget for &StackContainer {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let layout = Layout::default()
            .direction(self.direction)
            .constraints(self.widgets.iter().map(|(_, constraint)| *constraint))
            .split(area);
        let widgets = self.widgets.iter().map(|(widget, _)| widget);
        for (widget, area) in zip(widgets, layout.iter()) {
            widget.render_ref(*area, buf);
        }
    }
}
