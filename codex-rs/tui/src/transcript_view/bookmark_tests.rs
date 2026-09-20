use super::*;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use pretty_assertions::assert_eq;

#[test]
fn cancel_restores_both_positions_after_resize_and_pagination() {
    let mut cells = vec![
        cell("first prompt\nfirst response"),
        cell("second prompt\nsecond response"),
    ];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 30, /*height*/ 3);
    view.jump_to_entry(&cells, /*index*/ 0);
    let before = text(&render(
        &mut view, &cells, /*width*/ 30, /*height*/ 3,
    ));
    let positions = (view.position, view.saved_position);
    let bookmark = view.bookmark(&cells);
    view.set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
    view.jump_to_entry(&cells, /*index*/ 1);
    render(&mut view, &cells, /*width*/ 12, /*height*/ 4);
    cells.insert(/*index*/ 0, cell("older page"));
    cells.push(cell("new response"));
    view.restore_bookmark(bookmark);
    assert_eq!((view.position, view.saved_position), positions);
    assert_eq!(view.is_detailed(), false);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 3
        )),
        before
    );
}

#[test]
fn following_origin_returns_to_new_output_in_original_presentation() {
    let mut cells = vec![cell("first prompt"), cell("second prompt")];
    let mut view = TranscriptView::default();
    view.set_presentation(/*detailed*/ true, HistoryRenderMode::Rich);
    let bookmark = view.bookmark(&cells);
    view.set_presentation(/*detailed*/ false, HistoryRenderMode::Rich);
    view.jump_to_entry(&cells, /*index*/ 0);
    cells.push(cell("new output"));
    view.restore_bookmark(bookmark);
    assert_eq!((view.is_detailed(), view.is_following()), (true, true));
    assert!(
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 2
        ))
        .contains("new output")
    );
}

#[test]
fn cancel_restores_a_group_replaced_during_browsing() {
    let mut cells = vec![cell("original group\noriginal output"), cell("next prompt")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 30, /*height*/ 2);
    view.jump_to_entry(&cells, /*index*/ 0);
    let before = text(&render(
        &mut view, &cells, /*width*/ 30, /*height*/ 2,
    ));
    let bookmark = view.bookmark(&cells);
    view.jump_to_latest();
    cells[0] = cell("replacement group\nreplacement output");
    view.restore_bookmark(bookmark);
    assert_eq!(
        text(&render(
            &mut view, &cells, /*width*/ 30, /*height*/ 2
        )),
        before
    );
}
