use codex_core::protocol::AgentMessageEvent;
use codex_core::protocol::AgentReasoningEvent;
use codex_core::protocol::ErrorEvent;
use codex_core::protocol::Event;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandBeginEvent;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::FileChange;
use codex_core::protocol::McpInvocation;
use codex_core::protocol::McpToolCallBeginEvent;
use codex_core::protocol::McpToolCallEndEvent;
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
use codex_exec::exec_events::ItemStartedEvent;
use codex_exec::exec_events::ItemUpdatedEvent;
use codex_exec::exec_events::McpToolCallItem;
use codex_exec::exec_events::McpToolCallStatus;
use codex_exec::exec_events::PatchApplyStatus;
use codex_exec::exec_events::PatchChangeKind;
use codex_exec::exec_events::ReasoningItem;
use codex_exec::exec_events::ThreadStartedEvent;
use codex_exec::exec_events::TodoItem as ExecTodoItem;
use codex_exec::exec_events::TodoListItem as ExecTodoListItem;
use codex_exec::exec_events::TurnCompletedEvent;
use codex_exec::exec_events::TurnFailedEvent;
use codex_exec::exec_events::TurnStartedEvent;
use codex_exec::exec_events::Usage;
use codex_exec::experimental_event_processor_with_json_output::ExperimentalEventProcessorWithJsonOutput;
use mcp_types::CallToolResult;
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
fn session_configured_produces_thread_started_event() {
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
        vec![ConversationEvent::ThreadStarted(ThreadStartedEvent {
            thread_id: "67e55044-10b1-426f-9247-bb680e5fe0c8".to_string(),
        })]
    );
}

#[test]
fn task_started_produces_turn_started_event() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let out = ep.collect_conversation_events(&event(
        "t1",
        EventMsg::TaskStarted(codex_core::protocol::TaskStartedEvent {
            model_context_window: Some(32_000),
        }),
    ));

    assert_eq!(
        out,
        vec![ConversationEvent::TurnStarted(TurnStartedEvent {})]
    );
}

#[test]
fn plan_update_emits_todo_list_started_updated_and_completed() {
    use codex_core::plan_tool::PlanItemArg;
    use codex_core::plan_tool::StepStatus;
    use codex_core::plan_tool::UpdatePlanArgs;

    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // First plan update => item.started (todo_list)
    let first = event(
        "p1",
        EventMsg::PlanUpdate(UpdatePlanArgs {
            explanation: None,
            plan: vec![
                PlanItemArg {
                    step: "step one".to_string(),
                    status: StepStatus::Pending,
                },
                PlanItemArg {
                    step: "step two".to_string(),
                    status: StepStatus::InProgress,
                },
            ],
        }),
    );
    let out_first = ep.collect_conversation_events(&first);
    assert_eq!(
        out_first,
        vec![ConversationEvent::ItemStarted(ItemStartedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::TodoList(ExecTodoListItem {
                    items: vec![
                        ExecTodoItem {
                            text: "step one".to_string(),
                            completed: false
                        },
                        ExecTodoItem {
                            text: "step two".to_string(),
                            completed: false
                        },
                    ],
                }),
            },
        })]
    );

    // Second plan update in same turn => item.updated (same id)
    let second = event(
        "p2",
        EventMsg::PlanUpdate(UpdatePlanArgs {
            explanation: None,
            plan: vec![
                PlanItemArg {
                    step: "step one".to_string(),
                    status: StepStatus::Completed,
                },
                PlanItemArg {
                    step: "step two".to_string(),
                    status: StepStatus::InProgress,
                },
            ],
        }),
    );
    let out_second = ep.collect_conversation_events(&second);
    assert_eq!(
        out_second,
        vec![ConversationEvent::ItemUpdated(ItemUpdatedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::TodoList(ExecTodoListItem {
                    items: vec![
                        ExecTodoItem {
                            text: "step one".to_string(),
                            completed: true
                        },
                        ExecTodoItem {
                            text: "step two".to_string(),
                            completed: false
                        },
                    ],
                }),
            },
        })]
    );

    // Task completes => item.completed (same id, latest state)
    let complete = event(
        "p3",
        EventMsg::TaskComplete(codex_core::protocol::TaskCompleteEvent {
            last_agent_message: None,
        }),
    );
    let out_complete = ep.collect_conversation_events(&complete);
    assert_eq!(
        out_complete,
        vec![
            ConversationEvent::ItemCompleted(ItemCompletedEvent {
                item: ConversationItem {
                    id: "item_0".to_string(),
                    details: ConversationItemDetails::TodoList(ExecTodoListItem {
                        items: vec![
                            ExecTodoItem {
                                text: "step one".to_string(),
                                completed: true
                            },
                            ExecTodoItem {
                                text: "step two".to_string(),
                                completed: false
                            },
                        ],
                    }),
                },
            }),
            ConversationEvent::TurnCompleted(TurnCompletedEvent {
                usage: Usage::default(),
            }),
        ]
    );
}

