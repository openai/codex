use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::Span;
use ratatui::widgets::{Block, Gauge, LineGauge};
use ratatui::{Terminal, symbols};

#[test]
fn widgets_gauge_renders() {
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.area());

            let gauge = Gauge::default()
                .block(Block::bordered().title("Percentage"))
                .gauge_style(Style::default().bg(Color::Blue).fg(Color::Red))
                .use_unicode(true)
                .percent(43);
            f.render_widget(gauge, chunks[0]);
            let gauge = Gauge::default()
                .block(Block::bordered().title("Ratio"))
                .gauge_style(Style::default().bg(Color::Blue).fg(Color::Red))
                .use_unicode(true)
                .ratio(0.511_313_934_313_1);
            f.render_widget(gauge, chunks[1]);
        })
        .unwrap();
    let mut expected = Buffer::with_lines([
        "                                        ",
        "                                        ",
        "  ┌Percentage────────────────────────┐  ",
        "  │██████████████▋43%                │  ",
        "  └──────────────────────────────────┘  ",
        "  ┌Ratio─────────────────────────────┐  ",
        "  │███████████████51%                │  ",
        "  └──────────────────────────────────┘  ",
        "                                        ",
        "                                        ",
    ]);
    expected.set_style(Rect::new(3, 3, 34, 1), Style::new().red().on_blue());

    expected.set_style(Rect::new(3, 6, 15, 1), Style::new().red().on_blue());
    // Note that filled part of the gauge only covers the 5 and the 1, not the % symbol
    expected.set_style(Rect::new(18, 6, 2, 1), Style::new().blue().on_red());
    expected.set_style(Rect::new(20, 6, 17, 1), Style::new().red().on_blue());

    terminal.backend().assert_buffer(&expected);
}

#[test]
fn widgets_gauge_renders_no_unicode() {
    let backend = TestBackend::new(40, 10);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .margin(2)
                .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
                .split(f.area());

            let gauge = Gauge::default()
                .block(Block::bordered().title("Percentage"))
                .percent(43)
                .use_unicode(false);
            f.render_widget(gauge, chunks[0]);
            let gauge = Gauge::default()
                .block(Block::bordered().title("Ratio"))
                .ratio(0.211_313_934_313_1)
                .use_unicode(false);
            f.render_widget(gauge, chunks[1]);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        "                                        ",
        "                                        ",
        "  ┌Percentage────────────────────────┐  ",
        "  │███████████████43%                │  ",
        "  └──────────────────────────────────┘  ",
        "  ┌Ratio─────────────────────────────┐  ",
        "  │███████        21%                │  ",
        "  └──────────────────────────────────┘  ",
        "                                        ",
        "                                        ",
    ]);
}

#[test]
fn widgets_gauge_applies_styles() {
    let backend = TestBackend::new(12, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let gauge = Gauge::default()
                .block(
                    Block::bordered().title(Span::styled("Test", Style::default().fg(Color::Red))),
                )
                .gauge_style(Style::default().fg(Color::Blue).bg(Color::Red))
                .percent(43)
                .label(Span::styled(
                    "43%",
                    Style::default()
                        .fg(Color::Green)
                        .add_modifier(Modifier::BOLD),
                ));
            f.render_widget(gauge, f.area());
        })
        .unwrap();
    let mut expected = Buffer::with_lines([
        "┌Test──────┐",
        "│████      │",
        "│███43%    │",
        "│████      │",
        "└──────────┘",
    ]);
    // title
    expected.set_style(Rect::new(1, 0, 4, 1), Style::default().fg(Color::Red));
    // gauge area
    expected.set_style(
        Rect::new(1, 1, 10, 3),
        Style::default().fg(Color::Blue).bg(Color::Red),
    );
    // filled area
    expected.set_style(
        Rect::new(1, 1, 4, 3),
        Style::default().fg(Color::Blue).bg(Color::Red),
    );
    // label (foreground and modifier from label style)
    expected.set_style(
        Rect::new(4, 2, 1, 1),
        Style::default()
            .fg(Color::Green)
            // "4" is in the filled area so background is gauge_style foreground
            .bg(Color::Blue)
            .add_modifier(Modifier::BOLD),
    );
    expected.set_style(
        Rect::new(5, 2, 2, 1),
        Style::default()
            .fg(Color::Green)
            // "3%" is not in the filled area so background is gauge_style background
            .bg(Color::Red)
            .add_modifier(Modifier::BOLD),
    );
    terminal.backend().assert_buffer(&expected);
}

#[test]
fn widgets_gauge_supports_large_labels() {
    let backend = TestBackend::new(10, 1);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let gauge = Gauge::default()
                .percent(43)
                .label("43333333333333333333333333333%");
            f.render_widget(gauge, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(["4333333333"]);
}

#[test]
fn widgets_line_gauge_renders() {
    let backend = TestBackend::new(20, 6);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let gauge = LineGauge::default()
                .filled_style(Style::default().fg(Color::Green))
                .unfilled_style(Style::default().fg(Color::White))
                .ratio(0.43);
            f.render_widget(
                gauge,
                Rect {
                    x: 0,
                    y: 0,
                    width: 20,
                    height: 1,
                },
            );
            // custom (same) symbols for filled and unfilled parts
            let gauge = LineGauge::default()
                .block(Block::bordered().title("Gauge 2"))
                .filled_style(Style::default().fg(Color::Green))
                .filled_symbol(symbols::line::THICK_HORIZONTAL)
                .unfilled_symbol(symbols::line::THICK_HORIZONTAL)
                .ratio(0.211_313_934_313_1);
            f.render_widget(
                gauge,
                Rect {
                    x: 0,
                    y: 1,
                    width: 20,
                    height: 3,
                },
            );
            // default symbol for filled part, but empty for unfilled part
            let gauge = LineGauge::default().unfilled_symbol(" ").ratio(0.50);
            f.render_widget(
                gauge,
                Rect {
                    x: 0,
                    y: 4,
                    width: 20,
                    height: 1,
                },
            );
            // different custom symbols for filled unfilled parts
            let gauge = LineGauge::default()
                .filled_symbol("█") // similar to `symbols::bar::FULL`
                .unfilled_symbol("░") // similar to `symbols::shade::LIGHT`
                .ratio(0.80);
            f.render_widget(
                gauge,
                Rect {
                    x: 0,
                    y: 5,
                    width: 20,
                    height: 1,
                },
            );
        })
        .unwrap();
    let mut expected = Buffer::with_lines([
        " 43% ───────────────",
        "┌Gauge 2───────────┐",
        "│ 21% ━━━━━━━━━━━━━│",
        "└──────────────────┘",
        " 50% ───────        ",
        " 80% ████████████░░░",
    ]);
    for col in 5..11 {
        expected[(col, 0)].set_fg(Color::Green);
    }
    for col in 11..20 {
        expected[(col, 0)].set_fg(Color::White);
    }
    for col in 6..8 {
        expected[(col, 2)].set_fg(Color::Green);
    }
    terminal.backend().assert_buffer(&expected);
}
