use super::*;
use crate::test_support::PathBufExt;
use crate::test_support::test_path_buf;
use crate::thread_transcript::RawReasoningVisibility;
use crate::thread_transcript::thread_items_to_transcript_cells;
use codex_app_server_protocol::CommandAction;
use codex_app_server_protocol::McpToolCallResult;
use codex_utils_path_uri::LegacyAppPathString;
use pretty_assertions::assert_eq;
use serde_json::json;

fn command_item(status: CommandExecutionStatus) -> ThreadItem {
    ThreadItem::CommandExecution {
        id: "command".to_string(),
        plugin_id: None,
        script_path: None,
        model_context: None,
        sandbox_type: None,
        command: "cargo check".to_string(),
        cwd: LegacyAppPathString::from_string("/tmp/project"),
        process_id: None,
        source: CommandExecutionSource::Agent,
        status,
        command_actions: vec![CommandAction::Unknown {
            command: "cargo check".to_string(),
        }],
        aggregated_output: Some(
            (1..=12)
                .map(|line| format!("output line {line}"))
                .collect::<Vec<_>>()
                .join("\n"),
        ),
        exit_code: Some(0),
        duration_ms: Some(-5),
    }
}

fn mcp_item(server: &str, id: &str) -> ThreadItem {
    ThreadItem::McpToolCall {
        id: id.to_string(),
        server: server.to_string(),
        tool: "js".to_string(),
        status: McpToolCallStatus::Completed,
        arguments: json!({"title": format!("Inspect page {id}"), "code": "await cua.getState()"}),
        app_context: None,
        mcp_app_resource_uri: None,
        plugin_id: None,
        read_only_hint: None,
        mcp_app_ui: None,
        result: Some(Box::new(McpToolCallResult {
            content: vec![json!({"type": "text", "text": format!("Full result for {id}")})],
            structured_content: None,
            meta: None,
        })),
        error: None,
        duration_ms: Some(5),
    }
}

