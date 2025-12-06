//! # [Ratatui] `Paragraph` example
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
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Stylize};
use ratatui::text::{Line, Masked, Span};
use ratatui::widgets::{Paragraph, Wrap};

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

/// Render the UI with various text.
fn render(frame: &mut Frame) {
    let vertical = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let horizontal = Layout::horizontal([Constraint::Percentage(50); 2]).spacing(1);
    let [top, main] = frame.area().layout(&vertical);
    let [first, second] = main.layout(&horizontal);

    let title = Line::from_iter([
        Span::from("Paragraph Widget").bold(),
        Span::from(" (Press 'q' to quit)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_centered_paragraph(frame, first);
    render_wrapped_paragraph(frame, second);
}

/// Render a paragraph with centered text.
pub fn render_centered_paragraph(frame: &mut Frame, area: Rect) {
    let text = "Centered text\nwith multiple lines.\nCheck out the recipe!";
    let paragraph = Paragraph::new(text)
        .style(Color::White)
        .alignment(Alignment::Center);

    frame.render_widget(paragraph, area);
}

/// Render a long paragraph that wraps text.
pub fn render_wrapped_paragraph(frame: &mut Frame, area: Rect) {
    let paragraph = Paragraph::new(create_lines(area))
        .style(Color::White)
        .scroll((0, 0))
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

/// Returns the lines for the paragraph.
fn create_lines(area: Rect) -> Vec<Line<'static>> {
    let short_line = "Slice, layer, and bake the vegetables. ";
    let long_line = short_line.repeat((area.width as usize) / short_line.len() + 2);
    vec![
        "Recipe: Ratatouille".into(),
        "Ingredients:".bold().into(),
        Line::from_iter([
            "Bell Peppers".into(),
            ", Eggplant".italic(),
            ", Tomatoes".bold(),
            ", Onion".into(),
        ]),
        Line::from_iter([
            "Secret Ingredient: ".underlined(),
            Span::styled(Masked::new("herbs de Provence", '*'), Color::Red),
        ]),
        "Instructions:".bold().yellow().into(),
        long_line.green().italic().into(),
    ]
}
