use super::*;
use codex_app_server_protocol::CommandExecutionSource;
use codex_app_server_protocol::CommandExecutionStatus;
use codex_app_server_protocol::ItemCompletedNotification;
use codex_utils_absolute_path::AbsolutePathBuf;

#[test]
fn agent_status_uses_bounded_buffered_activity() {
    let mut store = ThreadEventStore::new(/*capacity*/ 8);
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification {
            item: ThreadItem::CommandExecution {
                id: "command-1".to_string(),
                command: "cargo test -p codex-tui".to_string(),
                cwd: codex_app_server_protocol::native_command_cwd(
                    &AbsolutePathBuf::try_from("/workspace").expect("absolute path"),
                ),
                process_id: None,
                source: CommandExecutionSource::Agent,
                status: CommandExecutionStatus::Completed,
                command_actions: Vec::new(),
                aggregated_output: Some("unbounded output\n".repeat(10_000)),
                exit_code: Some(0),
                duration_ms: Some(42),
            },
            thread_id: "thread-child".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 1,
        },
    ));
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification {
            item: ThreadItem::AgentMessage {
                id: "message-1".to_string(),
                text: "Finished checking the focused TUI tests.".to_string(),
                phase: None,
                memory_citation: None,
            },
            thread_id: "thread-child".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 2,
        },
    ));

    let preview = AgentStatusThreadPreview::from_store("/root/reviewer".to_string(), &store);
    let cell = AgentStatusHistoryCell::new(vec![preview]);
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r###"
    /agent
    Sub-agents running

      • `/root/reviewer`
        $ cargo test -p codex-tui
        Finished checking the focused TUI tests.
    "###);
    assert!(!rendered.contains("unbounded output"));
}

#[test]
fn agent_status_uses_reasoning_summaries_only() {
    let mut store = ThreadEventStore::new(/*capacity*/ 8);
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification {
            item: ThreadItem::Reasoning {
                id: "reasoning-with-summary".to_string(),
                summary: vec!["safe summary".to_string()],
                content: vec!["hidden raw reasoning".to_string()],
            },
            thread_id: "thread-child".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 1,
        },
    ));
    store.push_notification(ServerNotification::ItemCompleted(
        ItemCompletedNotification {
            item: ThreadItem::Reasoning {
                id: "reasoning-without-summary".to_string(),
                summary: Vec::new(),
                content: vec!["raw-only reasoning".to_string()],
            },
            thread_id: "thread-child".to_string(),
            turn_id: "turn-1".to_string(),
            completed_at_ms: 2,
        },
    ));

    let preview = AgentStatusThreadPreview::from_store("/root/reviewer".to_string(), &store);
    let cell = AgentStatusHistoryCell::new(vec![preview]);
    let rendered = cell
        .display_lines(/*width*/ 80)
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join("\n");

    insta::assert_snapshot!(rendered, @r###"
    /agent
    Sub-agents running

      • `/root/reviewer`
        safe summary
    "###);
    assert!(!rendered.contains("hidden raw reasoning"));
    assert!(!rendered.contains("raw-only reasoning"));
}
