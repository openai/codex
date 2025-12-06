#![allow(deprecated)]

use ratatui::Terminal;
use ratatui::backend::TestBackend;
use ratatui::buffer::Buffer;
use ratatui::layout::Constraint;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Cell, HighlightSpacing, Row, Table, TableState};
use rstest::rstest;

#[rstest]
#[case::no_space_between_columns(0, [
    "┌────────────────────────────┐",
    "│Head1Head2Head3             │",
    "│                            │",
    "│Row11Row12Row13             │",
    "│Row21Row22Row23             │",
    "│Row31Row32Row33             │",
    "│Row41Row42Row43             │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::one_space_between_columns(1, [
    "┌────────────────────────────┐",
    "│Head1 Head2 Head3           │",
    "│                            │",
    "│Row11 Row12 Row13           │",
    "│Row21 Row22 Row23           │",
    "│Row31 Row32 Row33           │",
    "│Row41 Row42 Row43           │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::large_width_just_before_pushing_a_column_off(6, [
    "┌────────────────────────────┐",
    "│Head1      Head2      Head3 │",
    "│                            │",
    "│Row11      Row12      Row13 │",
    "│Row21      Row22      Row23 │",
    "│Row31      Row32      Row33 │",
    "│Row41      Row42      Row43 │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::large_width_pushes_part_of_third_column_off(7, [
    "┌────────────────────────────┐",
    "│Head1       Head       Head3│",
    "│                            │",
    "│Row11       Row1       Row13│",
    "│Row21       Row2       Row23│",
    "│Row31       Row3       Row33│",
    "│Row41       Row4       Row43│",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
