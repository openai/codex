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
    projection: &CellSelectionProjection,
    cell: usize,
    row: usize,
    column: u16,
) -> SelectionPoint {
    SelectionPoint {
        cell,
        bytes: projection
            .hit(row, column)
            .expect("coordinate should map to source text"),
    }
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
        selection.finish(/*point*/ None, &[Some(projection)]),
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
        selection.finish(/*point*/ None, &[Some(projection)]),
        Some("alpha\nbeta".to_string())
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
        selection.finish(/*point*/ None, &[Some(projection)]),
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
        selection.finish(/*point*/ None, &[Some(projection)]),
        Some("a\tb".to_string())
    );
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
    selection.update(SelectionPoint {
        cell: 0,
        bytes: projection
            .closest_hit(/*row*/ 0, /*column*/ 7)
            .expect("padding should clamp to the last rendered grapheme"),
    });

    assert_eq!(
        selection.finish(/*point*/ None, &[Some(projection)]),
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
    selection.update(SelectionPoint {
        cell: 0,
        bytes: projection
            .closest_hit(/*row*/ 1, /*column*/ 0)
            .expect("the prefix should clamp to the boundary before beta"),
    });

    assert_eq!(
        selection.finish(/*point*/ None, &[Some(projection)]),
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
        selection.finish(/*point*/ None, &[Some(first), Some(second)]),
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
        selection.finish(/*point*/ None, &[Some(first), Some(second)]),
        Some("first\nsecond".to_string())
    );
}

#[test]
fn selection_does_not_silently_skip_an_unsupported_cell() {
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
        selection.finish(/*point*/ None, &[Some(first), None, Some(third)],),
        None
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

    assert_eq!(selection.finish(/*point*/ None, &[Some(projection)]), None);
}
