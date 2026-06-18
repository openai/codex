use anyhow::Result;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

fn write_rollout(codex_home: &Path) -> Result<()> {
    let id = "019ed917-334f-7393-8070-2b72069cd533";
    let dir = codex_home.join("sessions/2026/06/18");
    fs::create_dir_all(&dir)?;
    let mut file = fs::File::create(dir.join(format!("rollout-2026-06-18T12-00-00-{id}.jsonl")))?;
    for value in [
        json!({
            "timestamp": "2026-06-18T12:00:00Z",
            "type": "session_meta",
            "payload": {
                "id": id,
                "timestamp": "2026-06-18T12:00:00Z",
                "cwd": codex_home,
                "originator": "test",
                "cli_version": "test",
                "source": "cli",
                "model_provider": null,
            },
        }),
        call("test", "cargo test -p codex-cli"),
        output(
            "test",
            "Wall time: 1.5000 seconds\nProcess exited with code 0\nOutput:",
        ),
        call("check", "cargo check"),
        output(
            "check",
            "Wall time: 0.5000 seconds\nProcess exited with code 0\nOutput:",
        ),
    ] {
        writeln!(file, "{value}")?;
    }
    Ok(())
}

fn call(call_id: &str, command: &str) -> Value {
    json!({
        "timestamp": "2026-06-18T12:00:01Z",
        "type": "response_item",
        "payload": {
            "type": "function_call",
            "name": "exec_command",
            "arguments": json!({"cmd": command}).to_string(),
            "call_id": call_id,
        },
    })
}

fn output(call_id: &str, text: &str) -> Value {
    json!({
        "timestamp": "2026-06-18T12:00:02Z",
        "type": "response_item",
        "payload": {"type": "function_call_output", "call_id": call_id, "output": text},
    })
}

fn run(codex_home: &Path) -> Result<String> {
    run_with(codex_home, &[], None)
}

fn run_with(codex_home: &Path, args: &[&str], stdin: Option<&str>) -> Result<String> {
    let mut command = assert_cmd::Command::new(codex_utils_cargo_bin::cargo_bin("codex")?);
    command
        .env("CODEX_HOME", codex_home)
        .args(["debug", "slow-commands"])
        .args(args);
    if let Some(stdin) = stdin {
        command.write_stdin(stdin);
    }
    let output = command.output()?;
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8(output.stdout)?;
    for line in stdout.lines() {
        serde_json::from_str::<Value>(line)?;
    }
    Ok(stdout)
}

#[test]
fn slow_commands_prints_versioned_ranked_jsonl() -> Result<()> {
    let home = TempDir::new()?;
    write_rollout(home.path())?;

    let stdout = run(home.path())?;

    assert_eq!(
        stdout.lines().collect::<Vec<_>>(),
        vec![
            r#"{"type":"summary","schema_version":1,"rollout_files_seen":1,"rollout_files_analyzed":1,"rollout_files_skipped":0,"internal_rollout_files_skipped":0,"rollout_lines_skipped":0,"direct_shell_calls_seen":2,"direct_shell_calls_analyzed":2,"direct_shell_calls_skipped":0,"continuation_calls_seen":0,"continuation_calls_attributed":0,"continuation_calls_skipped":0,"duplicate_call_ids":0,"open_processes_at_eof":0,"code_mode_cells_ignored":0,"total_wait_seconds":2.0000}"#,
            r#"{"type":"exact_command","schema_version":1,"rank":1,"command":"cargo test -p codex-cli","invocation_count":1,"continuation_wait_count":0,"total_wait_seconds":1.5000,"average_wait_seconds":1.5000,"max_wait_seconds":1.5000}"#,
            r#"{"type":"exact_command","schema_version":1,"rank":2,"command":"cargo check","invocation_count":1,"continuation_wait_count":0,"total_wait_seconds":0.5000,"average_wait_seconds":0.5000,"max_wait_seconds":0.5000}"#,
            r#"{"type":"command_family","schema_version":1,"rank":1,"executable":"cargo","invocation_count":2,"continuation_wait_count":0,"total_wait_seconds":2.0000,"average_wait_seconds":1.0000,"max_wait_seconds":1.5000}"#,
        ]
    );
    assert!(stdout.ends_with('\n'));
    Ok(())
}

#[test]
fn slow_commands_empty_home_prints_only_summary() -> Result<()> {
    let home = TempDir::new()?;

    let stdout = run(home.path())?;

    assert_eq!(
        stdout,
        concat!(
            r#"{"type":"summary","schema_version":1,"rollout_files_seen":0,"rollout_files_analyzed":0,"rollout_files_skipped":0,"internal_rollout_files_skipped":0,"rollout_lines_skipped":0,"direct_shell_calls_seen":0,"direct_shell_calls_analyzed":0,"direct_shell_calls_skipped":0,"continuation_calls_seen":0,"continuation_calls_attributed":0,"continuation_calls_skipped":0,"duplicate_call_ids":0,"open_processes_at_eof":0,"code_mode_cells_ignored":0,"total_wait_seconds":0.0000}"#,
            "\n"
        )
    );
    Ok(())
}

#[test]
fn slow_commands_collections_round_trip_and_deduplicate_across_machines() -> Result<()> {
    let first_home = TempDir::new()?;
    let second_home = TempDir::new()?;
    write_rollout(first_home.path())?;
    write_rollout(second_home.path())?;

    let first = run_with(first_home.path(), &["collect"], None)?;
    let second = run_with(second_home.path(), &["collect"], None)?;
    let collected = first
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(collected.len(), 3);
    assert_eq!(collected[0]["type"], "collection_summary");
    assert_eq!(collected[1]["type"], "direct_call");
    assert_eq!(collected[1]["call_id"], "check");
    assert_eq!(collected[1]["wait_seconds"].as_f64(), Some(0.5));

    let input = format!("{first}{second}");
    let merged = run_with(first_home.path(), &["merge"], Some(&input))?;
    let records = merged
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<serde_json::Result<Vec<_>>>()?;

    assert_eq!(records.len(), 4);
    assert_eq!(records[0]["type"], "summary");
    assert_eq!(records[0]["rollout_files_seen"], 2);
    assert_eq!(records[0]["direct_shell_calls_seen"], 2);
    assert_eq!(records[0]["duplicate_call_ids"], 2);
    assert_eq!(records[0]["total_wait_seconds"].as_f64(), Some(2.0));
    assert_eq!(records[1]["command"], "cargo test -p codex-cli");
    assert_eq!(records[1]["invocation_count"], 1);
    assert_eq!(records[3]["executable"], "cargo");
    assert_eq!(records[3]["invocation_count"], 2);
    Ok(())
}
