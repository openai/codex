use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style, Stylize};
use ratatui::widgets::{BarChart, Block, Paragraph};

#[test]
fn barchart_can_be_stylized() {
    let barchart = BarChart::default()
        .on_white()
        .bar_style(Style::new().red())
        .bar_width(2)
        .value_style(Style::new().green())
        .label_style(Style::new().blue())
        .data(&[("A", 1), ("B", 2), ("C", 3)])
        .max(3);

    let area = Rect::new(0, 0, 9, 5);
    let mut terminal = Terminal::new(TestBackend::new(9, 6)).unwrap();
    terminal
        .draw(|f| {
            f.render_widget(barchart, area);
        })
        .unwrap();

    let mut expected = Buffer::with_lines([
        "      ██ ",
        "   ▅▅ ██ ",
        "▂▂ ██ ██ ",
        "1█ 2█ 3█ ",
        "A  B  C  ",
        "         ",
    ]);
    for y in area.y..area.height {
        // background
        for x in area.x..area.width {
            expected[(x, y)].set_bg(Color::White);
        }
        // bars
        for x in [0, 1, 3, 4, 6, 7] {
            expected[(x, y)].set_fg(Color::Red);
        }
    }
    // values
    for x in 0..3 {
        expected[(x * 3, 3)].set_fg(Color::Green);
    }
    // labels
    for x in 0..3 {
        expected[(x * 3, 4)].set_fg(Color::Blue);
        expected[(x * 3 + 1, 4)].set_fg(Color::Reset);
    }
    terminal.backend().assert_buffer(&expected);
}

#[test]
fn block_can_be_stylized() -> Result<(), core::convert::Infallible> {
    let block = Block::bordered()
        .title("Title".light_blue())
        .on_cyan()
        .cyan();

    let area = Rect::new(0, 0, 8, 3);
    let mut terminal = Terminal::new(TestBackend::new(11, 4))?;
    terminal.draw(|f| {
        f.render_widget(block, area);
    })?;

    #[rustfmt::skip]
    let mut expected = Buffer::with_lines([
        "┌Title─┐   ",
        "│      │   ",
        "└──────┘   ",
        "           ",
    ]);
    for x in area.x..area.width {
        for y in area.y..area.height {
            expected[(x, y)].set_fg(Color::Cyan).set_bg(Color::Cyan);
        }
    }
    for x in 1..=5 {
        expected[(x, 0)].set_fg(Color::LightBlue);
    }
    terminal.backend().assert_buffer(&expected);
    Ok(())
}

#[test]
fn paragraph_can_be_stylized() -> Result<(), core::convert::Infallible> {
    let paragraph = Paragraph::new("Text".cyan());

    let area = Rect::new(0, 0, 10, 1);
    let mut terminal = Terminal::new(TestBackend::new(10, 1))?;
    terminal.draw(|f| {
        f.render_widget(paragraph, area);
    })?;

    let mut expected = Buffer::with_lines(["Text      "]);
    for x in 0..4 {
        expected[(x, 0)].set_fg(Color::Cyan);
    }
    terminal.backend().assert_buffer(&expected);
    Ok(())
}
