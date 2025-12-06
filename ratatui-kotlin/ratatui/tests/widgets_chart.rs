use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::{Alignment, Rect};
use ratatui::style::{Color, Style};
use ratatui::text::{self, Span};
use ratatui::widgets::GraphType::Line;
use ratatui::widgets::{Axis, Block, Chart, Dataset};
use ratatui::{Terminal, symbols};
use rstest::rstest;

fn create_labels<'a>(labels: &'a [&'a str]) -> Vec<Span<'a>> {
    labels.iter().map(|l| Span::from(*l)).collect()
}

#[track_caller]
fn axis_test_case<'line, Lines>(
    width: u16,
    height: u16,
    x_axis: Axis,
    y_axis: Axis,
    expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<text::Line<'line>>,
{
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let chart = Chart::new(vec![]).x_axis(x_axis).y_axis(y_axis);
            f.render_widget(chart, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case(0, 0)]
#[case(0, 1)]
#[case(1, 0)]
#[case(1, 1)]
#[case(2, 2)]
fn widgets_chart_can_render_on_small_areas(#[case] width: u16, #[case] height: u16) {
    let backend = TestBackend::new(width, height);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let datasets = vec![
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::Magenta))
                    .data(&[(0.0, 0.0)]),
            ];
            let chart = Chart::new(datasets)
                .block(Block::bordered().title("Plot"))
                .x_axis(
                    Axis::default()
                        .bounds([0.0, 0.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                )
                .y_axis(
                    Axis::default()
                        .bounds([0.0, 0.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                );
            f.render_widget(chart, f.area());
        })
        .unwrap();
}

#[rstest]
#[case(
    Some(("AAAA", "B")),
    None,
    Alignment::Left,
    vec![
        "          ",
        "          ",
        "          ",
        "   ───────",
        "AAA      B",
    ],
)]
#[case(
    Some(("A", "BBBB")),
    None,
    Alignment::Left,
    vec![
        "          ",
        "          ",
        "          ",
        " ─────────",
        "A     BBBB",
    ],
)]
#[case(
    Some(("AAAAAAAAAAA", "B")),
    None,
    Alignment::Left,
    vec![
        "          ",
        "          ",
        "          ",
        "   ───────",
        "AAA      B",
    ],
)]
#[case(
    Some(("A", "B")),
    Some(("CCCCCCC", "D")),
    Alignment::Left,
    vec![
        "D  │      ",
        "   │      ",
        "CCC│      ",
        "   └──────",
        "   A     B",
    ],
)]
#[case(
    Some(("AAAAAAAAAA", "B")),
    Some(("C", "D")),
    Alignment::Center,
    vec![
        "D  │      ",
        "   │      ",
        "C  │      ",
        "   └──────",
        "AAAAAAA  B",
    ],
)]
#[case(
    Some(("AAAAAAA", "B")),
    Some(("C", "D")),
    Alignment::Right,
    vec![
        "D│        ",
        " │        ",
        "C│        ",
        " └────────",
        " AAAAA   B",
    ],
)]
#[case(
    Some(("AAAAAAA", "BBBBBBB")),
    Some(("C", "D")),
    Alignment::Right,
    vec![
        "D│        ",
        " │        ",
        "C│        ",
        " └────────",
        " AAAAABBBB",
    ],
)]
fn widgets_chart_handles_long_labels<'line, Lines>(
    #[case] x_labels: Option<(&str, &str)>,
    #[case] y_labels: Option<(&str, &str)>,
    #[case] x_alignment: Alignment,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<text::Line<'line>>,
{
    let mut x_axis = Axis::default().bounds([0.0, 1.0]);
    if let Some((left_label, right_label)) = x_labels {
        x_axis = x_axis
            .labels([left_label, right_label])
            .labels_alignment(x_alignment);
    }
    let mut y_axis = Axis::default().bounds([0.0, 1.0]);
    if let Some((left_label, right_label)) = y_labels {
        y_axis = y_axis.labels([left_label, right_label]);
    }
    axis_test_case(10, 5, x_axis, y_axis, expected);
}

#[rstest]
#[case::left(
    Alignment::Left,
    vec![
        "          ",
        "          ",
        "          ",
        "   ───────",
        "AAA   B  C",
    ],
)]
#[case::center(
    Alignment::Center,
    vec![
        "          ",
        "          ",
        "          ",
        "  ────────",
        "AAAA B   C",
    ],
)]
#[case::right(
    Alignment::Right,
    vec![
        "          ",
        "          ",
        "          ",
        "──────────",
        "AAA B    C",
    ],
)]
fn widgets_chart_handles_x_axis_labels_alignments<'line, Lines>(
    #[case] y_alignment: Alignment,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<text::Line<'line>>,
{
    let x_axis = Axis::default()
        .labels(["AAAA", "B", "C"])
        .labels_alignment(y_alignment);
    let y_axis = Axis::default();
    axis_test_case(10, 5, x_axis, y_axis, expected);
}

