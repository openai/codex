use std::time::Duration;

use pretty_assertions::assert_eq;
use ratatui::layout::Position;
use ratatui::layout::Rect;

use super::*;

const AREA: Rect = Rect::new(
    /*x*/ 10, /*y*/ 20, /*width*/ 40, /*height*/ 20,
);

fn advance_ms(autoscroll: &mut SelectionAutoscroll, millis: u64) -> Option<AutoscrollStep> {
    autoscroll.advance(Duration::from_millis(millis))
}

#[test]
fn edge_direction_and_speed_match_opentui() {
    let cases = [
        (20, Some((AutoscrollDirection::Up, 36))),
        (22, Some((AutoscrollDirection::Up, 18))),
        (23, Some((AutoscrollDirection::Up, 3))),
        (24, None),
        (36, None),
        (37, Some((AutoscrollDirection::Down, 3))),
        (38, Some((AutoscrollDirection::Down, 18))),
        (39, Some((AutoscrollDirection::Down, 36))),
    ];
    for (y, expected) in cases {
        let pointer = Position::new(/*x*/ 30, y);
        let mut autoscroll = SelectionAutoscroll::default();
        autoscroll.update_pointer(AREA, pointer);
        assert_eq!(
            advance_ms(&mut autoscroll, /*millis*/ 500),
            expected.map(|(direction, rows)| AutoscrollStep {
                direction,
                rows,
                pointer,
            })
        );
    }

    let pointer = Position::new(/*x*/ 55, /*y*/ 7);
    let mut outside = SelectionAutoscroll::default();
    outside.update_pointer(AREA, pointer);
    assert_eq!(outside.pointer(), Some(pointer));
    assert_eq!(
        advance_ms(&mut outside, /*millis*/ 500),
        Some(AutoscrollStep {
            direction: AutoscrollDirection::Up,
            rows: 36,
            pointer,
        })
    );

    let short_area = Rect::new(
        /*x*/ 10, /*y*/ 20, /*width*/ 40, /*height*/ 3,
    );
    let mut short = SelectionAutoscroll::default();
    short.update_pointer(short_area, Position::new(/*x*/ 30, /*y*/ 22));
    assert_eq!(
        advance_ms(&mut short, /*millis*/ 500).map(|step| step.direction),
        Some(AutoscrollDirection::Down)
    );
}

#[test]
fn fractional_motion_stops_resets_and_changes_direction() {
    let top = Position::new(/*x*/ 30, /*y*/ 23);
    let bottom = Position::new(/*x*/ 30, /*y*/ 37);
    let mut autoscroll = SelectionAutoscroll::default();
    autoscroll.update_pointer(AREA, top);
    assert_eq!(advance_ms(&mut autoscroll, /*millis*/ 100), None);
    assert_eq!(
        advance_ms(&mut autoscroll, /*millis*/ 67).map(|step| step.rows),
        Some(1)
    );

    autoscroll.update_pointer(AREA, bottom);
    assert_eq!(advance_ms(&mut autoscroll, /*millis*/ 100), None);
    assert_eq!(
        advance_ms(&mut autoscroll, /*millis*/ 150).map(|step| step.direction),
        Some(AutoscrollDirection::Down)
    );

    autoscroll.update_pointer(AREA, Position::new(/*x*/ 30, /*y*/ 30));
    assert!(!autoscroll.needs_frame());
    autoscroll.update_pointer(AREA, top);
    assert_eq!(advance_ms(&mut autoscroll, /*millis*/ 100), None);
    autoscroll.reset();
    assert_eq!(autoscroll.pointer(), None);
    assert_eq!(autoscroll.advance(Duration::from_secs(/*secs*/ 1)), None);
}
