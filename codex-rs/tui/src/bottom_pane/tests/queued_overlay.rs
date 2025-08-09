
#[test]
fn queued_list_renders_below_working_above_composer() {
    let (tx_raw, _rx) = channel::<AppEvent>();
    let tx = AppEventSender::new(tx_raw);
    let mut pane = BottomPane::new(BottomPaneParams {
        app_event_tx: tx,
        has_input_focus: true,
        enhanced_keys_supported: false,
        allow_input_while_running: true, // overlay mode
    });

    // Status overlay only (no live ring), then a queued list with two items.
    pane.set_task_running(true);
    pane.update_status_text("waiting for model".to_string());
    pane.set_queued_list_rows(vec![
        Line::from("  ⎿ ⏳ queued1"),
        Line::from("    ⏳ queued2"),
    ]);

    // Render: expect status at row0, queued rows at row1-2, spacer at row3, then composer.
    let area = Rect::new(0, 0, 50, 6);
    let mut buf = Buffer::empty(area);
    (&pane).render_ref(area, &mut buf);

    let row0: String = (0..area.width)
        .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(row0.contains("Working"), "row0 should be Working: {row0:?}");

    let row1: String = (0..area.width)
        .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row1.contains("⏳"),
        "row1 should show first queued item: {row1:?}"
    );

    let row2: String = (0..area.width)
        .map(|x| buf[(x, 2)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row2.contains("queued2"),
        "row2 should show second queued item: {row2:?}"
    );

    let row3: String = (0..area.width)
        .map(|x| buf[(x, 3)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        row3.trim().is_empty(),
        "row3 should be spacer above composer: {row3:?}"
    );

    // Composer content should appear on the next row (not Working and not queued).
    let row4: String = (0..area.width)
        .map(|x| buf[(x, 4)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(
        !row4.contains("Working") && !row4.contains("⏳"),
        "row4 should be composer (not Working/queued): {row4:?}"
    );
}

#[test]
fn queued_list_clear_hides_overlay() {
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
    pane.set_queued_list_rows(vec![Line::from("  ⎿ ⏳ queued1")]);

    // Now clear the queued overlay and ensure the line after Working is not a queued item.
    pane.clear_queued_list();

    let area = Rect::new(0, 0, 50, 5);
    let mut buf = Buffer::empty(area);
    (&pane).render_ref(area, &mut buf);

    let row0: String = (0..area.width)
        .map(|x| buf[(x, 0)].symbol().chars().next().unwrap_or(' '))
        .collect();
    assert!(row0.contains("Working"));
    let row1: String = (0..area.width)
        .map(|x| buf[(x, 1)].symbol().chars().next().unwrap_or(' '))
        .collect();
    // Depending on height, row1 may be spacer or start of composer; in either
    // case it must not contain queued content.
    assert!(
        !row1.contains("⏳") && !row1.contains("queued1"),
        "queued overlay should be cleared: {row1:?}"
    );
}
