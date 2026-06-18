use super::*;
use crate::compression::compressed_rollout_path;
use codex_protocol::protocol::InternalSessionSource;
use codex_protocol::protocol::SessionSource;
use codex_protocol::protocol::SubAgentSource;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use tempfile::TempDir;
use uuid::Uuid;

struct Fixture {
    home: TempDir,
}

impl Fixture {
    fn new() -> Self {
        Self {
            home: TempDir::new().expect("create temp Codex home"),
        }
    }

    fn write_rollout(&self, archived: bool, source: SessionSource, lines: &[String]) -> PathBuf {
        let id = Uuid::new_v4();
        let root = if archived {
            self.home.path().join(ARCHIVED_SESSIONS_SUBDIR)
        } else {
            self.home.path().join(SESSIONS_SUBDIR).join("2026/06/18")
        };
        fs::create_dir_all(&root).expect("create rollout directory");
        let path = root.join(format!("rollout-2026-06-18T12-00-00-{id}.jsonl"));
        let mut file = fs::File::create(&path).expect("create rollout");
        writeln!(
            file,
            "{}",
            json!({
                "timestamp": "2026-06-18T12:00:00Z",
                "type": "session_meta",
                "payload": {
                    "id": id,
                    "timestamp": "2026-06-18T12:00:00Z",
                    "cwd": self.home.path(),
                    "originator": "test",
                    "cli_version": "test",
                    "source": source,
                    "model_provider": null,
                },
            })
        )
        .expect("write session metadata");
        for line in lines {
            writeln!(file, "{line}").expect("write rollout line");
        }
        path
    }
}

fn response_item(payload: Value) -> String {
    json!({
        "timestamp": "2026-06-18T12:00:01Z",
        "type": "response_item",
        "payload": payload,
    })
    .to_string()
}

fn function_call(call_id: &str, name: &str, arguments: Value) -> String {
    response_item(json!({
        "type": "function_call",
        "name": name,
        "arguments": arguments.to_string(),
        "call_id": call_id,
    }))
}

fn function_output(call_id: &str, output: &str) -> String {
    response_item(json!({
        "type": "function_call_output",
        "call_id": call_id,
        "output": output,
    }))
}

fn direct_command(call_id: &str, command: &str, output: &str) -> Vec<String> {
    vec![
        function_call(call_id, "exec_command", json!({"cmd": command})),
        function_output(call_id, output),
    ]
}

fn aggregate(
    key: &str,
    invocation_count: usize,
    continuation_wait_count: usize,
    total_seconds: u64,
    max_seconds: u64,
) -> SlowCommandAggregate {
    SlowCommandAggregate {
        key: key.to_string(),
        invocation_count,
        continuation_wait_count,
        total_wait: Duration::from_secs(total_seconds),
        max_wait: Duration::from_secs(max_seconds),
    }
}