#[test]
fn mcp_tool_call_begin_and_end_emit_item_events() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let invocation = McpInvocation {
        server: "server_a".to_string(),
        tool: "tool_x".to_string(),
        arguments: None,
    };

    let begin = event(
        "m1",
        EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: "call-1".to_string(),
            invocation: invocation.clone(),
        }),
    );
    let begin_events = ep.collect_conversation_events(&begin);
    assert_eq!(
        begin_events,
        vec![ConversationEvent::ItemStarted(ItemStartedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::McpToolCall(McpToolCallItem {
                    server: "server_a".to_string(),
                    tool: "tool_x".to_string(),
                    status: McpToolCallStatus::InProgress,
                }),
            },
        })]
    );

    let end = event(
        "m2",
        EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: "call-1".to_string(),
            invocation,
            duration: Duration::from_secs(1),
            result: Ok(CallToolResult {
                content: Vec::new(),
                is_error: None,
                structured_content: None,
            }),
        }),
    );
    let end_events = ep.collect_conversation_events(&end);
    assert_eq!(
        end_events,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::McpToolCall(McpToolCallItem {
                    server: "server_a".to_string(),
                    tool: "tool_x".to_string(),
                    status: McpToolCallStatus::Completed,
                }),
            },
        })]
    );
}

#[test]
fn mcp_tool_call_failure_sets_failed_status() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);
    let invocation = McpInvocation {
        server: "server_b".to_string(),
        tool: "tool_y".to_string(),
        arguments: None,
    };

    let begin = event(
        "m3",
        EventMsg::McpToolCallBegin(McpToolCallBeginEvent {
            call_id: "call-2".to_string(),
            invocation: invocation.clone(),
        }),
    );
    ep.collect_conversation_events(&begin);

    let end = event(
        "m4",
        EventMsg::McpToolCallEnd(McpToolCallEndEvent {
            call_id: "call-2".to_string(),
            invocation,
            duration: Duration::from_millis(5),
            result: Err("tool exploded".to_string()),
        }),
    );
    let events = ep.collect_conversation_events(&end);
    assert_eq!(
        events,
        vec![ConversationEvent::ItemCompleted(ItemCompletedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::McpToolCall(McpToolCallItem {
                    server: "server_b".to_string(),
                    tool: "tool_y".to_string(),
                    status: McpToolCallStatus::Failed,
                }),
            },
        })]
    );
}