fn widgets_table_column_spacing_can_be_changed<'line, Lines>(
    #[case] column_spacing: u16,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                ],
                [
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .column_spacing(column_spacing);
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::zero_width_shows_nothing( &[
    Constraint::Length(0),
    Constraint::Length(0),
    Constraint::Length(0),
], [
    "┌────────────────────────────┐",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::slim_columns_trim_data(&[
    Constraint::Length(1),
    Constraint::Length(1),
    Constraint::Length(1),
], [
    "┌────────────────────────────┐",
    "│H H H                       │",
    "│                            │",
    "│R R R                       │",
    "│R R R                       │",
    "│R R R                       │",
    "│R R R                       │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::large_width_just_before_pushing_a_column_off(&[
    Constraint::Length(8),
    Constraint::Length(8),
    Constraint::Length(8),
], [
    "┌────────────────────────────┐",
    "│Head1    Head2    Head3     │",
    "│                            │",
    "│Row11    Row12    Row13     │",
    "│Row21    Row22    Row23     │",
    "│Row31    Row32    Row33     │",
    "│Row41    Row42    Row43     │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
fn widgets_table_columns_widths_can_use_fixed_length_constraints<'line, Lines>(
    #[case] widths: &[Constraint],
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                ],
                widths,
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered());
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::zero_width_shows_nothing(&[
    Constraint::Percentage(0),
    Constraint::Percentage(0),
    Constraint::Percentage(0),
], [
    "┌────────────────────────────┐",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::slim_columns_trim_data(&[
    Constraint::Percentage(11),
    Constraint::Percentage(11),
    Constraint::Percentage(11),
], [
    "┌────────────────────────────┐",
    "│HeaHeaHea                   │",
    "│                            │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::large_width_just_before_pushing_a_column_off(&[
    Constraint::Percentage(33),
    Constraint::Percentage(33),
    Constraint::Percentage(33),
], [
    "┌────────────────────────────┐",
    "│Head1    Head2    Head3     │",
    "│                            │",
    "│Row11    Row12    Row13     │",
    "│Row21    Row22    Row23     │",
    "│Row31    Row32    Row33     │",
    "│Row41    Row42    Row43     │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::sum_100_equal_widths(&[Constraint::Percentage(50), Constraint::Percentage(50)], [
    "┌────────────────────────────┐",
    "│Head1         Head2         │",
    "│                            │",
    "│Row11         Row12         │",
    "│Row21         Row22         │",
    "│Row31         Row32         │",
    "│Row41         Row42         │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
fn widgets_table_columns_widths_can_use_percentage_constraints<'line, Lines>(
    #[case] widths: &[Constraint],
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                ],
                widths,
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .column_spacing(0);
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::zero_width_shows_nothing(&[
    Constraint::Percentage(0),
    Constraint::Length(0),
    Constraint::Percentage(0),
], [
    "┌────────────────────────────┐",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::slim_columns_trim_data(&[
    Constraint::Percentage(11),
    Constraint::Length(20),
    Constraint::Percentage(11),
], [
    "┌────────────────────────────┐",
    "│Hea Head2                Hea│",
    "│                            │",
    "│Row Row12                Row│",
    "│Row Row22                Row│",
    "│Row Row32                Row│",
    "│Row Row42                Row│",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::large_width_just_before_pushing_a_column_off(&[
    Constraint::Percentage(33),
    Constraint::Length(10),
    Constraint::Percentage(33),
], [
    "┌────────────────────────────┐",
    "│Head1     Head2      Head3  │",
    "│                            │",
    "│Row11     Row12      Row13  │",
    "│Row21     Row22      Row23  │",
    "│Row31     Row32      Row33  │",
    "│Row41     Row42      Row43  │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::more_than_100(&[
    Constraint::Percentage(60),
    Constraint::Length(10),
    Constraint::Percentage(60),
], [
    "┌────────────────────────────┐",
    "│Head1      Head2      Head3 │",
    "│                            │",
    "│Row11      Row12      Row13 │",
    "│Row21      Row22      Row23 │",
    "│Row31      Row32      Row33 │",
    "│Row41      Row42      Row43 │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
fn widgets_table_columns_widths_can_use_mixed_constraints<'line, Lines>(
    #[case] widths: &[Constraint],
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                ],
                widths,
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered());
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::zero_shows_nothing(&[
    Constraint::Ratio(0, 1),
    Constraint::Ratio(0, 1),
    Constraint::Ratio(0, 1),
], [
    "┌────────────────────────────┐",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::slim_trims_data(&[
    Constraint::Ratio(1, 9),
    Constraint::Ratio(1, 9),
    Constraint::Ratio(1, 9),
], [
    "┌────────────────────────────┐",
    "│HeaHeaHea                   │",
    "│                            │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│RowRowRow                   │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::three(&[Constraint::Ratio(1, 3), Constraint::Ratio(1, 3), Constraint::Ratio(1, 3)], [
    "┌────────────────────────────┐",
    "│Head1    Head2     Head3    │",
    "│                            │",
    "│Row11    Row12     Row13    │",
    "│Row21    Row22     Row23    │",
    "│Row31    Row32     Row33    │",
    "│Row41    Row42     Row43    │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
#[case::two(&[Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)], [
    "┌────────────────────────────┐",
    "│Head1         Head2         │",
    "│                            │",
    "│Row11         Row12         │",
    "│Row21         Row22         │",
    "│Row31         Row32         │",
    "│Row41         Row42         │",
    "│                            │",
    "│                            │",
    "└────────────────────────────┘",
])]
fn widgets_table_columns_widths_can_use_ratio_constraints<'line, Lines>(
    #[case] widths: &[Constraint],
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let backend = TestBackend::new(30, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                ],
                widths,
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .column_spacing(0);
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::none(
    None,
    [
        "┌────────────────────────────┐",
        "│Head1 Head2 Head3           │",
        "│                            │",
        "│Row11 Row12 Row13           │",
        "│Row21 Row22 Row23           │",
        "│                            │",
        "│Row31 Row32 Row33           │",
        "└────────────────────────────┘",
    ],
)]
#[case::first(
    Some(0),
    [
        "┌────────────────────────────┐",
        "│   Head1 Head2 Head3        │",
        "│                            │",
        "│>> Row11 Row12 Row13        │",
        "│   Row21 Row22 Row23        │",
        "│                            │",
        "│   Row31 Row32 Row33        │",
        "└────────────────────────────┘",
    ],
)]
#[case::second_no_partially_fourth(
    Some(1),
    [
        "┌────────────────────────────┐",
        "│   Head1 Head2 Head3        │",
        "│                            │",
        "│   Row11 Row12 Row13        │",
        "│>> Row21 Row22 Row23        │",
        "│                            │",
        "│   Row31 Row32 Row33        │",
        "└────────────────────────────┘",
    ],
)]
#[case::fourth_no_partially_first(
    Some(3),
    [
        "┌────────────────────────────┐",
        "│   Head1 Head2 Head3        │",
        "│                            │",
        "│   Row31 Row32 Row33        │",
        "│>> Row41 Row42 Row43        │",
        "│                            │",
        "│                            │",
        "└────────────────────────────┘",
    ],
)]
fn widgets_table_can_have_rows_with_multi_lines<'line, Lines>(
    #[case] selected: Option<usize>,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let mut state = TableState::new().with_selected(selected);
    let backend = TestBackend::new(30, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]).height(2),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]).height(2),
                ],
                [
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .highlight_symbol(">> ")
            .column_spacing(1);
            f.render_stateful_widget(table, f.area(), &mut state);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[rstest]
#[case::none_when_selected(None, HighlightSpacing::WhenSelected, [
    "┌────────────────────────────┐",
    "│Head1 Head2 Head3           │",
    "│                            │",
    "│Row11 Row12 Row13           │",
    "│Row21 Row22 Row23           │",
    "│                            │",
    "│Row31 Row32 Row33           │",
    "└────────────────────────────┘",
])]
#[case::none_always(
    None, HighlightSpacing::Always, [
    "┌────────────────────────────┐",
    "│   Head1 Head2 Head3        │",
    "│                            │",
    "│   Row11 Row12 Row13        │",
    "│   Row21 Row22 Row23        │",
    "│                            │",
    "│   Row31 Row32 Row33        │",
    "└────────────────────────────┘",
])]
#[case::none_never(None, HighlightSpacing::Never, [
    "┌────────────────────────────┐",
    "│Head1 Head2 Head3           │",
    "│                            │",
    "│Row11 Row12 Row13           │",
    "│Row21 Row22 Row23           │",
    "│                            │",
    "│Row31 Row32 Row33           │",
    "└────────────────────────────┘",
])]
#[case::first_when_selected(Some(0), HighlightSpacing::WhenSelected, [
    "┌────────────────────────────┐",
    "│   Head1 Head2 Head3        │",
    "│                            │",
    "│>> Row11 Row12 Row13        │",
    "│   Row21 Row22 Row23        │",
    "│                            │",
    "│   Row31 Row32 Row33        │",
    "└────────────────────────────┘",
])]
#[case::first_always(Some(0), HighlightSpacing::Always, [
    "┌────────────────────────────┐",
    "│   Head1 Head2 Head3        │",
    "│                            │",
    "│>> Row11 Row12 Row13        │",
    "│   Row21 Row22 Row23        │",
    "│                            │",
    "│   Row31 Row32 Row33        │",
    "└────────────────────────────┘",
])]
#[case::first_never(Some(0), HighlightSpacing::Never, [
    "┌────────────────────────────┐",
    "│Head1 Head2 Head3           │",
    "│                            │",
    "│Row11 Row12 Row13           │",
    "│Row21 Row22 Row23           │",
    "│                            │",
    "│Row31 Row32 Row33           │",
    "└────────────────────────────┘",
])]
fn widgets_table_enable_always_highlight_spacing<'line, Lines>(
    #[case] selected: Option<usize>,
    #[case] space: HighlightSpacing,
    #[case] expected: Lines,
) where
    Lines: IntoIterator,
    Lines::Item: Into<Line<'line>>,
{
    let mut state = TableState::new().with_selected(selected);
    let backend = TestBackend::new(30, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]).height(2),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]).height(2),
                ],
                [
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .highlight_symbol(">> ")
            .highlight_spacing(space)
            .column_spacing(1);
            f.render_stateful_widget(table, f.area(), &mut state);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines(expected);
}