#[tokio::test]
async fn analyzes_direct_shell_waits_across_root_and_subagent_rollouts() {
    let fixture = Fixture::new();
    let mut root_lines = Vec::new();
    root_lines.extend(direct_command(
        "exec-background",
        "cargo test -p codex-core",
        "Wall time: 10.0000 seconds\nProcess running with session ID 42\nOutput:",
    ));
    root_lines.push(function_call(
        "poll-background",
        "write_stdin",
        json!({"session_id": 42, "chars": ""}),
    ));
    root_lines.push(function_output(
        "poll-background",
        "Wall time: 5.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    root_lines.extend(direct_command(
        "exec-check",
        "cargo check",
        "Wall time: 2.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    root_lines.extend(direct_command(
        "exec-rg",
        "/usr/bin/rg TODO",
        "Wall time: 3.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    root_lines.extend(direct_command(
        "exec-open",
        "sleep 100",
        "Wall time: 1.0000 seconds\nProcess running with session ID 77\nOutput:",
    ));
    root_lines.extend(direct_command(
        "exec-bad-output",
        "echo missing-time",
        "Process exited with code 0\nOutput:",
    ));
    root_lines.push(response_item(json!({
        "type": "function_call",
        "name": "exec_command",
        "arguments": "{malformed",
        "call_id": "exec-bad-arguments",
    })));
    root_lines.push(function_output(
        "exec-bad-arguments",
        "Wall time: 8.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    root_lines.push(function_call(
        "orphan-poll",
        "write_stdin",
        json!({"session_id": 999, "chars": ""}),
    ));
    root_lines.push(function_output(
        "orphan-poll",
        "Wall time: 4.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    root_lines.push(response_item(json!({
        "type": "custom_tool_call",
        "call_id": "code-cell",
        "name": "exec",
        "input": "await tools.exec_command({cmd: 'ignored'})",
    })));
    root_lines.push(response_item(json!({
        "type": "message",
        "role": "user",
        "content": [{"type": "input_text", "text": "<user_shell_command>ignored</user_shell_command>"}],
    })));
    root_lines.push("{malformed".to_string());
    fixture.write_rollout(/*archived*/ false, SessionSource::Cli, &root_lines);

    let subagent_lines = direct_command(
        "subagent-test",
        "cargo test -p codex-core",
        "Duration: 3.0000 seconds\nOutput:",
    );
    fixture.write_rollout(
        /*archived*/ true,
        SessionSource::SubAgent(SubAgentSource::Review),
        &subagent_lines,
    );

    let internal_lines = direct_command(
        "internal-call",
        "cargo test --workspace",
        "Wall time: 100.0000 seconds\nProcess exited with code 0\nOutput:",
    );
    fixture.write_rollout(
        /*archived*/ false,
        SessionSource::Internal(InternalSessionSource::MemoryConsolidation),
        &internal_lines,
    );

    let analysis = analyze_slow_commands(fixture.home.path()).await;

    assert_eq!(
        analysis,
        SlowCommandAnalysis {
            summary: SlowCommandSummary {
                rollout_files_seen: 3,
                rollout_files_analyzed: 2,
                rollout_files_skipped: 0,
                internal_rollout_files_skipped: 1,
                rollout_lines_skipped: 1,
                direct_shell_calls_seen: 7,
                direct_shell_calls_analyzed: 5,
                direct_shell_calls_skipped: 2,
                continuation_calls_seen: 2,
                continuation_calls_attributed: 1,
                continuation_calls_skipped: 1,
                duplicate_call_ids: 0,
                open_processes_at_eof: 1,
                code_mode_cells_ignored: 1,
                total_wait: Duration::from_secs(24),
            },
            exact_commands: vec![
                aggregate("cargo test -p codex-core", 2, 1, 18, 15),
                aggregate("/usr/bin/rg TODO", 1, 0, 3, 3),
                aggregate("cargo check", 1, 0, 2, 2),
                aggregate("sleep 100", 1, 0, 1, 1),
            ],
            command_families: vec![
                aggregate("cargo", 3, 1, 20, 15),
                aggregate("rg", 1, 0, 3, 3),
                aggregate("sleep", 1, 0, 1, 1),
            ],
        }
    );
}

#[tokio::test]
async fn sums_full_waits_for_overlapping_background_processes() {
    let fixture = Fixture::new();
    let mut lines = Vec::new();
    lines.extend(direct_command(
        "first",
        "cargo test first",
        "Wall time: 10.0000 seconds\nProcess running with session ID 1\nOutput:",
    ));
    lines.extend(direct_command(
        "second",
        "cargo test second",
        "Wall time: 7.0000 seconds\nProcess running with session ID 2\nOutput:",
    ));
    for (call_id, session_id, seconds) in [("poll-first", 1, 5), ("poll-second", 2, 4)] {
        lines.push(function_call(
            call_id,
            "write_stdin",
            json!({"session_id": session_id, "chars": ""}),
        ));
        lines.push(function_output(
            call_id,
            &format!("Wall time: {seconds}.0000 seconds\nProcess exited with code 0\nOutput:"),
        ));
    }
    fixture.write_rollout(/*archived*/ false, SessionSource::Cli, &lines);

    let analysis = analyze_slow_commands(fixture.home.path()).await;

    assert_eq!(analysis.summary.total_wait, Duration::from_secs(26));
    assert_eq!(
        analysis.command_families,
        vec![aggregate("cargo", 2, 2, 26, 15)]
    );
}

