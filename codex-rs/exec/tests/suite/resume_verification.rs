#![allow(clippy::unwrap_used, clippy::expect_used)]
use core_test_support::test_codex_exec::test_codex_exec;
use predicates::prelude::*;
use predicates::str::contains;
use serde_json::Value;
use std::path::Path;
use std::string::ToString;
use uuid::Uuid;
use walkdir::WalkDir;

/// Utility: scan the sessions dir for a rollout file that contains `marker`
/// in any response_item.message.content entry. Returns the absolute path.
fn find_session_file_containing_marker(
    sessions_dir: &std::path::Path,
    marker: &str,
) -> Option<std::path::PathBuf> {
    for entry in WalkDir::new(sessions_dir) {
        let entry = match entry {
            Ok(e) => e,
            Err(_) => continue,
        };
        if !entry.file_type().is_file() {
            continue;
        }
        if !entry.file_name().to_string_lossy().ends_with(".jsonl") {
            continue;
        }
        let path = entry.path();
        let Ok(content) = std::fs::read_to_string(path) else {
            continue;
        };
        // Skip the first meta line and scan remaining JSONL entries.
        let mut lines = content.lines();
        if lines.next().is_none() {
            continue;
        }
        for line in lines {
            if line.trim().is_empty() {
                continue;
            }
            let Ok(item): Result<Value, _> = serde_json::from_str(line) else {
                continue;
            };
            if item.get("type").and_then(|t| t.as_str()) == Some("response_item")
                && let Some(payload) = item.get("payload")
                && payload.get("type").and_then(|t| t.as_str()) == Some("message")
                && payload
                    .get("content")
                    .map(ToString::to_string)
                    .unwrap_or_default()
                    .contains(marker)
            {
                return Some(path.to_path_buf());
            }
        }
    }
    None
}

/// Test the exact scenario from issue #6717:
/// `codex exec --json resume --last "2+2"` should work
#[test]
fn exec_resume_last_json_mode_exact_issue_scenario() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session
    let marker = format!("resume-json-issue-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");

    // 2) Test the exact command from issue #6717
    let marker2 = format!("resume-json-issue-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    // This is the exact command from the issue: codex exec --json resume --last "2+2"
    // But we'll use a more testable prompt
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("--json")
        .arg("resume")
        .arg("--last")
        .arg(&prompt2)
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .expect("no resumed session file containing marker2");
    assert_eq!(
        resumed_path, path,
        "resume --last should append to existing file"
    );
    let content = std::fs::read_to_string(&resumed_path)?;
    assert!(content.contains(&marker));
    assert!(content.contains(&marker2));
    Ok(())
}

/// Test security issue: UUID with --last should error
#[test]
fn exec_resume_uuid_with_last_should_error() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let session_id = Uuid::new_v4().to_string();

    // Test UUID with dashes - clap rejects this with conflicts_with
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(&session_id)
        .arg("--last")
        .assert()
        .failure()
        .stderr(contains("cannot be used with").or(contains("conflicts")));

    // Test UUID without dashes (32 hex chars) - clap rejects this with conflicts_with
    let session_id_no_dashes = session_id.replace('-', "");
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(&session_id_no_dashes)
        .arg("--last")
        .assert()
        .failure()
        .stderr(contains("cannot be used with").or(contains("conflicts")));

    Ok(())
}

/// Test that positional arguments before --last are rejected (simplified approach)
#[test]
fn exec_resume_rejects_positional_before_last() -> anyhow::Result<()> {
    let test = test_codex_exec();

    // With conflicts_with, clap rejects any positional argument before --last
    // This ensures users use the documented form: codex exec resume --last "prompt"
    let test_cases = vec![
        "2+2",                                  // Original issue example - simple math
        "hello world",                          // Simple text
        "12345678-1234-1234-1234-123456789012", // Wrong format (too many chars)
        "12345678-1234-1234-1234-1234567890",   // Wrong format (too few chars)
    ];

    for non_uuid_string in test_cases {
        // All of these should be rejected because clap treats them as session_id
        // and conflicts_with prevents session_id and --last from being used together
        test.cmd()
            .arg("--skip-git-repo-check")
            .arg("-C")
            .arg(env!("CARGO_MANIFEST_DIR"))
            .arg("resume")
            .arg(non_uuid_string)
            .arg("--last")
            .assert()
            .failure()
            .stderr(contains("cannot be used with").or(contains("conflicts")));
    }

    Ok(())
}

/// Test edge case: --last with value vs without value
#[test]
fn exec_resume_last_with_and_without_value() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session
    let marker = format!("resume-last-value-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");

    // 2) Test --last without value (should work, reads from parent prompt or stdin)
    let marker2 = format!("resume-last-value-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    // --last without value, prompt provided at parent level
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt2)
        .arg("resume")
        .arg("--last")
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .expect("no resumed session file containing marker2");
    assert_eq!(
        resumed_path, path,
        "resume --last should append to existing file"
    );

    Ok(())
}
