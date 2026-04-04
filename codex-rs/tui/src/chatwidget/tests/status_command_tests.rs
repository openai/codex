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
    let request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits { request_id }) => request_id,
        other => panic!("expected rate-limit refresh request, got {other:?}"),
    };
    pretty_assertions::assert_eq!(request_id, 0);
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
    let first_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits { request_id }) => request_id,
        other => panic!("expected rate-limit refresh request, got {other:?}"),
    };

    let initial = lines_to_single_string(&cell.display_lines(/*width*/ 80));
    assert!(
        initial.contains("refreshing limits"),
        "expected initial /status output to show refresh notice, got: {initial}"
    );

    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.finish_status_rate_limit_refresh(first_request_id);

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
            .any(|event| matches!(event, AppEvent::RefreshRateLimits { .. })),
        "non-ChatGPT sessions should not request a rate-limit refresh for /status"
    );
}

#[tokio::test]
async fn status_command_overlapping_refreshes_update_matching_cells_only() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);
    let first_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected first status output, got {other:?}"),
    };
    let first_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits { request_id }) => request_id,
        other => panic!("expected first refresh request, got {other:?}"),
    };

    chat.dispatch_command(SlashCommand::Status);
    let second_cell = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => cell,
        other => panic!("expected second status output, got {other:?}"),
    };
    let second_request_id = match rx.try_recv() {
        Ok(AppEvent::RefreshRateLimits { request_id }) => request_id,
        other => panic!("expected second refresh request, got {other:?}"),
    };

    assert_ne!(first_request_id, second_request_id);

    chat.finish_status_rate_limit_refresh(first_request_id);

    let first_after_failure = lines_to_single_string(&first_cell.display_lines(/*width*/ 80));
    let second_still_refreshing = lines_to_single_string(&second_cell.display_lines(/*width*/ 80));
    assert!(
        !first_after_failure.contains("refreshing limits"),
        "expected first status cell to stop refreshing after its request completed, got: {first_after_failure}"
    );
    assert!(
        second_still_refreshing.contains("refreshing limits"),
        "expected later status cell to keep refreshing until its own request completes, got: {second_still_refreshing}"
    );

    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.finish_status_rate_limit_refresh(second_request_id);

    let second_after_success = lines_to_single_string(&second_cell.display_lines(/*width*/ 80));
    assert!(
        !second_after_success.contains("refreshing limits"),
        "expected second status cell to refresh once its own request completed, got: {second_after_success}"
    );
}
