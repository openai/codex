#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Context;
use codex_utils_cargo_bin::find_resource;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::Value;
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

/// Extract the conversation UUID from the first SessionMeta line in the rollout file.
fn extract_conversation_id(path: &std::path::Path) -> String {
    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines();
    let meta_line = lines.next().expect("missing meta line");
    let meta: Value = serde_json::from_str(meta_line).expect("invalid meta json");
    meta.get("payload")
        .and_then(|p| p.get("id"))
        .and_then(|v| v.as_str())
        .unwrap_or_default()
        .to_string()
}

fn extract_forked_from_id(path: &std::path::Path) -> Option<String> {
    let content = std::fs::read_to_string(path).unwrap();
    let mut lines = content.lines();
    let meta_line = lines.next().expect("missing meta line");
    let meta: Value = serde_json::from_str(meta_line).expect("invalid meta json");
    meta.get("payload")
        .and_then(|payload| payload.get("forked_from_id"))
        .and_then(Value::as_str)
        .map(ToString::to_string)
}

fn rollout_contains_fork_reference(path: &std::path::Path) -> bool {
    let Ok(content) = std::fs::read_to_string(path) else {
        return false;
    };
    content.lines().skip(1).any(|line| {
        serde_json::from_str::<Value>(line)
            .ok()
            .and_then(|item| item.get("type").and_then(Value::as_str).map(str::to_string))
            .as_deref()
            == Some("fork_reference")
    })
}

fn exec_fixture() -> anyhow::Result<std::path::PathBuf> {
    Ok(find_resource!("tests/fixtures/cli_responses_fixture.sse")?)
}

#[test]
fn exec_fork_by_id_creates_new_session_with_copied_history() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;

    let marker = format!("fork-base-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg(&prompt)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let original_path = find_session_file_containing_marker(&sessions_dir, &marker)
        .context("no session file found after first run")?;
    let session_id = extract_conversation_id(&original_path);

    let marker2 = format!("fork-follow-up-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("--fork")
        .arg(&session_id)
        .arg(&prompt2)
        .assert()
        .success();

    let forked_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .context("no forked session file found for second marker")?;

    assert_ne!(
        forked_path, original_path,
        "fork should create a new session file"
    );

    let forked_content = std::fs::read_to_string(&forked_path)?;
    assert_eq!(
        extract_forked_from_id(&forked_path).as_deref(),
        Some(session_id.as_str())
    );
    assert!(
        forked_content.contains(&marker) || rollout_contains_fork_reference(&forked_path),
        "forked rollout should either inline parent history or record a fork reference"
    );
    assert!(forked_content.contains(&marker2));

    let original_content = std::fs::read_to_string(&original_path)?;
    assert!(original_content.contains(&marker));
    assert!(
        !original_content.contains(&marker2),
        "original session should not receive the forked prompt"
    );

    Ok(())
}