#[test]
fn plan_update_after_complete_starts_new_todo_list_with_new_id() {
    use codex_core::plan_tool::PlanItemArg;
    use codex_core::plan_tool::StepStatus;
    use codex_core::plan_tool::UpdatePlanArgs;

    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // First turn: start + complete
    let start = event(
        "t1",
        EventMsg::PlanUpdate(UpdatePlanArgs {
            explanation: None,
            plan: vec![PlanItemArg {
                step: "only".to_string(),
                status: StepStatus::Pending,
            }],
        }),
    );
    let _ = ep.collect_conversation_events(&start);
    let complete = event(
        "t2",
        EventMsg::TaskComplete(codex_core::protocol::TaskCompleteEvent {
            last_agent_message: None,
        }),
    );
    let _ = ep.collect_conversation_events(&complete);

    // Second turn: a new todo list should have a new id
    let start_again = event(
        "t3",
        EventMsg::PlanUpdate(UpdatePlanArgs {
            explanation: None,
            plan: vec![PlanItemArg {
                step: "again".to_string(),
                status: StepStatus::Pending,
            }],
        }),
    );
    let out = ep.collect_conversation_events(&start_again);

    match &out[0] {
        ConversationEvent::ItemStarted(ItemStartedEvent { item }) => {
            assert_eq!(&item.id, "item_1");
        }
        other => panic!("unexpected event: {other:?}"),
    }
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
fn error_followed_by_task_complete_produces_turn_failed() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    let error_event = event(
        "e1",
        EventMsg::Error(ErrorEvent {
            message: "boom".to_string(),
        }),
    );
    assert_eq!(
        ep.collect_conversation_events(&error_event),
        vec![ConversationEvent::Error(ConversationErrorEvent {
            message: "boom".to_string(),
        })]
    );

    let complete_event = event(
        "e2",
        EventMsg::TaskComplete(codex_core::protocol::TaskCompleteEvent {
            last_agent_message: None,
        }),
    );
    assert_eq!(
        ep.collect_conversation_events(&complete_event),
        vec![ConversationEvent::TurnFailed(TurnFailedEvent {
            error: ConversationErrorEvent {
                message: "boom".to_string(),
            },
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
    assert_eq!(
        out_begin,
        vec![ConversationEvent::ItemStarted(ItemStartedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                    command: "bash -lc 'echo hi'".to_string(),
                    aggregated_output: String::new(),
                    exit_code: None,
                    status: CommandExecutionStatus::InProgress,
                }),
            },
        })]
    );

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
                    command: "bash -lc 'echo hi'".to_string(),
                    aggregated_output: "hi\n".to_string(),
                    exit_code: Some(0),
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
    assert_eq!(
        ep.collect_conversation_events(&begin),
        vec![ConversationEvent::ItemStarted(ItemStartedEvent {
            item: ConversationItem {
                id: "item_0".to_string(),
                details: ConversationItemDetails::CommandExecution(CommandExecutionItem {
                    command: "sh -c 'exit 1'".to_string(),
                    aggregated_output: String::new(),
                    exit_code: None,
                    status: CommandExecutionStatus::InProgress,
                }),
            },
        })]
    );

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
                    command: "sh -c 'exit 1'".to_string(),
                    aggregated_output: String::new(),
                    exit_code: Some(1),
                    status: CommandExecutionStatus::Failed,
                }),
            },
        })]
    );
}

#[test]
fn exec_command_end_without_begin_is_ignored() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // End event arrives without a prior Begin; should produce no conversation events.
    let end_only = event(
        "c1",
        EventMsg::ExecCommandEnd(ExecCommandEndEvent {
            call_id: "no-begin".to_string(),
            stdout: String::new(),
            stderr: String::new(),
            aggregated_output: String::new(),
            exit_code: 0,
            duration: Duration::from_millis(1),
            formatted_output: String::new(),
        }),
    );
    let out = ep.collect_conversation_events(&end_only);
    assert!(out.is_empty());
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

#[test]
fn task_complete_produces_turn_completed_with_usage() {
    let mut ep = ExperimentalEventProcessorWithJsonOutput::new(None);

    // First, feed a TokenCount event with known totals.
    let usage = codex_core::protocol::TokenUsage {
        input_tokens: 1200,
        cached_input_tokens: 200,
        output_tokens: 345,
        reasoning_output_tokens: 0,
        total_tokens: 0,
    };
    let info = codex_core::protocol::TokenUsageInfo {
        total_token_usage: usage.clone(),
        last_token_usage: usage,
        model_context_window: None,
    };
    let token_count_event = event(
        "e1",
        EventMsg::TokenCount(codex_core::protocol::TokenCountEvent {
            info: Some(info),
            rate_limits: None,
        }),
    );
    assert!(
        ep.collect_conversation_events(&token_count_event)
            .is_empty()
    );

    // Then TaskComplete should produce turn.completed with the captured usage.
    let complete_event = event(
        "e2",
        EventMsg::TaskComplete(codex_core::protocol::TaskCompleteEvent {
            last_agent_message: Some("done".to_string()),
        }),
    );
    let out = ep.collect_conversation_events(&complete_event);
    assert_eq!(
        out,
        vec![ConversationEvent::TurnCompleted(TurnCompletedEvent {
            usage: Usage {
                input_tokens: 1200,
                cached_input_tokens: 200,
                output_tokens: 345,
            },
        })]
    );
}