#[tokio::test]
async fn merges_overlapping_collections_by_direct_and_continuation_call_id() {
    let first_fixture = Fixture::new();
    let second_fixture = Fixture::new();
    let mut shared_lines = direct_command(
        "shared-direct",
        "cargo test merge",
        "Wall time: 10.0000 seconds\nProcess running with session ID 42\nOutput:",
    );
    shared_lines.push(function_call(
        "shared-poll",
        "write_stdin",
        json!({"session_id": 42, "chars": ""}),
    ));
    shared_lines.push(function_output(
        "shared-poll",
        "Wall time: 5.0000 seconds\nProcess running with session ID 42\nOutput:",
    ));
    first_fixture.write_rollout(/*archived*/ false, SessionSource::Cli, &shared_lines);

    let mut completed_lines = shared_lines;
    completed_lines.push(function_call(
        "terminal-poll",
        "write_stdin",
        json!({"session_id": 42, "chars": ""}),
    ));
    completed_lines.push(function_output(
        "terminal-poll",
        "Wall time: 7.0000 seconds\nProcess exited with code 0\nOutput:",
    ));
    second_fixture.write_rollout(
        /*archived*/ false,
        SessionSource::Cli,
        &completed_lines,
    );

    let first = collect_slow_commands(first_fixture.home.path()).await;
    let second = collect_slow_commands(second_fixture.home.path()).await;
    let analysis = merge_slow_command_collections([first, second]);

    assert_eq!(
        analysis,
        SlowCommandAnalysis {
            summary: SlowCommandSummary {
                rollout_files_seen: 2,
                rollout_files_analyzed: 2,
                direct_shell_calls_seen: 1,
                direct_shell_calls_analyzed: 1,
                continuation_calls_seen: 2,
                continuation_calls_attributed: 2,
                duplicate_call_ids: 2,
                total_wait: Duration::from_secs(22),
                ..Default::default()
            },
            exact_commands: vec![aggregate("cargo test merge", 1, 2, 22, 22)],
            command_families: vec![aggregate("cargo", 1, 2, 22, 22)],
        }
    );
}

#[tokio::test]
async fn reads_compressed_history_and_deduplicates_copied_calls_and_siblings() {
    let fixture = Fixture::new();
    let lines = direct_command(
        "copied-call",
        "just test -p codex-rollout",
        "Wall time: 4.0000 seconds\nProcess exited with code 0\nOutput:",
    );
    let active_path = fixture.write_rollout(/*archived*/ false, SessionSource::Cli, &lines);
    let active_contents = fs::read(&active_path).expect("read active rollout");
    fs::write(
        compressed_rollout_path(&active_path),
        zstd::stream::encode_all(active_contents.as_slice(), 0).expect("compress active rollout"),
    )
    .expect("write compressed sibling");

    let archived_path = fixture.write_rollout(/*archived*/ true, SessionSource::Cli, &lines);
    let archived_contents = fs::read(&archived_path).expect("read archived rollout");
    let archived_compressed = compressed_rollout_path(&archived_path);
    fs::write(
        &archived_compressed,
        zstd::stream::encode_all(archived_contents.as_slice(), 0)
            .expect("compress archived rollout"),
    )
    .expect("write compressed archived rollout");
    fs::remove_file(archived_path).expect("remove plain archived rollout");

    let analysis = analyze_slow_commands(fixture.home.path()).await;

    assert_eq!(analysis.summary.rollout_files_seen, 2);
    assert_eq!(analysis.summary.rollout_files_analyzed, 2);
    assert_eq!(analysis.summary.duplicate_call_ids, 1);
    assert_eq!(analysis.summary.direct_shell_calls_seen, 1);
    assert_eq!(analysis.summary.direct_shell_calls_analyzed, 1);
    assert_eq!(analysis.summary.total_wait, Duration::from_secs(4));
    assert_eq!(
        analysis.exact_commands,
        vec![aggregate("just test -p codex-rollout", 1, 0, 4, 4)]
    );
}

#[tokio::test]
async fn empty_home_returns_an_empty_analysis() {
    let fixture = Fixture::new();

    assert_eq!(
        analyze_slow_commands(fixture.home.path()).await,
        SlowCommandAnalysis::default()
    );
}

#[test]
fn executable_family_uses_shell_parsing_and_cross_platform_path_normalization() {
    assert_eq!(
        parsing::first_executable("FOO=bar /usr/local/bin/cargo test"),
        "cargo"
    );
    assert_eq!(
        parsing::normalize_executable(r"C:\Tools\cargo.EXE"),
        "cargo"
    );
    assert_eq!(parsing::first_executable(""), "<unknown>");
}
