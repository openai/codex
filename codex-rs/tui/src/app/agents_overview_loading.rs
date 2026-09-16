//! Transient feedback while command-center selection waits for a session to load.
//! Draw directly because the app event handler cannot process scheduled frames while awaiting.

use crate::tui::Tui;
use ratatui::style::Stylize;
use ratatui::text::Line;
use ratatui::widgets::Paragraph;

pub(super) fn draw(tui: &mut Tui) -> std::io::Result<()> {
    tui.draw(u16::MAX, |frame| {
        let lines = textwrap::wrap("Loading task…", usize::from(frame.area().width.max(1)))
            .into_iter()
            .map(|line| Line::from(line.into_owned().bold()))
            .collect::<Vec<_>>();
        frame.render_widget_ref(&Paragraph::new(lines), frame.area());
    })?;
    tui.frame_requester().schedule_frame();
    Ok(())
}