#[test]
fn widgets_table_can_have_elements_styled_individually() {
    let backend = TestBackend::new(30, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = TableState::default();
    state.select(Some(0));
    state.select_column(Some(1));
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row11", "Row12", "Row13"])
                        .style(Style::default().fg(Color::Green)),
                    Row::new(vec![
                        Cell::from("Row21"),
                        Cell::from("Row22").style(Style::default().fg(Color::Yellow)),
                        Cell::from(Line::from(vec![
                            Span::raw("Row"),
                            Span::styled("23", Style::default().fg(Color::Blue)),
                        ]))
                        .style(Style::default().fg(Color::Red)),
                    ])
                    .style(Style::default().fg(Color::LightGreen)),
                ],
                [
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(6),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::new().borders(Borders::LEFT | Borders::RIGHT))
            .highlight_symbol(">> ")
            .row_highlight_style(Style::default().add_modifier(Modifier::BOLD))
            .column_highlight_style(Style::default().add_modifier(Modifier::ITALIC))
            .cell_highlight_style(Style::default().add_modifier(Modifier::DIM))
            .column_spacing(1);
            f.render_stateful_widget(table, f.area(), &mut state);
        })
        .unwrap();

    let mut expected = Buffer::with_lines([
        "│   Head1  Head2  Head3      │",
        "│                            │",
        "│>> Row11  Row12  Row13      │",
        "│   Row21  Row22  Row23      │",
    ]);
    // First row = row color + highlight style
    for col in 1..=28 {
        expected[(col, 2)].set_style(
            Style::default()
                .fg(Color::Green)
                .add_modifier(Modifier::BOLD),
        );
    }

    // Second column highlight style
    for row in 2..=3 {
        for col in 11..=16 {
            expected[(col, row)].set_style(Style::default().add_modifier(Modifier::ITALIC));
        }
    }

    // First row, second column highlight style (cell highlight)
    for col in 11..=16 {
        expected[(col, 2)].set_style(Style::default().add_modifier(Modifier::DIM));
    }

    // Second row:
    // 1. row color
    for col in 1..=28 {
        expected[(col, 3)].set_style(Style::default().fg(Color::LightGreen));
    }
    // 2. cell color
    for col in 11..=16 {
        expected[(col, 3)].set_style(Style::default().fg(Color::Yellow));
    }
    for col in 18..=23 {
        expected[(col, 3)].set_style(Style::default().fg(Color::Red));
    }
    // 3. text color
    for col in 21..=22 {
        expected[(col, 3)].set_style(Style::default().fg(Color::Blue));
    }
    terminal.backend().assert_buffer(&expected);
}

