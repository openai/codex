use crate::app_event::AppEvent;
use crate::app_event_sender::AppEventSender;
use crate::bottom_pane::{BottomPane, BottomPaneParams};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::Line;
use ratatui::widgets::WidgetRef;
use std::sync::mpsc::channel;

#[test]
fn overlay_status_has_blank_above_when_live_ring_present() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true, // overlay mode
    });

    // Live status overlay (Working) is shown while running.
    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());

    // Simulate a live ring with two streamed rows.
    pane.set_live_ring_rows(2, vec![Line::from("stream1"), Line::from("stream2")]);

    // Render enough height to include ring (2), spacer (1), status (1), and a couple more.
    let area = Rect::new(0, 0, 40, 6);
    let mut buf = Buffer::empty(area);
    (&pane).render_ref(area, &mut buf);

    // Row 0..1 are the live ring content.
    let row0: String = (0..area.width)
        .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    let row1: String = (0..area.width)
        .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(row0.contains("stream1"));
    assert!(row1.contains("stream2"));

    // Row 2 should be a blank spacer between ring and Working.
    let spacer: String = (0..area.width)
        .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        spacer.trim().is_empty(),
        "expected blank spacer line above Working: {spacer:?}"
    );

    // Row 3 should contain the Working header.
    let row3: String = (0..area.width)
        .map(|x| buf[(x, 3)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row3.contains("Working"),
        "expected Working header after spacer: {row3:?}"
    );
}

#[test]
fn overlay_status_has_no_blank_above_when_no_live_ring() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true, // overlay mode
    });

    // Only the status overlay should be present.
    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());

    // Render a small area; the first row should be Working (no blank above it).
    let area = Rect::new(0, 0, 40, 3);
    let mut buf = Buffer::empty(area);
    (&pane).render_ref(area, &mut buf);

    // Top row contains the Working header (no leading blank line).
    let top: String = (0..area.width)
        .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        top.contains("Working"),
        "expected Working header on top row: {top:?}"
    );
}
