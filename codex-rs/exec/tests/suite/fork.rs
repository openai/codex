#![allow(clippy::unwrap_used, clippy::expect_used)]

use anyhow::Context;
use codex_core::resolve_fork_reference_rollout_path;
use codex_core::RolloutRecorder;
use codex_protocol::models::ResponseItem;
use codex_protocol::protocol::RolloutItem;
use codex_utils_cargo_bin::find_resource;
use core_test_support::test_codex_exec::test_codex_exec;
use serde_json::Value;
use std::collections::HashSet;
use std::path::Path;
use std::path::PathBuf;
use std::string::ToString;
use tokio::runtime::Runtime;
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

fn exec_fixture() -> anyhow::Result<std::path::PathBuf> {
    Ok(find_resource!("tests/fixtures/cli_responses_fixture.sse")?)
}

fn rollout_items_contain_marker(
    rollout_items: &[RolloutItem],
    codex_home: &Path,
    marker: &str,
    runtime: &Runtime,
    visited_paths: &mut HashSet<PathBuf>,
) -> anyhow::Result<bool> {
    for item in rollout_items {
        match item {
            RolloutItem::ResponseItem(ResponseItem::Message { content, .. })
                if serde_json::to_string(content)?.contains(marker) =>
            {
                return Ok(true);
            }
            RolloutItem::ForkReference(reference) => {
                let resolved_path = runtime.block_on(resolve_fork_reference_rollout_path(
                    codex_home,
                    &reference.rollout_path,
                ))?;
                if !visited_paths.insert(resolved_path.clone()) {
                    continue;
                }
                let parent_history =
                    runtime.block_on(RolloutRecorder::get_rollout_history(&resolved_path))?;
                if rollout_items_contain_marker(
                    &parent_history.get_rollout_items(),
                    codex_home,
                    marker,
                    runtime,
                    visited_paths,
                )? {
                    return Ok(true);
                }
            }
            RolloutItem::SessionMeta(_)
            | RolloutItem::ResponseItem(_)
            | RolloutItem::Compacted(_)
            | RolloutItem::TurnContext(_)
            | RolloutItem::EventMsg(_) => {}
        }
    }

    Ok(false)
}

fn session_history_contains_marker(
    session_path: &Path,
    codex_home: &Path,
    marker: &str,
) -> anyhow::Result<bool> {
    let runtime = Runtime::new()?;
    let history = runtime.block_on(RolloutRecorder::get_rollout_history(session_path))?;
    let mut visited_paths = HashSet::from([session_path.to_path_buf()]);
    rollout_items_contain_marker(
        &history.get_rollout_items(),
        codex_home,
        marker,
        &runtime,
        &mut visited_paths,
    )
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

    assert!(session_history_contains_marker(
        &forked_path,
        test.home_path(),
        &marker
    )?);
    assert!(session_history_contains_marker(
        &forked_path,
        test.home_path(),
        &marker2
    )?);

    let original_content = std::fs::read_to_string(&original_path)?;
    assert!(original_content.contains(&marker));
    assert!(
        !original_content.contains(&marker2),
        "original session should not receive the forked prompt"
    );

    Ok(())
}
