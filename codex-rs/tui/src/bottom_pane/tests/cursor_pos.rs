use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::{BottomPane, BottomPaneParams};
use ratatui::layout::Rect;
use ratatui::text::Line;
use std::sync::mpsc::channel;

#[test]
fn cursor_pos_accounts_for_live_ring_and_status_spacer() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true, // overlay mode keeps composer visible
    });

    // Begin running: show overlay status.
    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());
    // Provide a live ring with two rows (e.g., streaming content preview).
    pane.set_live_ring_rows(3, vec![Line::from("one"), Line::from("two")]);

    let area = Rect::new(0, 0, 60, 12);
    // Offsets: ring_h (2) + spacer (1) + status (1) + overlay→composer spacer (1) = 5
    let expected_offset_y = 5u16;
    let (_x, y) = pane
        .cursor_pos(area)
        .expect("cursor position should be available in overlay mode");
    assert_eq!(
        y, expected_offset_y,
        "cursor y should be offset by overlays"
    );
}

#[test]
fn cursor_pos_accounts_for_status_only_no_ring() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true,
    });

    // Running with status overlay only (no live ring).
    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());

    let area = Rect::new(0, 0, 60, 10);
    // Offsets: status height (1) + overlay→composer spacer (1) = 2
    let expected_offset_y = 2u16;
    let (_x, y) = pane
        .cursor_pos(area)
        .expect("cursor position should be available in overlay mode");
    assert_eq!(
        y, expected_offset_y,
        "cursor y should include status + spacer"
    );
}

#[test]
fn cursor_pos_accounts_for_queued_list() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true,
    });

    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());
    pane.set_queued_list_rows(vec![Line::from("  ⎿ ⏳ q1"), Line::from("    ⏳ q2")]);

    let area = Rect::new(0, 0, 60, 12);
    // Offsets: status (1) + queued (2) + overlay→composer spacer (1) = 4
    let expected_offset_y = 4u16;
    let (_x, y) = pane
        .cursor_pos(area)
        .expect("cursor position should be available");
    assert_eq!(
        y, expected_offset_y,
        "cursor y should include queued overlay"
    );
}
