//! Exercise copy entry through the app dispatcher, including an empty transcript.

use super::*;

#[tokio::test]
async fn empty_owned_copy_reports_feedback() -> Result<()> {
    let (mut app, mut rx, _op_rx) = make_test_app_with_channels().await;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    let guard = Arc::new(crate::copy_input_guard::CopyInputGuard(
        app.app_event_tx.clone(),
    ));
    app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::SelectTranscriptCopy { guard },
    )
    .await?;
    let lines = std::iter::from_fn(|| rx.try_recv().ok())
        .filter_map(|event| match event {
            AppEvent::InsertHistoryCell(cell) => Some(cell.display_lines(/*width*/ 80)),
            _ => None,
        })
        .flatten()
        .map(|line| line.to_string())
        .collect::<Vec<_>>()
        .join("\n");
    insta::assert_snapshot!(lines, @"• Nothing to copy");
    Ok(())
}
