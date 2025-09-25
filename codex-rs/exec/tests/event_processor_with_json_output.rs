use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::PatchApplyBeginEvent;
use codex_core::protocol::PatchApplyEndEvent;
use codex_core::protocol::SessionConfiguredEvent;
use codex_exec::exec_events::AssistantMessageItem;
use codex_exec::exec_events::CommandExecutionItem;
use codex_exec::exec_events::CommandExecutionStatus;
use codex_exec::exec_events::ConversationErrorEvent;
use codex_exec::exec_events::ConversationEvent;
use codex_exec::exec_events::ConversationItem;
use codex_exec::exec_events::ConversationItemDetails;
use codex_exec::exec_events::ItemCompletedEvent;
use codex_exec::exec_events::PatchApplyStatus;
use codex_exec::exec_events::PatchChangeKind;
use codex_exec::exec_events::ReasoningItem;
use codex_exec::exec_events::SessionCreatedEvent;
use codex_exec::experimental_event_processor_with_json_output::ExperimentalEventProcessorWithJsonOutput;
use pretty_assertions::assert_eq;
use std::path::PathBuf;
use std::time::Duration;

fn event(id: &str, msg: EventMsg) -> Event {
    Event {
        id: id.to_string(),
        msg,
    }
}

#[test]
fn session_configured_produces_session_created_event() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let session_id = codex_protocol::mcp_protocol::ConversationId::from_string(
        "67e55044-10b1-426f-9247-bb680e5fe0c8",
    )
    .unwrap();
    let rollout_path = PathBuf::from("/tmp/rollout.json");
    let ev = event(
        "e1",
        EventMsg::SessionConfigured(SessionConfiguredEvent {
            session_id,
            model: "codex-mini-latest".to_string(),
            reasoning_effort: None,
            history_log_id: 0,
            history_entry_count: 0,
            initial_messages: None,
            rollout_path,
        }),
    );
    let out = ep.collect_conversation_events(&ev);
    assert_eq!(
        out,
        vec![ConversationEvent::SessionCreated(SessionCreatedEvent {
            session_id: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
        })]
    );
}

#[test]
fn agent_reasoning_produces_item_completed_reasoning() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let ev = event(
        "e1",
        EventMsg::AgentReasoning(AgentReasoningEvent {
            text: "thinking...".to_string(),
        }),
    );
    let out = ep.collect_conversation_events(&ev);
    assert_eq!(
        out,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::Reasoning(ReasoningItem {
                    text: "thinking...".to_string(),
                }),
            },
        })]
    );
}

#[test]
fn agent_message_produces_item_completed_assistant_message() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let ev = event(
        "e1",
        EventMsg::AgentMessage(AgentMessageEvent {
            message: "hello".to_string(),
        }),
    );
    let out = ep.collect_conversation_events(&ev);
    assert_eq!(
        out,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::AssistantMessage(AssistantMessageItem {
                    text: "hello".to_string(),
                }),
            },
        })]
    );
}

#[test]
fn error_event_produces_error() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let out = ep.collect_conversation_events(&event(
        "e1",
        EventMsg::Error(codex_core::protocol::ErrorEvent {
            message: "boom".to_string(),
        }),
    ));
    assert_eq!(
        out,
        vec![ConversationEvent::Error(ConversationErrorEvent {
            message: "boom".to_string(),
        })]
    );
}

#[test]
fn stream_error_event_produces_error() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let out = ep.collect_conversation_events(&event(
        "e1",
        EventMsg::StreamError(codex_core::protocol::StreamErrorEvent {
            message: "retrying".to_string(),
        }),
    ));
    assert_eq!(
        out,
        vec![ConversationEvent::Error(ConversationErrorEvent {
            message: "retrying".to_string(),
        })]
    );
}

#[test]
fn exec_command_end_success_produces_completed_command_item() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // Begin -> no output
    let begin = event(
        "c1",
        EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "1".to_string(),
            command: vec!["bash".to_string(), "-lc".to_string(), "echo hi".to_string()],
            cwd: std::env::current_dir().unwrap(),
            parsed_cmd: Vec::new(),
        }),
    );
    let out_begin = ep.collect_conversation_events(&begin);
    assert!(out_begin.is_empty());

    // End (success) -> item.completed (item_0)
    let end_ok = event(
        "c2",
        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "1".to_string(),
            stdout: String::new(),
            stderr: String::new(),
            aggregated_output: "hi\n".to_string(),
            exit_code: 0,
            duration: Duration::from_millis(5),
            formatted_output: String::new(),
        }),
    );
    let out_ok = ep.collect_conversation_events(&end_ok);
    assert_eq!(
        out_ok,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                    command: "bash -lc echo hi".to_string(),
                    aggregated_output: "hi\n".to_string(),
                    exit_code: 0,
                    status: CommandExecutionStatus::Completed,
                }),
            },
        })]
    );
}

