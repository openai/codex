use codex_app_server_protocol::ServerNotification;
use codex_app_server_protocol::ThreadItem;
use codex_app_server_protocol::Turn;
use codex_app_server_protocol::TurnStatus;
use owo_colors::Style;
use pretty_assertions::assert_eq;

use super::EventProcessorWithHumanOutput;
use super::final_message_from_turn_items;
use super::reasoning_text;
use super::should_print_final_message_to_stdout;
use super::should_print_final_message_to_tty;
use crate::event_processor::EventProcessor;

#[test]
fn suppresses_final_stdout_message_when_both_streams_are_terminals() {
    assert!(!should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn prints_final_stdout_message_when_stdout_is_not_terminal() {
    assert!(should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ false,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn prints_final_stdout_message_when_stderr_is_not_terminal() {
    assert!(should_print_final_message_to_stdout(
        Some("hello"),
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ false
    ));
}

#[test]
fn suppresses_final_stdout_message_when_missing() {
    assert!(!should_print_final_message_to_stdout(
        /*final_message*/ None, /*stdout_is_terminal*/ false,
        /*stderr_is_terminal*/ false
    ));
}

#[test]
fn prints_final_tty_message_when_not_yet_rendered() {
    assert!(should_print_final_message_to_tty(
        Some("hello"),
        /*final_message_rendered*/ false,
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn suppresses_final_tty_message_when_already_rendered() {
    assert!(!should_print_final_message_to_tty(
        Some("hello"),
        /*final_message_rendered*/ true,
        /*stdout_is_terminal*/ true,
        /*stderr_is_terminal*/ true
    ));
}

#[test]
fn reasoning_text_prefers_summary_when_raw_reasoning_is_hidden() {
    let text = reasoning_text(
        &["summary".to_string()],
        &["raw".to_string()],
        /*show_raw_agent_reasoning*/ false,
    );

    assert_eq!(text.as_deref(), Some("summary"));
}

#[test]
fn reasoning_text_uses_raw_content_when_enabled() {
    let text = reasoning_text(
        &["summary".to_string()],
        &["raw".to_string()],
        /*show_raw_agent_reasoning*/ true,
    );

    assert_eq!(text.as_deref(), Some("raw"));
}

#[test]
fn final_message_from_turn_items_uses_latest_agent_message() {
    let message = final_message_from_turn_items(&[
        ThreadItem::AgentMessage {
            id: "msg-1".to_string(),
            text: "first".to_string(),
            phase: None,
            memory_citation: None,
        },
        ThreadItem::Plan {
            id: "plan-1".to_string(),
            text: "plan".to_string(),
        },
        ThreadItem::AgentMessage {
            id: "msg-2".to_string(),
            text: "second".to_string(),
            phase: None,
            memory_citation: None,
        },
    ]);

    assert_eq!(message.as_deref(), Some("second"));
}

#[test]
fn final_message_from_turn_items_falls_back_to_latest_plan() {
    let message = final_message_from_turn_items(&[
        ThreadItem::Reasoning {
            id: "reasoning-1".to_string(),
            summary: vec!["inspect".to_string()],
            content: Vec::new(),
        },
        ThreadItem::Plan {
            id: "plan-1".to_string(),
            text: "first plan".to_string(),
        },
        ThreadItem::Plan {
            id: "plan-2".to_string(),
            text: "final plan".to_string(),
        },
    ]);

    assert_eq!(message.as_deref(), Some("final plan"));
}

#[test]
fn turn_completed_recovers_final_message_from_turn_items() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: None,
        final_message_rendered: false,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: vec![ThreadItem::AgentMessage {
                    id: "msg-1".to_string(),
                    text: "final answer".to_string(),
                    phase: None,
                    memory_citation: None,
                }],
                status: TurnStatus::Completed,
                error: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("final answer"));
}

#[test]
fn turn_completed_overwrites_stale_final_message_from_turn_items() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("stale answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: vec![ThreadItem::AgentMessage {
                    id: "msg-1".to_string(),
                    text: "final answer".to_string(),
                    phase: None,
                    memory_citation: None,
                }],
                status: TurnStatus::Completed,
                error: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("final answer"));
    assert!(!processor.final_message_rendered);
}

#[test]
fn turn_completed_preserves_streamed_final_message_when_turn_items_are_empty() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("streamed answer".to_string()),
        final_message_rendered: false,
        emit_final_message_on_shutdown: false,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: TurnStatus::Completed,
                error: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message.as_deref(), Some("streamed answer"));
    assert!(processor.emit_final_message_on_shutdown);
}

#[test]
fn turn_failed_clears_stale_final_message() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("partial answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: true,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: TurnStatus::Failed,
                error: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message, None);
    assert!(!processor.final_message_rendered);
    assert!(!processor.emit_final_message_on_shutdown);
}

#[test]
fn turn_interrupted_clears_stale_final_message() {
    let mut processor = EventProcessorWithHumanOutput {
        bold: Style::new(),
        cyan: Style::new(),
        dimmed: Style::new(),
        green: Style::new(),
        italic: Style::new(),
        magenta: Style::new(),
        red: Style::new(),
        yellow: Style::new(),
        show_agent_reasoning: true,
        show_raw_agent_reasoning: false,
        last_message_path: None,
        final_message: Some("partial answer".to_string()),
        final_message_rendered: true,
        emit_final_message_on_shutdown: true,
        last_total_token_usage: None,
    };

    let status = processor.process_server_notification(ServerNotification::TurnCompleted(
        codex_app_server_protocol::TurnCompletedNotification {
            thread_id: "thread-1".to_string(),
            turn: Turn {
                id: "turn-1".to_string(),
                items: Vec::new(),
                status: TurnStatus::Interrupted,
                error: None,
            },
        },
    ));

    assert_eq!(
        status,
        crate::event_processor::CodexStatus::InitiateShutdown
    );
    assert_eq!(processor.final_message, None);
    assert!(!processor.final_message_rendered);
    assert!(!processor.emit_final_message_on_shutdown);
}
