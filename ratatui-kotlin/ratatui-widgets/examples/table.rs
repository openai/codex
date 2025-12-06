//! # [Ratatui] `Table` example
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
use crossterm::event::{self, KeyCode};
use ratatui::Frame;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Style, Stylize};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Row, Table, TableState};

fn main() -> Result<()> {
    color_eyre::install()?;

    let mut table_state = TableState::default();
    table_state.select_first();
    table_state.select_first_column();
    ratatui::run(|terminal| {
        loop {
            terminal.draw(|frame| render(frame, &mut table_state))?;
            if let Some(key) = event::read()?.as_key_press_event() {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('j') | KeyCode::Down => table_state.select_next(),
                    KeyCode::Char('k') | KeyCode::Up => table_state.select_previous(),
                    KeyCode::Char('l') | KeyCode::Right => table_state.select_next_column(),
                    KeyCode::Char('h') | KeyCode::Left => table_state.select_previous_column(),
                    KeyCode::Char('g') => table_state.select_first(),
                    KeyCode::Char('G') => table_state.select_last(),
                    _ => {}
                }
            }
        }
    })
}

/// Render the UI with a table.
fn render(frame: &mut Frame, table_state: &mut TableState) {
    let layout = Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).spacing(1);
    let [top, main] = frame.area().layout(&layout);

    let title = Line::from_iter([
        Span::from("Table Widget").bold(),
        Span::from(" (Press 'q' to quit and arrow keys to navigate)"),
    ]);
    frame.render_widget(title.centered(), top);

    render_table(frame, main, table_state);
}

/// Render a table with some rows and columns.
pub fn render_table(frame: &mut Frame, area: Rect, table_state: &mut TableState) {
    let header = Row::new(["Ingredient", "Quantity", "Macros"])
        .style(Style::new().bold())
        .bottom_margin(1);

    let rows = [
        Row::new(["Eggplant", "1 medium", "25 kcal, 6g carbs, 1g protein"]),
        Row::new(["Tomato", "2 large", "44 kcal, 10g carbs, 2g protein"]),
        Row::new(["Zucchini", "1 medium", "33 kcal, 7g carbs, 2g protein"]),
        Row::new(["Bell Pepper", "1 medium", "24 kcal, 6g carbs, 1g protein"]),
        Row::new(["Garlic", "2 cloves", "9 kcal, 2g carbs, 0.4g protein"]),
    ];
    let footer = Row::new([
        "Ratatouille Recipe",
        "",
        "135 kcal, 31g carbs, 6.4g protein",
    ]);
    let widths = [
        Constraint::Percentage(30),
        Constraint::Percentage(20),
        Constraint::Percentage(50),
    ];
    let table = Table::new(rows, widths)
        .header(header)
        .footer(footer.italic())
        .column_spacing(1)
        .style(Color::White)
        .row_highlight_style(Style::new().on_black().bold())
        .column_highlight_style(Color::Gray)
        .cell_highlight_style(Style::new().reversed().yellow())
        .highlight_symbol("🍴 ");

    frame.render_stateful_widget(table, area, table_state);
}
