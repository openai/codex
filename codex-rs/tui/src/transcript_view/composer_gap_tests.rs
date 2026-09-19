//! Copy outcomes remain truthful at narrow widths and release stale navigation hit targets.

use super::*;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use pretty_assertions::assert_eq;

#[test]
fn copy_feedback_is_right_aligned_and_does_not_claim_terminal_delivery() {
    let mut snapshots = Vec::new();
    for result in [
        Ok(CopyStatus::Confirmed),
        Ok(CopyStatus::Unconfirmed),
        Err("unavailable".to_owned()),
    ] {
        for width in [80, 32, 18] {
            let mut view = TranscriptView::default();
            view.show_copy_feedback(&result, /*characters*/ 24);
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
            let mut buffer = Buffer::empty(area);
            assert!(
                view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer)
                    .is_some()
            );
            assert_eq!(buffer[(width - 1, 0)].symbol(), " ");
            snapshots.push(format!("{result:?}, {width} columns\n{}", text(&buffer)));
        }
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn feedback_releases_navigation_targets_and_expiry_restores_them() {
    let cells = vec![cell("one\ntwo\nthree\nfour\nfive")];
    let mut view = TranscriptView::default();
    render(&mut view, &cells, /*width*/ 60, /*height*/ 3);
    view.scroll(&cells, /*rows*/ -2);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 3, /*width*/ 60, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer);
    assert!(text(&buffer).contains("Back to bottom"));
    view.show_copy_feedback(&Ok(CopyStatus::Unconfirmed), /*characters*/ 3);
    buffer.reset();
    view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer);
    assert!(!text(&buffer).contains("Back to bottom"));
    assert!(!view.is_following());
    view.copy_feedback.as_mut().unwrap().expires_at = Instant::now();
    buffer.reset();
    assert_eq!(
        view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer),
        None
    );
    assert!(text(&buffer).contains("Back to bottom"));
}
