use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Bar, BarChart, BarGroup, Block};

// check that bars fill up correctly up to max value
#[test]
fn widgets_barchart_not_full_below_max_value() {
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let barchart = BarChart::default()
                .block(Block::bordered())
                .data(&[("empty", 0), ("half", 50), ("almost", 99), ("full", 100)])
                .max(100)
                .bar_width(7)
                .bar_gap(0);
            f.render_widget(barchart, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        "┌────────────────────────────┐",
        "│              ▇▇▇▇▇▇▇███████│",
        "│              ██████████████│",
        "│              ██████████████│",
        "│       ▄▄▄▄▄▄▄██████████████│",
        "│       █████████████████████│",
        "│       █████████████████████│",
        "│       ██50█████99█████100██│",
        "│ empty  half  almost  full  │",
        "└────────────────────────────┘",
    ]);
}

#[test]
fn widgets_barchart_group() {
    const TERMINAL_HEIGHT: u16 = 11;
    let backend = TestBackend::new(35, TERMINAL_HEIGHT);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let barchart = BarChart::default()
                .block(Block::bordered())
                .data(
                    BarGroup::default().label("Mar").bars(&[
                        Bar::default()
                            .value(10)
                            .label("C1")
                            .style(Style::default().fg(Color::Red))
                            .value_style(Style::default().fg(Color::Blue)),
                        Bar::default()
                            .value(20)
                            .style(Style::default().fg(Color::Green))
                            .text_value("20M"),
                    ]),
                )
                .data(&vec![("C1", 50), ("C2", 40)])
                .data(&[("C1", 60), ("C2", 90)])
                .data(&[("xx", 10), ("xx", 10)])
                .group_gap(2)
                .bar_width(4)
                .bar_gap(1);
            f.render_widget(barchart, f.area());
        })
        .unwrap();

    let mut expected = Buffer::with_lines([
        "┌─────────────────────────────────┐",
        "│                             ████│",
        "│                             ████│",
        "│                        ▅▅▅▅ ████│",
        "│            ▇▇▇▇        ████ ████│",
        "│            ████ ████   ████ ████│",
        "│     ▄▄▄▄   ████ ████   ████ ████│",
        "│▆10▆ 20M█   █50█ █40█   █60█ █90█│",
        "│ C1          C1   C2     C1   C2 │",
        "│Mar                              │",
        "└─────────────────────────────────┘",
    ]);
    for y in 1..(TERMINAL_HEIGHT - 3) {
        for x in 1..5 {
            expected[(x, y)].set_fg(Color::Red);
            expected[(x + 5, y)].set_fg(Color::Green);
        }
    }
    expected[(2, 7)].set_fg(Color::Blue);
    expected[(3, 7)].set_fg(Color::Blue);
    terminal.backend().assert_buffer(&expected);
}
