use super::*;
use assert_matches::assert_matches;

#[tokio::test]
async fn status_command_renders_immediately_and_refreshes_rate_limits_for_chatgpt_auth() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected status output before refresh request, got {other:?}"),
    };
    assert!(
        rendered.contains("refreshing limits"),
        "expected /status to explain the background refresh, got: {rendered}"
    );
    assert_matches!(rx.try_recv(), Ok(AppEvent::RefreshRateLimits));
}

#[tokio::test]
async fn status_command_updates_rendered_cell_after_rate_limit_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    let cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected status output before refresh request, got {other:?}"),
    };
    assert_matches!(rx.try_recv(), Ok(AppEvent::RefreshRateLimits));

    let initial = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert!(
        initial.contains("refreshing limits"),
        "expected initial /status output to show refresh notice, got: {initial}"
    );

    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.finish_status_rate_limit_refresh();

    let updated = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert_ne!(
        initial, updated,
        "expected refreshed /status output to change"
    );
    assert!(
        !updated.contains("refreshing limits"),
        "expected refresh notice to clear after background update, got: {updated}"
    );
}

#[tokio::test]
async fn status_command_renders_immediately_without_rate_limit_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Status);

    assert_matches!(rx.try_recv(), Ok(AppEvent::InsertHistoryCell(_)));
    assert!(
        !std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::RefreshRateLimits)),
        "non-ChatGPT sessions should not request a rate-limit refresh for /status"
    );
}
