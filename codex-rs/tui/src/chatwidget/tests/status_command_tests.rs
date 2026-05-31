use super::*;
use assert_matches::assert_matches;

fn render_requested_status(
    chat: &mut ChatWidget,
    rx: &mut tokio::sync::mpsc::UnboundedReceiver<AppEvent>,
) -> (String, Option<u64>) {
    let (refreshing_rate_limits, request_id) = match rx.try_recv() {
        Ok(AppEvent::ReadThreadWorkspaceForStatus {
            refreshing_rate_limits,
            request_id,
        }) => (refreshing_rate_limits, request_id),
        other => panic!("expected workspace read request for /status, got {other:?}"),
    };
    if let Some(request_id) = request_id {
        assert_matches!(
            rx.try_recv(),
            Ok(AppEvent::RefreshRateLimits {
                origin: RateLimitRefreshOrigin::StatusCommand {
                    request_id: event_request_id,
                },
            }) if event_request_id == request_id
        );
    }
    chat.add_status_output(refreshing_rate_limits, request_id);
    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 80))
        }
        other => panic!("expected status output after workspace read, got {other:?}"),
    };
    (rendered, request_id)
}

#[tokio::test]
async fn status_command_reads_workspace_and_refreshes_rate_limits_for_chatgpt_auth() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    let (rendered, request_id) = render_requested_status(&mut chat, &mut rx);
    assert!(
        !rendered.contains("refreshing limits"),
        "expected /status to avoid transient refresh text in terminal history, got: {rendered}"
    );
    pretty_assertions::assert_eq!(request_id, Some(0));
}

#[tokio::test]
async fn status_command_refresh_updates_cached_limits_for_future_status_outputs() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);

    let (_, first_request_id) = render_requested_status(&mut chat, &mut rx);
    let first_request_id = first_request_id.expect("ChatGPT status should refresh limits");

    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.finish_status_rate_limit_refresh(first_request_id);
    drain_insert_history(&mut rx);

    chat.dispatch_command(SlashCommand::Status);
    let (refreshed, _) = render_requested_status(&mut chat, &mut rx);
    assert!(
        refreshed.contains("8% left"),
        "expected a future /status output to use refreshed cached limits, got: {refreshed}"
    );
}

#[tokio::test]
async fn status_command_reads_workspace_without_rate_limit_refresh() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.dispatch_command(SlashCommand::Status);

    let (_, request_id) = render_requested_status(&mut chat, &mut rx);
    pretty_assertions::assert_eq!(request_id, None);
    assert!(
        !std::iter::from_fn(|| rx.try_recv().ok())
            .any(|event| matches!(event, AppEvent::RefreshRateLimits { .. })),
        "non-ChatGPT sessions should not request a rate-limit refresh for /status"
    );
}

#[tokio::test]
async fn status_command_uses_catalog_default_reasoning_when_config_empty() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(Some("gpt-5.4")).await;
    chat.config.model_reasoning_effort = None;

    chat.dispatch_command(SlashCommand::Status);

    let (rendered, _) = render_requested_status(&mut chat, &mut rx);
    assert!(
        rendered.contains("gpt-5.4 (reasoning medium, summaries auto)"),
        "expected /status to render the catalog default reasoning effort, got: {rendered}"
    );
}

#[tokio::test]
async fn status_command_renders_instruction_sources_from_thread_session() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    chat.instruction_source_paths = vec![chat.config.cwd.join("AGENTS.md")];

    chat.dispatch_command(SlashCommand::Status);

    let (rendered, _) = render_requested_status(&mut chat, &mut rx);
    assert!(
        rendered.contains("Agents.md"),
        "expected /status to render app-server instruction sources, got: {rendered}"
    );
    assert!(
        !rendered.contains("Agents.md  <none>"),
        "expected /status to avoid stale <none> when app-server provided instruction sources, got: {rendered}"
    );
}

#[tokio::test]
async fn status_command_overlapping_refreshes_update_matching_cells_only() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    set_chatgpt_auth(&mut chat);

    chat.dispatch_command(SlashCommand::Status);
    let (_, first_request_id) = render_requested_status(&mut chat, &mut rx);
    let first_request_id = first_request_id.expect("ChatGPT status should refresh limits");

    chat.dispatch_command(SlashCommand::Status);
    let (second_rendered, second_request_id) = render_requested_status(&mut chat, &mut rx);
    let second_request_id = second_request_id.expect("ChatGPT status should refresh limits");

    assert_ne!(first_request_id, second_request_id);
    assert!(
        !second_rendered.contains("refreshing limits"),
        "expected /status to avoid transient refresh text in terminal history, got: {second_rendered}"
    );

    chat.finish_status_rate_limit_refresh(first_request_id);
    pretty_assertions::assert_eq!(chat.refreshing_status_outputs.len(), 1);

    chat.on_rate_limit_snapshot(Some(snapshot(/*percent*/ 92.0)));
    chat.finish_status_rate_limit_refresh(second_request_id);
    assert!(chat.refreshing_status_outputs.is_empty());
}

#[tokio::test]
async fn status_output_uses_authoritative_runtime_workspace() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;
    let cwd = test_path_buf("/workspace/runtime").abs();
    let extra_root = test_path_buf("/workspace/shared").abs();
    let cwd_display = cwd.display().to_string();
    let extra_root_display = extra_root.display().to_string();
    chat.instruction_source_paths = vec![cwd.join("AGENTS.md")];

    chat.add_status_output_with_workspace(
        /*refreshing_rate_limits*/ false,
        /*request_id*/ None,
        Some(&codex_app_server_protocol::ThreadWorkspaceReadResponse {
            cwd: cwd.clone(),
            runtime_workspace_roots: vec![cwd, extra_root],
        }),
        /*workspace_state_stale*/ false,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 120))
        }
        other => panic!("expected status output, got {other:?}"),
    };
    assert!(rendered.contains("Directory:"));
    assert!(rendered.contains("Workspace roots:"));
    assert!(rendered.contains(&cwd_display));
    assert!(rendered.contains(&extra_root_display));
    assert!(rendered.contains("Agents.md:"));
    assert!(rendered.contains("AGENTS.md"));
}

#[tokio::test]
async fn status_output_warns_when_workspace_state_may_be_stale() {
    let (mut chat, mut rx, _op_rx) = make_chatwidget_manual(/*model_override*/ None).await;

    chat.add_status_output_with_workspace(
        /*refreshing_rate_limits*/ false, /*request_id*/ None, /*workspace*/ None,
        /*workspace_state_stale*/ true,
    );

    let rendered = match rx.try_recv() {
        Ok(AppEvent::InsertHistoryCell(cell)) => {
            lines_to_single_string(&cell.display_lines(/*width*/ 120))
        }
        other => panic!("expected status output, got {other:?}"),
    };
    assert!(rendered.contains("Warning:"));
    assert!(rendered.contains("workspace state may be stale"));
}
