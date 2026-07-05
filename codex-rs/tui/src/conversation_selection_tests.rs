use pretty_assertions::assert_eq;
use ratatui::text::Line;

use super::*;

fn projection(
    text: &str,
    rendered: impl IntoIterator<Item = &'static str>,
    width: u16,
    prefix_columns: u16,
) -> CellSelectionProjection {
    CellSelectionProjection::from_display_lines(
        text.to_string(),
        rendered.into_iter().map(Line::from).collect(),
        width,
        prefix_columns,
    )
    .expect("projection should contain selectable text")
}

fn point(
    _projection: &CellSelectionProjection,
    cell: usize,
    row: usize,
    column: u16,
) -> SelectionPoint {
    SelectionPoint {
        row: cell.saturating_add(row),
        column,
    }
}

fn finish(
    selection: &mut ConversationSelection,
    projections: Vec<Option<CellSelectionProjection>>,
) -> Option<String> {
    let mut top = 0;
    let layout = projections
        .iter()
        .map(|projection| {
            let height = projection
                .as_ref()
                .map(|projection| projection.rows().len())
                .unwrap_or(1);
            let cell = SelectionCellLayout { top, height };
            top = top.saturating_add(height);
            cell
        })
        .collect::<Vec<_>>();
    selection.finish(/*point*/ None, &projections, &layout)
}

#[test]
fn soft_wrapped_selection_returns_original_space_without_newline() {
    let projection = projection(
        "alpha beta",
        ["› alpha", "  beta"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 2,
    ));
    selection.update(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 1,
        /*column*/ 5,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha beta".to_string())
    );
}

#[test]
fn authored_newline_is_preserved_across_rendered_rows() {
    let projection = projection(
        "alpha\nbeta",
        ["› alpha", "  beta"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 2,
    ));
    selection.update(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 1,
        /*column*/ 5,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha\nbeta".to_string())
    );
}

#[test]
fn forward_selection_can_start_in_spaces_after_an_authored_newline() {
    let projection = projection(
        "alpha\n  beta",
        ["alpha", "  beta"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 0 });
    selection.update(SelectionPoint { row: 1, column: 5 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("  beta".to_string())
    );
}

#[test]
fn backward_selection_can_end_in_a_tab_after_an_authored_newline() {
    let projection = projection(
        "alpha\n\tbeta",
        ["alpha", "    beta"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 7 });
    selection.update(SelectionPoint { row: 1, column: 2 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("\tbeta".to_string())
    );
}

#[test]
fn backward_drag_normalizes_to_the_same_source_range() {
    let projection = projection(
        "alpha beta",
        ["› alpha", "  beta"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 1,
        /*column*/ 5,
    ));
    selection.update(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 2,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha beta".to_string())
    );
}

#[test]
fn tabs_are_copied_from_source_instead_of_visual_spaces() {
    let projection = projection(
        "a\tb",
        ["a    b"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 0,
    ));
    selection.update(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 5,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("a\tb".to_string())
    );
}

#[test]
fn every_visual_column_of_a_tab_expansion_maps_to_the_authored_tab() {
    let projection = projection(
        "a\tb",
        ["a    b"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );

    for column in 1..5 {
        assert_eq!(
            projection.hit(/*row*/ 0, column),
            Some(1..2),
            "tab expansion column {column} should select the tab byte"
        );
    }
}

#[test]
fn wide_and_combining_graphemes_map_as_complete_utf8_ranges() {
    let projection = projection(
        "界e\u{301}🙂",
        ["界é🙂"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );

    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 0), Some(0.."界".len()));
    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 1), Some(0.."界".len()));
    assert_eq!(
        projection.hit(/*row*/ 0, /*column*/ 2),
        Some("界".len().."界e\u{301}".len())
    );
}

#[test]
fn presentation_prefix_and_padding_have_no_source_mapping() {
    let projection = projection(
        "hello",
        ["› hello"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );

    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 0), None);
    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 1), None);
    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 8), None);
}

#[test]
fn per_line_prefixes_exclude_each_lines_chrome() {
    let projection = CellSelectionProjection::from_display_lines_with_prefixes(
        "alpha beta".to_string(),
        vec!["› alpha".into(), "    beta".into()],
        /*width*/ 20,
        /*prefix_columns*/ &[2, 4],
    )
    .expect("projection should contain selectable text");

    assert_eq!(projection.hit(/*row*/ 1, /*column*/ 3), None);
    assert_eq!(
        projection.hit(/*row*/ 1, /*column*/ 4),
        Some("alpha ".len().."alpha b".len())
    );
}