#[rstest]
#[case::left(Alignment::Left, [
    "D   │               ",
    "    │               ",
    "C   │               ",
    "    └───────────────",
    "AAAAA              B",
])]
#[case::center(Alignment::Center, [
    " D  │               ",
    "    │               ",
    " C  │               ",
    "    └───────────────",
    "AAAAA              B",
])]
#[case::right(Alignment::Right, [
    "   D│               ",
    "    │               ",
    "   C│               ",
    "    └───────────────",
    "AAAAA              B",
])]
fn widgets_chart_handles_y_axis_labels_alignments<'line, Lines>(
    #[case] y_alignment: Alignment,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<text::Line<'line>>,
{
    let x_axis = Axis::default().labels(create_labels(&["AAAAA", "B"]));
    let y_axis = Axis::default()
        .labels(create_labels(&["C", "D"]))
        .labels_alignment(y_alignment);
    axis_test_case(20, 5, x_axis, y_axis, expected);
}

#[test]
fn widgets_chart_can_have_axis_with_zero_length_bounds() {
    let backend = TestBackend::new(100, 100);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let datasets = vec![
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::Magenta))
                    .data(&[(0.0, 0.0)]),
            ];
            let chart = Chart::new(datasets)
                .block(Block::bordered().title("Plot"))
                .x_axis(
                    Axis::default()
                        .bounds([0.0, 0.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                )
                .y_axis(
                    Axis::default()
                        .bounds([0.0, 0.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                );
            f.render_widget(
                chart,
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            );
        })
        .unwrap();
}

#[test]
fn widgets_chart_handles_overflows() {
    let backend = TestBackend::new(80, 30);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let datasets = vec![
                Dataset::default()
                    .marker(symbols::Marker::Braille)
                    .style(Style::default().fg(Color::Magenta))
                    .data(&[
                        (1_588_298_471.0, 1.0),
                        (1_588_298_473.0, 0.0),
                        (1_588_298_496.0, 1.0),
                    ]),
            ];
            let chart = Chart::new(datasets)
                .block(Block::bordered().title("Plot"))
                .x_axis(
                    Axis::default()
                        .bounds([1_588_298_471.0, 1_588_992_600.0])
                        .labels(create_labels(&["1588298471.0", "1588992600.0"])),
                )
                .y_axis(
                    Axis::default()
                        .bounds([0.0, 1.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                );
            f.render_widget(
                chart,
                Rect {
                    x: 0,
                    y: 0,
                    width: 80,
                    height: 30,
                },
            );
        })
        .unwrap();
}

#[test]
fn widgets_chart_can_have_empty_datasets() {
    let backend = TestBackend::new(100, 100);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|f| {
            let datasets = vec![Dataset::default().data(&[]).graph_type(Line)];
            let chart = Chart::new(datasets)
                .block(Block::bordered().title("Empty Dataset With Line"))
                .x_axis(
                    Axis::default()
                        .bounds([0.0, 0.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                )
                .y_axis(
                    Axis::default()
                        .bounds([0.0, 1.0])
                        .labels(create_labels(&["0.0", "1.0"])),
                );
            f.render_widget(
                chart,
                Rect {
                    x: 0,
                    y: 0,
                    width: 100,
                    height: 100,
                },
            );
        })
        .unwrap();
}

#[test]
fn widgets_chart_top_line_styling_is_correct() {
    let backend = TestBackend::new(9, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    let title_style = Style::default().fg(Color::Red).bg(Color::LightRed);
    let data_style = Style::default().fg(Color::Blue);

    terminal
        .draw(|f| {
            let data: [(f64, f64); 2] = [(0.0, 1.0), (1.0, 1.0)];
            let widget = Chart::new(vec![
                Dataset::default()
                    .data(&data)
                    .graph_type(ratatui::widgets::GraphType::Line)
                    .style(data_style),
            ])
            .y_axis(
                Axis::default()
                    .title(Span::styled("abc", title_style))
                    .bounds([0.0, 1.0])
                    .labels(create_labels(&["a", "b"])),
            )
            .x_axis(Axis::default().bounds([0.0, 1.0]));
            f.render_widget(widget, f.area());
        })
        .unwrap();

    let mut expected = Buffer::with_lines([
        "b│abc••••",
        " │       ",
        " │       ",
        " │       ",
        "a│       ",
    ]);
    expected.set_style(Rect::new(2, 0, 3, 1), title_style);
    expected.set_style(Rect::new(5, 0, 4, 1), data_style);
    terminal.backend().assert_buffer(&expected);
}