#[test]
fn completed_tools_keep_compact_and_detailed_presentations() {
    let cwd = test_path_buf("/workspace").abs();
    let items = [
        command_item(CommandExecutionStatus::Completed),
        mcp_item("example", "mcp"),
    ];
    let cells = thread_items_to_transcript_cells(
        /*thread_id*/ None,
        &cwd,
        items,
        RawReasoningVisibility::Hidden,
        /*config*/ None,
    );
    assert_eq!(cells.len(), 2);
    let rendered = ["Command", "MCP"]
        .into_iter()
        .zip(cells)
        .map(|(label, cell)| {
            assert_eq!(cell.transcript_animation_tick(), None);
            let display = cell
                .display_lines(/*width*/ 80)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            let transcript = cell
                .transcript_lines(/*width*/ 80)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            format!("{label}: compact\n{display}\n\n{label}: detailed\n{transcript}")
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    insta::assert_snapshot!("completed_tool_presentations", rendered);
}

#[test]
fn failed_commands_keep_failure_when_exit_code_is_missing_or_zero() {
    for exit in [None, Some(0)] {
        let mut item = command_item(CommandExecutionStatus::Failed);
        if let ThreadItem::CommandExecution { exit_code, .. } = &mut item {
            *exit_code = exit;
        }
        let command = CommandHistory::from_item(item).unwrap();
        assert_eq!((command.exit_code, command.duration), (1, Duration::ZERO));
    }
}

#[test]
fn incomplete_tool_payloads_preserve_last_known_status_and_output() {
    let command = command_item(CommandExecutionStatus::InProgress);
    let mut mcp = mcp_item("example", "pending");
    if let ThreadItem::McpToolCall { status, result, .. } = &mut mcp {
        *status = McpToolCallStatus::InProgress;
        *result = None;
    }
    let mut missing_result = mcp_item("example", "completed");
    if let ThreadItem::McpToolCall { result, .. } = &mut missing_result {
        *result = None;
    }
    let cwd = test_path_buf("/workspace").abs();
    let rendered = thread_items_to_transcript_cells(
        /*thread_id*/ None,
        &cwd,
        [command, mcp, missing_result],
        RawReasoningVisibility::Hidden,
        /*config*/ None,
    )
    .into_iter()
    .flat_map(|cell| cell.display_lines(/*width*/ 80))
    .map(|line| line.to_string())
    .collect::<Vec<_>>()
    .join("\n");
    insta::assert_snapshot!("pending_tool_presentations", rendered);
}

#[test]
fn historical_command_fallbacks_preserve_status_output_and_group_boundaries() {
    let cwd = test_path_buf("/workspace").abs();
    let exploration = |call_id: &str| {
        let mut item = command_item(CommandExecutionStatus::Completed);
        if let ThreadItem::CommandExecution {
            id,
            command,
            command_actions,
            ..
        } = &mut item
        {
            *id = call_id.to_owned();
            *command = "cat README.md".to_owned();
            *command_actions = vec![CommandAction::Read {
                command: command.clone(),
                name: "README.md".to_owned(),
                path: LegacyAppPathString::from_string("/workspace/README.md"),
            }];
        }
        item
    };
    let mut snapshots = Vec::new();
    for (source, status, recorded_exit, label) in [
        (
            CommandExecutionSource::Agent,
            CommandExecutionStatus::Declined,
            None,
            "declined",
        ),
        (
            CommandExecutionSource::Agent,
            CommandExecutionStatus::Completed,
            Some(0),
            "opaque command",
        ),
        (
            CommandExecutionSource::Agent,
            CommandExecutionStatus::Completed,
            Some(0),
            "UNC command",
        ),
        (
            CommandExecutionSource::UnifiedExecInteraction,
            CommandExecutionStatus::Completed,
            Some(0),
            "completed interaction",
        ),
        (
            CommandExecutionSource::UnifiedExecInteraction,
            CommandExecutionStatus::Failed,
            Some(7),
            "failed interaction",
        ),
        (
            CommandExecutionSource::UnifiedExecInteraction,
            CommandExecutionStatus::Failed,
            None,
            "failed interaction without exit code",
        ),
        (
            CommandExecutionSource::UnifiedExecInteraction,
            CommandExecutionStatus::InProgress,
            None,
            "pending interaction",
        ),
    ] {
        let mut item = command_item(status);
        if let ThreadItem::CommandExecution {
            source: item_source,
            command,
            aggregated_output,
            exit_code,
            ..
        } = &mut item
        {
            *item_source = source;
            if label == "opaque command" {
                *command = r#"C:\Program Files\Git\bin\bash.exe -lc "echo hi""#.to_owned();
            } else if label == "UNC command" {
                *command = r"\\server\share\tool.exe -arg".to_owned();
            }
            *aggregated_output = Some("first line\n  indented output\nlast line".to_owned());
            *exit_code = recorded_exit;
        }
        let cells = thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &cwd,
            [exploration("before"), item, exploration("after")],
            RawReasoningVisibility::Hidden,
            /*config*/ None,
        );
        assert_eq!(cells.len(), 3);
        for cell in [&cells[0], &cells[2]] {
            assert!(
                cell.as_any()
                    .downcast_ref::<ExecCell>()
                    .unwrap()
                    .is_exploring_cell()
            );
        }
        let [compact, detailed, raw] = [
            cells[1].display_lines(/*width*/ 80),
            cells[1].transcript_lines(/*width*/ 80),
            cells[1].raw_lines(),
        ]
        .map(|lines| {
            lines
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n")
        });
        assert_eq!((&detailed, &raw), (&compact, &compact));
        snapshots.push(format!("{label}\n{compact}"));
    }
    insta::assert_snapshot!(snapshots.join("\n\n"));
}

#[test]
fn agent_tool_fallbacks_preserve_status_without_duplicating_v2_activity() {
    let cwd = test_path_buf("/workspace").abs();
    let mut snapshots = Vec::new();
    for (tool, status) in [
        (
            CollabAgentTool::SpawnAgent,
            CollabAgentToolCallStatus::InProgress,
        ),
        (
            CollabAgentTool::SendInput,
            CollabAgentToolCallStatus::Failed,
        ),
        (
            CollabAgentTool::CloseAgent,
            CollabAgentToolCallStatus::Interrupted,
        ),
        (
            CollabAgentTool::ResumeAgent,
            CollabAgentToolCallStatus::Failed,
        ),
        (
            CollabAgentTool::Wait,
            CollabAgentToolCallStatus::Interrupted,
        ),
        (
            CollabAgentTool::SendMessage,
            CollabAgentToolCallStatus::InProgress,
        ),
        (
            CollabAgentTool::FollowupTask,
            CollabAgentToolCallStatus::InProgress,
        ),
        (
            CollabAgentTool::InterruptAgent,
            CollabAgentToolCallStatus::InProgress,
        ),
        (
            CollabAgentTool::ListAgents,
            CollabAgentToolCallStatus::InProgress,
        ),
    ] {
        let visible = matches!(
            tool,
            CollabAgentTool::SpawnAgent
                | CollabAgentTool::SendInput
                | CollabAgentTool::CloseAgent
                | CollabAgentTool::ResumeAgent
                | CollabAgentTool::Wait
        );
        let item = ThreadItem::CollabAgentToolCall {
            id: "pending-agent-call".to_string(),
            tool,
            status,
            sender_thread_id: "00000000-0000-0000-0000-000000000001".to_string(),
            receiver_thread_ids: vec!["00000000-0000-0000-0000-000000000002".to_string()],
            prompt: Some("Inspect the parser".to_string()),
            model: None,
            reasoning_effort: None,
            agents_states: Default::default(),
        };
        let cells = thread_items_to_transcript_cells(
            /*thread_id*/ None,
            &cwd,
            [item],
            RawReasoningVisibility::Hidden,
            /*config*/ None,
        );
        assert_eq!(cells.len(), usize::from(visible));
        for cell in cells {
            assert_eq!(cell.transcript_animation_tick(), None);
            let text = cell
                .transcript_lines(/*width*/ 80)
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("\n");
            snapshots.push(text);
        }
    }
    insta::assert_snapshot!(snapshots.join("\n"));
}
