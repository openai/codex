use super::*;

#[tokio::test]
async fn chained_config_error_wraps_in_history_snapshot() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.add_error_message(
        "Failed to save default model: config/batchWrite failed in TUI: Invalid configuration: features.fast_mode=true is not supported; allowed set [fast_mode=false]"
            .to_string(),
    );

    let width = 56;
    let height = 8;
    let backend = VT100Backend::new(width, height);
    let mut term = crate::custom_terminal::Terminal::with_options(backend).expect("terminal");
    term.set_viewport_area(ratatui::layout::Rect::new(0, 0, width, height));
    for lines in drain_insert_history(&mut rx) {
        crate::insert_history::insert_history_lines(&mut term, lines)
            .expect("insert history lines");
    }

    assert_chatwidget_snapshot!(
        "chained_config_error_wraps_in_history_snapshot",
        normalize_snapshot_paths(term.backend().vt100().screen().contents())
    );
}
