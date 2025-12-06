/// A Ratatui example that how to create hyperlinks in the terminal using [OSC 8].
///
/// This example runs with the Ratatui library code in the branch that you are currently
/// reading. See the [`latest`] branch for the code which works with the most recent Ratatui
/// release.
///
/// [`latest`]: https://github.com/ratatui/ratatui/tree/latest
/// [OSC 8]: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
use color_eyre::Result;
use crossterm::event;
use itertools::Itertools;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Stylize;
use ratatui::text::{Line, Text};
use ratatui::widgets::Widget;

fn main() -> Result<()> {
    color_eyre::install()?;

    let text = Line::from(vec!["Example ".into(), "hyperlink".blue()]);
    let hyperlink = Hyperlink::new(text, "https://example.com");

    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| frame.render_widget(&hyperlink, frame.area()))?;
            if event::read()?.is_key_press() {
                break Ok(());
            }
        }
    })
}

/// A hyperlink widget that renders a hyperlink in the terminal using [OSC 8].
///
/// [OSC 8]: https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda
struct Hyperlink<'content> {
    text: Text<'content>,
    url: String,
}

impl<'content> Hyperlink<'content> {
    fn new(text: impl Into<Text<'content>>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            url: url.into(),
        }
    }
}

impl Widget for &Hyperlink<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        (&self.text).render(area, buffer);

        // this is a hacky workaround for https://github.com/ratatui/ratatui/issues/902, a bug
        // in the terminal code that incorrectly calculates the width of ANSI escape sequences. It
        // works by rendering the hyperlink as a series of 2-character chunks, which is the
        // calculated width of the hyperlink text.
        for (i, two_chars) in self
            .text
            .to_string()
            .chars()
            .chunks(2)
            .into_iter()
            .enumerate()
        {
            let text = two_chars.collect::<String>();
            let hyperlink = format!("\x1B]8;;{}\x07{}\x1B]8;;\x07", self.url, text);
            buffer[(area.x + i as u16 * 2, area.y)].set_symbol(hyperlink.as_str());
        }
    }
}
