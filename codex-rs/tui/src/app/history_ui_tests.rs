//! Desktop handoff output and clear-screen history reset behavior.

use super::*;
use crate::app::tests::make_test_app_with_channels;
use crate::history_cell;
use crate::history_cell::HistoryCell;
use crate::history_cell::PlainHistoryCell;
use pretty_assertions::assert_eq;

#[test]
fn desktop_thread_opened_history_snapshot() {
    let cell = history_cell::new_info_event(
        DESKTOP_THREAD_OPENED_MESSAGE.to_string(),
        /*hint*/ None,
    );

    insta::assert_snapshot!("desktop_thread_opened_history", render_cell(&cell));
}

#[test]
fn desktop_thread_open_error_history_snapshot() {
    let cell = history_cell::new_error_event(desktop_thread_open_error_message("launch failed"));

    insta::assert_snapshot!("desktop_thread_open_error_history", render_cell(&cell));
}

fn render_cell(cell: &impl HistoryCell) -> String {
    let lines = cell.display_lines(/*width*/ 80);
    lines
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n")
}

#[tokio::test]
async fn viewed_image_links_follow_workspace_locality() -> Result<()> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("frame.png");
    image::RgbaImage::new(1, 1).save(&path)?;
    let endpoint = crate::resolve_remote_addr("ws://127.0.0.1:4500")?;
    let (mut app, _, _) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let local_links = !crate::clipboard_copy::is_ssh_session();
    for (target, linked) in [
        (AppServerTarget::Embedded, local_links),
        (
            AppServerTarget::LocalDaemon {
                endpoint: endpoint.clone(),
                allow_embedded_fallback: false,
            },
            local_links,
        ),
        (AppServerTarget::Remote { endpoint }, false),
    ] {
        app.app_server_target = target;
        let cell = history_cell::new_view_image_tool_call(
            "view".into(),
            codex_utils_path_uri::LegacyAppPathString::from_path(&path),
        );
        app.insert_history_cell(&mut tui, Box::new(cell));
        let rows = app
            .transcript_cells
            .last()
            .unwrap()
            .compact_hyperlink_lines(/*width*/ 80);
        assert_eq!(!rows[0].hyperlinks.is_empty(), linked);
    }
    Ok(())
}

#[tokio::test]
async fn owned_clear_resets_navigation_and_retains_one_fresh_header() -> Result<()> {
    let (mut app, _rx, _op_rx) = make_test_app_with_channels().await;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    tui.set_owned_screen(/*owned*/ true)?;
    app.insert_history_cell(
        &mut tui,
        Box::new(PlainHistoryCell::new(vec!["old".into()])),
    );
    app.transcript_view
        .jump_to_entry(&app.transcript_cells, /*index*/ 0);
    app.reset_transcript_state_after_clear();
    app.queue_clear_ui_header(&mut tui);
    app.queue_clear_ui_header(&mut tui);

    assert_eq!(app.transcript_cells.len(), 1);
    assert_eq!(
        app.transcript_cells[0].display_lines(/*width*/ 80),
        app.clear_ui_header_lines(/*width*/ 80),
    );
    app.chat_widget.set_raw_output_mode(/*enabled*/ true);
    assert_eq!(
        app.transcript_cells[0].raw_lines(),
        app.clear_ui_header_lines(/*width*/ 80),
    );
    assert!(app.transcript_view.is_following());
    assert!(tui.pending_history_lines_for_test().is_empty());
    tui.set_owned_screen(/*owned*/ false)?;
    Ok(())
}
