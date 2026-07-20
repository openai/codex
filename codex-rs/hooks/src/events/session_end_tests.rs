use std::collections::HashMap;

use codex_protocol::protocol::HookEventName;
use codex_protocol::protocol::HookRunStatus;
use codex_protocol::protocol::HookSource;
use codex_utils_absolute_path::test_support::PathBufExt;
use codex_utils_absolute_path::test_support::test_path_buf;
use pretty_assertions::assert_eq;

use super::parse_completed;
use super::preview;
use crate::engine::ConfiguredHandler;
use crate::engine::command_runner::CommandRunResult;

#[test]
fn session_end_matches_other_reason() {
    let selected = preview(&[
        ConfiguredHandler {
            display_order: 0,
            ..handler(Some("clear"))
        },
        ConfiguredHandler {
            display_order: 1,
            ..handler(Some("other"))
        },
        ConfiguredHandler {
            display_order: 2,
            ..handler(/*matcher*/ None)
        },
    ]);

    assert_eq!(
        selected
            .iter()
            .map(|run| run.display_order)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn session_end_ignores_successful_output() {
    let completed = parse_completed(
        &handler(/*matcher*/ None),
        CommandRunResult {
            started_at: 1,
            completed_at: 2,
            duration_ms: 1,
            exit_code: Some(0),
            stdout: r#"{"continue":false,"decision":"block","reason":"ignored"}"#.to_string(),
            stderr: String::new(),
            error: None,
        },
        /*turn_id*/ None,
    );

    assert_eq!(completed.completed.run.status, HookRunStatus::Completed);
    assert_eq!(completed.completed.run.entries, Vec::new());
}

fn handler(matcher: Option<&str>) -> ConfiguredHandler {
    ConfiguredHandler {
        event_name: HookEventName::SessionEnd,
        matcher: matcher.map(str::to_string),
        command: "echo hook".to_string(),
        timeout_sec: 2,
        status_message: None,
        additional_context_limit: Default::default(),
        source_path: test_path_buf("/tmp/hooks.json").abs(),
        source: HookSource::User,
        display_order: 0,
        env: HashMap::new(),
    }
}
