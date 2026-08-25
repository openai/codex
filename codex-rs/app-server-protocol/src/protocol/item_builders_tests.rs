use super::*;
use crate::protocol::thread_history::build_turns_from_rollout_items;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandSource;
use codex_protocol::protocol::GuardianAssessmentStatus;
use codex_protocol::protocol::TurnStartedEvent;
use codex_rollout::RolloutItem;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test]
fn read_command_actions_preserve_native_and_foreign_paths() {
    let api_key = "sk-abcdefghijklmnopqrstuvwxyz123456";
    for (cwd_uri, relative_path, expected_path) in [
        (
            "file:///home/alice/repo",
            "src/main.rs",
            "/home/alice/repo/src/main.rs",
        ),
        (
            "file:///C:/Users/Alice%20Smith/repo",
            r"src\main.rs",
            r"C:\Users\Alice Smith\repo\src\main.rs",
        ),
        (
            "file:///C:/Users/Alice%20Smith/repo",
            r"C:src\main.rs",
            r"C:\Users\Alice Smith\repo\src\main.rs",
        ),
        (
            "file://server/share/repo",
            r"src\main.rs",
            r"\\server\share\repo\src\main.rs",
        ),
    ] {
        let cwd = PathUri::parse(cwd_uri).expect("valid cross-platform cwd");
        let command = format!("cat {relative_path}");
        let parsed_cmd = vec![
            ParsedCommand::Read {
                cmd: command.clone(),
                name: "main.rs".to_string(),
                path: PathBuf::from(relative_path),
            },
            ParsedCommand::ListFiles {
                cmd: "ls".to_string(),
                path: Some("subdir".to_string()),
            },
            ParsedCommand::Search {
                cmd: format!("rg {api_key}"),
                query: Some(api_key.to_string()),
                path: Some("src".to_string()),
            },
            ParsedCommand::Search {
                cmd: "rg needle".to_string(),
                query: Some("needle".to_string()),
                path: Some("src".to_string()),
            },
        ];

        assert_eq!(
            serde_json::to_value(command_actions_for_path_uri(&parsed_cmd, &cwd))
                .expect("command actions should serialize"),
            json!([
                {
                    "type": "read",
                    "command": command,
                    "name": "main.rs",
                    "path": expected_path,
                },
                {
                    "type": "listFiles",
                    "command": "ls",
                    "path": "subdir",
                },
                {
                    "type": "search",
                    "command": "rg [REDACTED_SECRET]",
                    "query": "[REDACTED_SECRET]",
                    "path": "src",
                },
                {
                    "type": "search",
                    "command": "rg needle",
                    "query": "needle",
                    "path": "src",
                },
            ]),
            "resolving command actions against {cwd_uri}",
        );
    }
}

#[test]
fn guardian_stdin_reviews_preserve_parent_command_history() {
    let cwd = PathUri::parse("file:///home/alice/repo").expect("valid cwd URI");
    let begin = ExecCommandBeginEvent {
        call_id: "terminal-command".into(),
        plugin_id: None,
        script_path: None,
        process_id: Some("42".into()),
        turn_id: "turn-1".into(),
        started_at_ms: 1_000,
        command: vec!["cat".into()],
        cwd: cwd.clone(),
        parsed_cmd: vec![ParsedCommand::Unknown { cmd: "cat".into() }],
        source: ExecCommandSource::UnifiedExecStartup,
        interaction_input: None,
    };
    let start_turn = |turn_id: &str| {
        RolloutItem::EventMsg(EventMsg::TurnStarted(TurnStartedEvent {
            turn_id: turn_id.into(),
            trace_id: None,
            started_at: None,
            model_context_window: None,
            collaboration_mode_kind: Default::default(),
        }))
    };
    let mut items = vec![
        start_turn("turn-1"),
        RolloutItem::EventMsg(EventMsg::ExecCommandBegin(begin.clone())),
        start_turn("turn-2"),
    ];
    let expected = build_turns_from_rollout_items(&items);
    assert_eq!(expected.len(), 2);
    assert_eq!(
        expected[0].items,
        vec![build_command_execution_begin_item(&begin)],
    );
    assert!(expected[1].items.is_empty());

    let assessment = GuardianAssessmentEvent {
        id: "review-stdin".into(),
        target_item_id: Some("terminal-command".into()),
        plugin_id: None,
        script_path: None,
        turn_id: "turn-2".into(),
        started_at_ms: 2_000,
        completed_at_ms: None,
        status: GuardianAssessmentStatus::InProgress,
        risk_level: None,
        user_authorization: None,
        rationale: None,
        decision_source: None,
        action: GuardianAssessmentAction::WriteStdin {
            approval_id: "stdin-approval".into(),
            process_id: "42".into(),
            stdin: "yes\n".into(),
            cwd,
        },
    };

    for turn_id in ["turn-1", "turn-2"] {
        for status in [
            GuardianAssessmentStatus::InProgress,
            GuardianAssessmentStatus::Approved,
            GuardianAssessmentStatus::Denied,
            GuardianAssessmentStatus::TimedOut,
            GuardianAssessmentStatus::Aborted,
        ] {
            items.push(RolloutItem::EventMsg(EventMsg::GuardianAssessment(
                GuardianAssessmentEvent {
                    turn_id: turn_id.into(),
                    status,
                    ..assessment.clone()
                },
            )));
            assert_eq!(
                build_turns_from_rollout_items(&items),
                expected,
                "stdin review {status:?} in {turn_id} must preserve command history",
            );
            items.pop();
        }
    }
}