#[test]
fn per_line_prefixes_require_one_width_per_display_line() {
    assert_eq!(
        CellSelectionProjection::from_display_lines_with_prefixes(
            "text".to_string(),
            vec!["text".into()],
            /*width*/ 20,
            /*prefix_columns*/ &[],
        ),
        None
    );
}

#[test]
fn selection_can_begin_before_the_first_glyph() {
    let projection = projection(
        "hello",
        ["› hello"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 0, column: 0 });
    selection.update(SelectionPoint { row: 0, column: 19 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("hello".to_string())
    );
}

#[test]
fn backward_selection_can_begin_after_the_last_glyph() {
    let projection = projection(
        "hello",
        ["› hello"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 0, column: 19 });
    selection.update(SelectionPoint { row: 0, column: 3 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("ello".to_string())
    );
}

#[test]
fn authored_blank_row_maps_to_its_semantic_boundary() {
    let projection = projection(
        "alpha\n\nbeta",
        ["alpha", "", "beta"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 10 });
    selection.update(SelectionPoint { row: 2, column: 3 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("\nbeta".to_string())
    );
}

#[test]
fn horizontal_drag_on_an_authored_blank_row_copies_nothing() {
    let projection = projection(
        "alpha\n\nbeta",
        ["alpha", "", "beta"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 0 });
    selection.update(SelectionPoint { row: 1, column: 19 });

    assert_eq!(finish(&mut selection, vec![Some(projection)]), None);
}

#[test]
fn blank_row_between_non_monotonic_segments_uses_a_safe_visual_boundary() {
    let projection = CellSelectionProjection::from_rows(
        "abcd".to_string(),
        vec![
            SelectionRow {
                segments: vec![SelectionSegment {
                    columns: 0..1,
                    bytes: 2..3,
                }],
            },
            SelectionRow::default(),
            SelectionRow {
                segments: vec![SelectionSegment {
                    columns: 0..1,
                    bytes: 0..1,
                }],
            },
        ],
    )
    .expect("non-monotonic projection should be valid");
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 10 });
    selection.update(SelectionPoint { row: 2, column: 0 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("a".to_string())
    );
}

#[test]
fn selection_ending_on_a_blank_row_stops_at_that_rows_start() {
    let projection = projection(
        "alpha\n\nbeta",
        ["alpha", "", "beta"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 0, column: 0 });
    selection.update(SelectionPoint { row: 1, column: 19 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha\n".to_string())
    );
}

#[test]
fn leading_and_trailing_blank_rows_preserve_authored_newlines() {
    let leading = projection(
        "\nalpha",
        ["", "alpha"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut leading_selection = ConversationSelection::default();
    leading_selection.start(SelectionPoint { row: 0, column: 10 });
    leading_selection.update(SelectionPoint { row: 1, column: 4 });
    assert_eq!(
        finish(&mut leading_selection, vec![Some(leading)]),
        Some("\nalpha".to_string())
    );

    let trailing = projection(
        "alpha\n",
        ["alpha", ""],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut trailing_selection = ConversationSelection::default();
    trailing_selection.start(SelectionPoint { row: 0, column: 0 });
    trailing_selection.update(SelectionPoint { row: 1, column: 10 });
    assert_eq!(
        finish(&mut trailing_selection, vec![Some(trailing)]),
        Some("alpha\n".to_string())
    );
}

#[test]
fn selection_can_begin_in_an_inter_cell_gap() {
    let first = projection(
        "first",
        ["first"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let second = projection(
        "second",
        ["second"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let layout = [
        SelectionCellLayout { top: 0, height: 1 },
        SelectionCellLayout { top: 2, height: 1 },
    ];
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 1, column: 10 });
    selection.update(SelectionPoint { row: 2, column: 5 });

    assert_eq!(
        selection.finish(/*point*/ None, &[Some(first), Some(second)], &layout),
        Some("second".to_string())
    );
}

#[test]
fn backward_selection_can_begin_below_the_content() {
    let projection = projection(
        "hello",
        ["hello"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let layout = [SelectionCellLayout { top: 0, height: 1 }];
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint {
        row: 10,
        column: 19,
    });
    selection.update(SelectionPoint { row: 0, column: 0 });

    assert_eq!(
        selection.finish(/*point*/ None, &[Some(projection)], &layout),
        Some("hello".to_string())
    );
}

#[test]
fn drag_entirely_within_padding_copies_nothing() {
    let projection = projection(
        "hello",
        ["hello"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(SelectionPoint { row: 0, column: 10 });
    selection.update(SelectionPoint { row: 0, column: 19 });

    assert_eq!(finish(&mut selection, vec![Some(projection)]), None);
}

#[test]
fn soft_wrap_padding_does_not_consume_an_invisible_source_space() {
    let projection = projection(
        "alpha beta",
        ["alpha beta"],
        /*width*/ 8,
        /*prefix_columns*/ 0,
    );
    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 5), None);

    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 0,
    ));
    selection.update(SelectionPoint { row: 0, column: 7 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha".to_string())
    );
}

#[test]
fn clamping_to_a_wrapped_prefix_stops_before_the_first_source_glyph() {
    let projection = projection(
        "alpha beta",
        ["› alpha", "  beta"],
        /*width*/ 20,
        /*prefix_columns*/ 2,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 2,
    ));
    selection.update(SelectionPoint { row: 1, column: 0 });

    assert_eq!(
        finish(&mut selection, vec![Some(projection)]),
        Some("alpha ".to_string())
    );
}

#[test]
fn wide_grapheme_continuation_does_not_consume_following_space() {
    let projection = projection(
        "界 x",
        ["界 x"],
        /*width*/ 8,
        /*prefix_columns*/ 0,
    );

    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 0), Some(0.."界".len()));
    assert_eq!(projection.hit(/*row*/ 0, /*column*/ 1), Some(0.."界".len()));
    assert_eq!(
        projection.hit(/*row*/ 0, /*column*/ 2),
        Some("界".len().."界 ".len())
    );
    assert_eq!(
        projection.hit(/*row*/ 0, /*column*/ 3),
        Some("界 ".len().."界 x".len())
    );
}

#[test]
fn cross_cell_selection_uses_explicit_message_separator() {
    let first = projection(
        "first",
        ["first"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let second = projection(
        "second",
        ["second"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &first, /*cell*/ 0, /*row*/ 0, /*column*/ 0,
    ));
    selection.update(point(
        &second, /*cell*/ 1, /*row*/ 0, /*column*/ 5,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(first), Some(second)]),
        Some("first\n\nsecond".to_string())
    );
}

#[test]
fn stream_continuation_uses_a_single_authored_line_break() {
    let first = projection(
        "first",
        ["first"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let second = projection(
        "second",
        ["second"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    )
    .with_separator_before("\n");
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &first, /*cell*/ 0, /*row*/ 0, /*column*/ 0,
    ));
    selection.update(point(
        &second, /*cell*/ 1, /*row*/ 0, /*column*/ 5,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(first), Some(second)]),
        Some("first\nsecond".to_string())
    );
}

#[test]
fn source_fragment_separator_does_not_add_text_between_stream_chunks() {
    for (second_text, expected) in [(" beta", "alpha beta"), ("\nbeta", "alpha\nbeta")] {
        let first = projection(
            "alpha",
            ["alpha"],
            /*width*/ 20,
            /*prefix_columns*/ 0,
        );
        let second = projection(
            second_text,
            ["beta"],
            /*width*/ 20,
            /*prefix_columns*/ 0,
        )
        .with_separator_before("")
        .with_default_separator_before("\n");
        let mut selection = ConversationSelection::default();
        selection.start(point(
            &first, /*cell*/ 0, /*row*/ 0, /*column*/ 0,
        ));
        selection.update(point(
            &second, /*cell*/ 1, /*row*/ 0, /*column*/ 3,
        ));

        assert_eq!(
            finish(&mut selection, vec![Some(first), Some(second)]).as_deref(),
            Some(expected)
        );
    }
}

#[test]
fn transparent_cell_is_skipped_without_losing_the_next_separator() {
    let first = projection(
        "first",
        ["first"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let third = projection(
        "third",
        ["third"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &first, /*cell*/ 0, /*row*/ 0, /*column*/ 0,
    ));
    selection.update(point(
        &third, /*cell*/ 2, /*row*/ 0, /*column*/ 4,
    ));

    assert_eq!(
        finish(&mut selection, vec![Some(first), None, Some(third)]),
        Some("first\n\nthird".to_string())
    );
}

#[test]
fn click_without_drag_does_not_copy() {
    let projection = projection(
        "hello",
        ["hello"],
        /*width*/ 20,
        /*prefix_columns*/ 0,
    );
    let mut selection = ConversationSelection::default();
    selection.start(point(
        &projection,
        /*cell*/ 0,
        /*row*/ 0,
        /*column*/ 0,
    ));

    assert_eq!(finish(&mut selection, vec![Some(projection)]), None);
}