#[test]
fn exec_command_end_failure_produces_failed_command_item() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // Begin -> no output
    let begin = event(
        "c1",
        EventMsg::ExecCommandBegin(ExecCommandBeginEvent {
            call_id: "2".to_string(),
            command: vec!["sh".to_string(), "-c".to_string(), "exit 1".to_string()],
            cwd: std::env::current_dir().unwrap(),
            parsed_cmd: Vec::new(),
        }),
    );
    assert!(ep.collect_conversation_events(&begin).is_empty());

    // End (failure) -> item.completed (item_0)
    let end_fail = event(
        "c2",
        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "2".to_string(),
            stdout: String::new(),
            stderr: String::new(),
            aggregated_output: String::new(),
            exit_code: 1,
            duration: Duration::from_millis(2),
            formatted_output: String::new(),
        }),
    );
    let out_fail = ep.collect_conversation_events(&end_fail);
    assert_eq!(
        out_fail,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                    command: "sh -c exit 1".to_string(),
                    aggregated_output: String::new(),
                    exit_code: 1,
                    status: CommandExecutionStatus::Failed,
                }),
            },
        })]
    );
}

#[test]
fn patch_apply_success_produces_item_completed_patchapply() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // Prepare a patch with multiple kinds of changes
    let mut changes = std::collections::HashMap::new();
    changes.insert(
        PathBuf::from("a/added.txt"),
        FileChange::Add {
            content: "+hello".to_string(),
        },
    );
    changes.insert(
        PathBuf::from("b/deleted.txt"),
        FileChange::Delete {
            content: "-goodbye".to_string(),
        },
    );
    changes.insert(
        PathBuf::from("c/modified.txt"),
        FileChange::Update {
            unified_diff: "--- c/modified.txt\n+++ c/modified.txt\n@@\n-old\n+new\n".to_string(),
            move_path: Some(PathBuf::from("c/renamed.txt")),
        },
    );

    // Begin -> no output
    let begin = event(
        "p1",
        EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "call-1".to_string(),
            auto_approved: true,
            changes: changes.clone(),
        }),
    );
    let out_begin = ep.collect_conversation_events(&begin);
    assert!(out_begin.is_empty());

    // End (success) -> item.completed (item_0)
    let end = event(
        "p2",
        EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: "call-1".to_string(),
            stdout: "applied 3 changes".to_string(),
            stderr: String::new(),
            success: true,
        }),
    );
    let out_end = ep.collect_conversation_events(&end);
    assert_eq!(out_end.len(), 1);

    // Validate structure without relying on HashMap iteration order
    match &out_end[0] {
        ConversationEvent::ItemCompleted(ItemCompletedEvent { item }) => {
            assert_eq!(&item.id, "item_0");
            match &item.details {
                ConversationItemDetails::FileChange(file_update) => {
                    assert_eq!(file_update.status, PatchApplyStatus::Completed);

                    let mut actual: Vec<(String, PatchChangeKind)> = file_update
                        .changes
                        .iter()
                        .map(|c| (c.path.clone(), c.kind.clone()))
                        .collect();
                    actual.sort_by(|a, b| a.0.cmp(&b.0));

                    let mut expected = vec![
                        ("a/added.txt".to_string(), PatchChangeKind::Add),
                        ("b/deleted.txt".to_string(), PatchChangeKind::Delete),
                        ("c/modified.txt".to_string(), PatchChangeKind::Update),
                    ];
                    expected.sort_by(|a, b| a.0.cmp(&b.0));

                    assert_eq!(actual, expected);
                }
                other => panic!("unexpected details: {other:?}"),
            }
        }
        other => panic!("unexpected event: {other:?}"),
    }
}

#[test]
fn patch_apply_failure_produces_item_completed_patchapply_failed() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    let mut changes = std::collections::HashMap::new();
    changes.insert(
        PathBuf::from("file.txt"),
        FileChange::Update {
            unified_diff: "--- file.txt\n+++ file.txt\n@@\n-old\n+new\n".to_string(),
            move_path: None,
        },
    );

    // Begin -> no output
    let begin = event(
        "p1",
        EventMsg::PatchApplyBegin(PatchApplyBeginEvent {
            call_id: "call-2".to_string(),
            auto_approved: false,
            changes: changes.clone(),
        }),
    );
    assert!(ep.collect_conversation_events(&begin).is_empty());

    // End (failure) -> item.completed (item_0) with Failed status
    let end = event(
        "p2",
        EventMsg::PatchApplyEnd(PatchApplyEndEvent {
            call_id: "call-2".to_string(),
            stdout: String::new(),
            stderr: "failed to apply".to_string(),
            success: false,
        }),
    );
    let out_end = ep.collect_conversation_events(&end);
    assert_eq!(out_end.len(), 1);

    match &out_end[0] {
        ConversationEvent::ItemCompleted(ItemCompletedEvent { item }) => {
            assert_eq!(&item.id, "item_0");
            match &item.details {
                ConversationItemDetails::FileChange(file_update) => {
                    assert_eq!(file_update.status, PatchApplyStatus::Failed);
                    assert_eq!(file_update.changes.len(), 1);
                    assert_eq!(file_update.changes[0].path, "file.txt".to_string());
                    assert_eq!(file_update.changes[0].kind, PatchChangeKind::Update);
                }
                other => panic!("unexpected details: {other:?}"),
            }
        }
        other => panic!("unexpected event: {other:?}"),
    }
}
