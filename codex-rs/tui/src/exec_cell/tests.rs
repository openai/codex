use super::*;
use crate::history_cell::HistoryCell;
use codex_core::protocol::ExecCommandSource;
use insta::assert_snapshot;

fn render(cell: &ExecCell) -> String {
    cell.display_lines(120)
        .into_iter()
        .map(|line| {
            line.spans
                .iter()
                .map(|s| s.content.to_string())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn make_cell(header: &str, render: SubagentCell, exit_code: i32) -> ExecCell {
    let command = vec![header.to_string()];
    let call = ExecCall {
        call_id: "c1".to_string(),
        subagent: Some(render),
        command,
        parsed: vec![],
        output: Some(CommandOutput {
            exit_code,
            aggregated_output: String::new(),
            formatted_output: String::new(),
        }),
        source: ExecCommandSource::Agent,
        is_user_shell_command: false,
        start_time: None,
        duration: None,
        interaction_input: None,
    };
    ExecCell::new(call, true)
}

#[test]
fn snapshot_spawn_with_model() {
    let cell = make_cell(
        "Spawned subagent exec-review",
        SubagentCell::Spawn {
            label: "exec-review".into(),
            model: Some("gpt-5.1-codex-mini".into()),
            summary: Some("Review exec changes".into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"
• Spawned subagent exec-review
  └ model=gpt-5.1-codex-mini
    "Review exec changes"
"###);
}

#[test]
fn snapshot_fork_prompt_only() {
    let cell = make_cell(
        "Forked subagent tui-review",
        SubagentCell::Fork {
            label: "tui-review".into(),
            model: None,
            summary: Some("Review TUI widgets".into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"
• Forked subagent tui-review
  └ "Review TUI widgets"
"###);
}

#[test]
fn snapshot_send_message() {
    let cell = make_cell(
        "Sent message to subagent tui-review",
        SubagentCell::SendMessage {
            label: "tui-review".into(),
            summary: Some("Remember stylize rules".into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"
• Sent message to subagent tui-review
  └ "Remember stylize rules"
"###);
}

#[test]
fn snapshot_await_complete() {
    let cell = make_cell(
        "Awaited subagent core-review",
        SubagentCell::Await {
            label: Some("core-review".into()),
            timed_out: Some(false),
            message: Some("Completed: \"finding summary\"".into()),
            lifecycle_status: Some("completed".into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"
• Awaited subagent core-review
  └ Completed: "finding summary"
"###);
}

#[test]
fn snapshot_await_timeout() {
    let cell = make_cell(
        "Awaited subagent core-review",
        SubagentCell::Await {
            label: Some("core-review".into()),
            timed_out: Some(true),
            message: None,
            lifecycle_status: Some("running".into()),
        },
        1,
    );
    assert_snapshot!(render(&cell), @r###"
• Awaited subagent core-review
  └ Timed out
"###);
}

#[test]
fn snapshot_logs() {
    let rendered = "Session s • status=idle • older_logs=false • at_latest=true\n2025-11-11T01:06:53.424Z Thinking: ** (1 delta)\n2025-11-11T01:06:53.442Z Reasoning summary: **Evaluating safe shell command execution**";
    let cell = make_cell(
        "Fetched subagent logs from core-review",
        SubagentCell::Logs {
            label: Some("core-review".into()),
            rendered: Some(rendered.into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"
• Fetched subagent logs from core-review
  └ Session s • status=idle • older_logs=false • at_latest=true
    2025-11-11T01:06:53.424Z Thinking: ** (1 delta)
    2025-11-11T01:06:53.442Z Reasoning summary: **Evaluating safe shell command execution**
"###);
}

#[test]
fn snapshot_cancel() {
    let cell = make_cell(
        "Canceled subagent core-review",
        SubagentCell::Cancel {
            label: Some("core-review".into()),
        },
        0,
    );
    assert_snapshot!(render(&cell), @r###"• Canceled subagent core-review"###);
}
