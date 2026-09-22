//! Copy outcomes remain truthful at narrow widths and release stale navigation hit targets.

use super::*;
use crate::transcript_view::tests::cell;
use crate::transcript_view::tests::render;
use crate::transcript_view::tests::text;
use crossterm::event::KeyModifiers;
use crossterm::event::MouseButton;
use crossterm::event::MouseEvent;
use crossterm::event::MouseEventKind;
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
            assert!(view.composer_gap_has_content(width, /*hint*/ None, Instant::now()));
            let area = Rect::new(/*x*/ 0, /*y*/ 0, width, /*height*/ 1);
            let mut buffer = Buffer::empty(area);
            assert!(
                view.render_composer_gap(
                    Some(area),
                    /*hint*/ None,
                    &mut buffer,
                    Instant::now()
                )
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
    render(&mut view, &cells, /*width*/ 60, /*height*/ 3);
    let area = Rect::new(
        /*x*/ 0, /*y*/ 3, /*width*/ 60, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    assert!(view.composer_gap_has_content(area.width, /*hint*/ None, Instant::now()));
    view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer, Instant::now());
    assert!(text(&buffer).contains("Back to bottom"));
    view.show_copy_feedback(&Ok(CopyStatus::Unconfirmed), /*characters*/ 3);
    buffer.reset();
    view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer, Instant::now());
    assert!(!text(&buffer).contains("Back to bottom"));
    assert!(!view.is_following());
    view.copy_feedback.as_mut().unwrap().expires_at = Instant::now();
    buffer.reset();
    assert_eq!(
        view.render_composer_gap(Some(area), /*hint*/ None, &mut buffer, Instant::now()),
        None
    );
    assert!(text(&buffer).contains("Back to bottom"));
    view.jump_to_latest();
    render(&mut view, &cells, /*width*/ 60, /*height*/ 3);
    assert!(!view.composer_gap_has_content(area.width, /*hint*/ None, Instant::now()));
}

#[test]
fn tip_links_follow_alignment_and_release_stale_targets() {
    let destination = "https://example.com/docs";
    let mut tip = HyperlinkLine::from("Tip: ");
    tip.push_span("文档".into(), Some(destination));
    let mut view = TranscriptView::default();
    let area = Rect::new(
        /*x*/ 4, /*y*/ 3, /*width*/ 30, /*height*/ 1,
    );
    let mut buffer = Buffer::empty(area);
    let click = MouseEvent {
        kind: MouseEventKind::Down(MouseButton::Left),
        column: area.right() - 3,
        row: area.y,
        modifiers: KeyModifiers::CONTROL,
    };
    view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
    assert!(
        buffer[(click.column, click.row)]
            .symbol()
            .contains(destination)
    );
    for column in area.right() - 5..area.right() - 1 {
        for modifiers in [KeyModifiers::CONTROL, KeyModifiers::SUPER] {
            let Some(ViewAction::OpenLink(url)) = view.handle_mouse(
                MouseEvent {
                    column,
                    modifiers,
                    ..click
                },
                &[],
            ) else {
                panic!("both columns of each linked glyph should open its destination");
            };
            assert_eq!(url, destination);
        }
    }
    for event in [
        MouseEvent {
            modifiers: KeyModifiers::NONE,
            ..click
        },
        MouseEvent {
            kind: MouseEventKind::Up(MouseButton::Left),
            ..click
        },
        MouseEvent {
            column: area.right() - 6,
            ..click
        },
        MouseEvent {
            row: area.y + 1,
            ..click
        },
    ] {
        assert!(view.handle_mouse(event, &[]).is_none());
    }

    for replacement in [None, Some(HyperlinkLine::from("Other tip"))] {
        view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
        view.render_composer_gap(
            Some(area),
            replacement.as_ref(),
            &mut buffer,
            Instant::now(),
        );
        assert!(view.handle_mouse(click, &[]).is_none());
    }
    for next_area in [
        None,
        Some(Rect::new(
            /*x*/ 4, /*y*/ 3, /*width*/ 0, /*height*/ 1,
        )),
        Some(Rect::new(
            /*x*/ 4, /*y*/ 3, /*width*/ 8, /*height*/ 1,
        )),
        Some(Rect::new(
            /*x*/ 4, /*y*/ 3, /*width*/ 20, /*height*/ 1,
        )),
    ] {
        view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
        view.render_composer_gap(next_area, Some(&tip), &mut buffer, Instant::now());
        assert!(view.handle_mouse(click, &[]).is_none());
    }
    view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
    view.show_copy_feedback(&Ok(CopyStatus::Confirmed), /*characters*/ 3);
    view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
    assert!(view.handle_mouse(click, &[]).is_none());
    view.copy_feedback.as_mut().unwrap().expires_at = Instant::now();
    view.render_composer_gap(Some(area), Some(&tip), &mut buffer, Instant::now());
    assert!(matches!(
        view.handle_mouse(click, &[]),
        Some(ViewAction::OpenLink(_))
    ));
    view.render(area, &mut buffer, &[]);
    assert!(!matches!(
        view.handle_mouse(click, &[]),
        Some(ViewAction::OpenLink(_))
    ));
}
