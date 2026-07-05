use pretty_assertions::assert_eq;

use super::*;

fn split_widths(layout: OwnedScreenLayout) -> (u16, u16, u16) {
    let OwnedScreenLayout::Split {
        parent,
        divider,
        side,
        ..
    } = layout
    else {
        panic!("expected split layout");
    };
    (parent.width, divider.x, side.width)
}

#[test]
fn layout_uses_threshold_parent_bias_and_nonzero_origin() {
    let preference = PaneSplitPreference::default();
    let narrow = Rect::new(
        /*x*/ 7, /*y*/ 3, /*width*/ 82, /*height*/ 20,
    );
    assert_eq!(
        OwnedScreenLayout::new(narrow, /*has_side*/ true, PaneSlot::Side, preference),
        OwnedScreenLayout::Single {
            slot: PaneSlot::Side,
            area: narrow,
            show_header: true,
        }
    );

    for (width, expected) in [(83, (41, 48, 41)), (84, (42, 49, 41))] {
        let area = Rect::new(/*x*/ 7, /*y*/ 3, width, /*height*/ 20);
        assert_eq!(
            split_widths(OwnedScreenLayout::new(
                area,
                /*has_side*/ true,
                PaneSlot::Parent,
                preference,
            )),
            expected
        );
    }
}

#[test]
fn drag_lifecycle_clamps_and_preserves_the_preferred_ratio() {
    let mut state = OwnedScreenSplitState::default();
    let initial_preference = state.preference();
    let wide = Rect::new(
        /*x*/ 10, /*y*/ 2, /*width*/ 120, /*height*/ 20,
    );
    state.record_layout(OwnedScreenLayout::new(
        wide,
        /*has_side*/ true,
        PaneSlot::Parent,
        state.preference(),
    ));
    assert!(!state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: 80,
        row: 4,
    }));
    assert!(!state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 20,
        row: 4,
    }));

    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 70,
        row: 4,
    }));
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: 10,
        row: 4,
    }));
    assert_eq!(
        split_widths(OwnedScreenLayout::new(
            wide,
            /*has_side*/ true,
            PaneSlot::Parent,
            state.preference(),
        )),
        (41, 51, 78)
    );
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Release,
        column: 70,
        row: 4,
    }));
    assert_eq!(state.preference(), initial_preference);

    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 70,
        row: 4,
    }));
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: 80,
        row: 4,
    }));
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Release,
        column: 80,
        row: u16::MAX,
    }));
    assert_eq!(
        split_widths(OwnedScreenLayout::new(
            wide,
            /*has_side*/ true,
            PaneSlot::Parent,
            state.preference(),
        )),
        (70, 80, 49)
    );
    assert!(!state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Drag,
        column: 70,
        row: 4,
    }));

    let preference = state.preference();
    let constrained = Rect::new(
        /*x*/ 10, /*y*/ 2, /*width*/ 90, /*height*/ 20,
    );
    let constrained_layout = OwnedScreenLayout::new(
        constrained,
        /*has_side*/ true,
        PaneSlot::Parent,
        preference,
    );
    assert_eq!(split_widths(constrained_layout), (48, 58, 41));
    state.record_layout(constrained_layout);
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 58,
        row: 4,
    }));
    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Release,
        column: 58,
        row: 4,
    }));
    assert_eq!(state.preference(), preference);

    assert!(state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 58,
        row: 4,
    }));
    let narrow = Rect::new(
        /*x*/ 10, /*y*/ 2, /*width*/ 82, /*height*/ 20,
    );
    state.record_layout(OwnedScreenLayout::new(
        narrow,
        /*has_side*/ true,
        PaneSlot::Parent,
        state.preference(),
    ));
    assert!(!state.is_dragging());
    assert_eq!(state.preference(), preference);
    assert_eq!(
        split_widths(OwnedScreenLayout::new(
            wide,
            /*has_side*/ true,
            PaneSlot::Parent,
            state.preference(),
        )),
        (70, 80, 49)
    );

    let fixed = Rect::new(
        /*x*/ 10,
        /*y*/ 2,
        MIN_SPLIT_WIDTH,
        /*height*/ 20,
    );
    state.record_layout(OwnedScreenLayout::new(
        fixed,
        /*has_side*/ true,
        PaneSlot::Parent,
        state.preference(),
    ));
    assert!(!state.handle_mouse(MousePrimaryEvent {
        kind: MousePrimaryEventKind::Press,
        column: 51,
        row: 4,
    }));
}
