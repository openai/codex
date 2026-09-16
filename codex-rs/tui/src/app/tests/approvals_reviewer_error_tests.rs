//! Verify that approvals persistence failures retain the backend's actionable cause.

use super::*;

#[tokio::test]
async fn approvals_reviewer_error_retains_config_cause() -> Result<()> {
    let (mut app, mut events, _op_rx) = make_test_app_with_channels().await;
    let codex_home = tempdir()?;
    app.config.codex_home = codex_home.path().to_path_buf().abs();
    let config_path = codex_home.path().join("config.toml");
    std::fs::write(&config_path, "")?;
    let mut app_server = start_config_write_test_app_server(&app).await?;
    let mut tui = crate::tui::test_support::make_test_tui()?;
    while events.try_recv().is_ok() {}

    // Simulate an external config edit after startup so the real batch-write
    // reload fails, rather than injecting an already-formatted UI error.
    std::fs::write(&config_path, "approvals_reviewer = [\n")?;
    Box::pin(app.handle_event(
        &mut tui,
        &mut app_server,
        AppEvent::UpdateApprovalsReviewer(ApprovalsReviewer::User),
    ))
    .await?;
    app_server.shutdown().await?;

    let cell = match events.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected the persistence error, got {other:?}"),
    };
    let rendered = lines_to_single_string(&cell.display_lines(/*width*/ 120)).replace(
        &config_path.to_string_lossy().to_string(),
        "<CODEX_HOME>/config.toml",
    );
    assert!(rendered.contains("unclosed array"), "{rendered}");
    insta::assert_snapshot!("approvals_reviewer_config_error", rendered);
    Ok(())
}
