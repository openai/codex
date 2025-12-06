//! A Ratatui example that demonstrates how to handle popups.
//! See also:
//! - <https://github.com/joshka/tui-popup> and
//! - <https://github.com/sephiroth74/tui-confirm-dialog>
//!
//! This example runs with the Ratatui library code in the branch that you are currently
//! reading. See the [`latest`] branch for the code which works with the most recent Ratatui
//! release.
//!
//! [`latest`]: https://github.com/ratatui/ratatui/tree/latest
use color_eyre::Result;
use crossterm::event::{self, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear};

fn main() -> Result<()> {
    color_eyre::install()?;

    // This flag will be toggled when the user presses 'p'. This could be stored in an app struct
    // if you have more state to manage than just this flag.
    let mut show_popup = false;

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| render(frame, show_popup))?;

            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char('q') => return Ok(()),
                    KeyCode::Char('p') => show_popup = !show_popup,
                    _ => {}
                }
            }
        }
    })
}

fn render(frame: &mut Frame, show_popup: bool) {
    let area = frame.area();

    let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]);
    let [instructions, content] = area.layout(&layout);

    frame.render_widget(
        Line::from("Press 'p' to toggle popup, 'q' to quit").centered(),
        instructions,
    );

    frame.render_widget(Block::bordered().title("Content").on_blue(), content);

    if show_popup {
        let popup = Block::bordered().title("Popup");
        let popup_area = centered_area(area, 60, 20);
        // clears out any background in the area before rendering the popup
        frame.render_widget(Clear, popup_area);
        frame.render_widget(popup, popup_area);
    }
}

/// Create a centered rect using up certain percentage of the available rect
fn centered_area(area: Rect, percent_x: u16, percent_y: u16) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(percent_y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(percent_x)]).flex(Flex::Center);
    let [area] = area.layout(&vertical);
    let [area] = area.layout(&horizontal);
    area
}