#[test]
fn widgets_table_should_render_even_if_empty() {
    let backend = TestBackend::new(30, 4);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| {
            let table = Table::new(
                Vec::<Row>::new(),
                [
                    Constraint::Length(6),
                    Constraint::Length(6),
                    Constraint::Length(6),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]))
            .block(Block::new().borders(Borders::LEFT | Borders::RIGHT))
            .column_spacing(1);
            f.render_widget(table, f.area());
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        "│Head1  Head2  Head3         │",
        "│                            │",
        "│                            │",
        "│                            │",
    ]);
}

// based on https://github.com/fdehau/tui-rs/issues/470#issuecomment-852562848
#[test]
fn widgets_table_columns_dont_panic() {
    let table_width = 98;
    let table = Table::new(
        vec![Row::new(vec!["r1", "r2", "r3", "r4"])],
        [
            Constraint::Percentage(15),
            Constraint::Percentage(15),
            Constraint::Percentage(25),
            Constraint::Percentage(45),
        ],
    )
    .header(Row::new(vec!["h1", "h2", "h3", "h4"]))
    .block(Block::bordered())
    .highlight_symbol(">> ")
    .column_spacing(1);

    let mut state = TableState::default();

    // select first, which would cause a panic before fix
    state.select(Some(0));

    let backend = TestBackend::new(table_width, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|f| f.render_stateful_widget(table, f.area(), &mut state))
        .unwrap();
}

#[test]
fn widgets_table_should_clamp_offset_if_rows_are_removed() {
    let backend = TestBackend::new(30, 8);
    let mut terminal = Terminal::new(backend).unwrap();
    let mut state = TableState::default();

    // render with 6 items => offset will be at 2
    state.select(Some(5));
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![
                    Row::new(vec!["Row01", "Row02", "Row03"]),
                    Row::new(vec!["Row11", "Row12", "Row13"]),
                    Row::new(vec!["Row21", "Row22", "Row23"]),
                    Row::new(vec!["Row31", "Row32", "Row33"]),
                    Row::new(vec!["Row41", "Row42", "Row43"]),
                    Row::new(vec!["Row51", "Row52", "Row53"]),
                ],
                [
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .column_spacing(1);
            f.render_stateful_widget(table, f.area(), &mut state);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        "┌────────────────────────────┐",
        "│Head1 Head2 Head3           │",
        "│                            │",
        "│Row21 Row22 Row23           │",
        "│Row31 Row32 Row33           │",
        "│Row41 Row42 Row43           │",
        "│Row51 Row52 Row53           │",
        "└────────────────────────────┘",
    ]);

    // render with 1 item => offset will be at 1
    state.select(Some(1));
    terminal
        .draw(|f| {
            let table = Table::new(
                vec![Row::new(vec!["Row31", "Row32", "Row33"])],
                [
                    Constraint::Length(5),
                    Constraint::Length(5),
                    Constraint::Length(5),
                ],
            )
            .header(Row::new(vec!["Head1", "Head2", "Head3"]).bottom_margin(1))
            .block(Block::bordered())
            .column_spacing(1);
            f.render_stateful_widget(table, f.area(), &mut state);
        })
        .unwrap();
    terminal.backend().assert_buffer_lines([
        "┌────────────────────────────┐",
        "│Head1 Head2 Head3           │",
        "│                            │",
        "│Row31 Row32 Row33           │",
        "│                            │",
        "│                            │",
        "│                            │",
        "└────────────────────────────┘",
    ]);
}
