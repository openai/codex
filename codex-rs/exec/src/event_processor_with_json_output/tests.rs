use super::*;
use crate::exec_events::ConversationEvent;
use crate::exec_events::ConversationItem;
use crate::exec_events::ConversationItemDetails;
use crate::exec_events::ItemCompletedEvent;
use pretty_assertions::assert_eq;
use std::time::Duration;

fn event(id: &str, msg: EventMsg) -> Event {
    Event {
        id: id.to_string(),
        msg,
    }
}

#[test]
fn session_configured_produces_session_created_event() {
    let mut ep = EventProcessorWithJsonOutput::new(None);
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
    let mut ep = EventProcessorWithJsonOutput::new(None);
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
                id: "itm_0".to_string(),
                details: ConversationItemDetails::Reasoning(ReasoningItem {
                    text: "thinking...".to_string(),
                }),
            },
        })]
    );
}

#[test]
fn agent_message_produces_item_completed_assistant_message() {
    let mut ep = EventProcessorWithJsonOutput::new(None);
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
                id: "itm_0".to_string(),
                details: ConversationItemDetails::AssistantMessage(AssistantMessageItem {
                    text: "hello".to_string(),
                }),
            },
        })]
    );
}

#[test]
fn error_event_produces_error() {
    let mut ep = EventProcessorWithJsonOutput::new(None);
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
    let mut ep = EventProcessorWithJsonOutput::new(None);
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
    let mut ep = EventProcessorWithJsonOutput::new(None);

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

    // End (success) -> item.completed (itm_0)
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
                id: "itm_0".to_string(),
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
    let mut ep = EventProcessorWithJsonOutput::new(None);

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

    // End (failure) -> item.completed (itm_0)
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
                id: "itm_0".to_string(),
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
