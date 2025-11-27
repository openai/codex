#![allow(clippy::unwrap_used, clippy::expect_used)]
use anyhow::Context;
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

#[test]
fn exec_resume_last_appends_to_existing_file() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-{}", Uuid::new_v4());
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

    // Find the created session file containing the marker.
    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");

    // 2) Second run: resume the most recent file with a new marker.
    let marker2 = format!("resume-last-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

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

    // Ensure the same file was updated and contains both markers.
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

#[test]
fn exec_resume_last_accepts_prompt_after_flag_in_json_mode() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-json-{}", Uuid::new_v4());
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

    // Find the created session file containing the marker.
    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");

    // 2) Second run: resume the most recent file and pass the prompt after --last.
    let marker2 = format!("resume-last-json-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

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

#[test]
fn exec_resume_last_conflicts_with_session_id() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let session_id = Uuid::new_v4().to_string();

    // With conflicts_with, clap rejects this combination immediately
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

    Ok(())
}

#[test]
fn exec_resume_by_id_appends_to_existing_file() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session
    let marker = format!("resume-by-id-{}", Uuid::new_v4());
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
    let session_id = extract_conversation_id(&path);
    assert!(
        !session_id.is_empty(),
        "missing conversation id in meta line"
    );

    // 2) Resume by id
    let marker2 = format!("resume-by-id-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt2)
        .arg("resume")
        .arg(&session_id)
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .expect("no resumed session file containing marker2");
    assert_eq!(
        resumed_path, path,
        "resume by id should append to existing file"
    );
    let content = std::fs::read_to_string(&resumed_path)?;
    assert!(content.contains(&marker));
    assert!(content.contains(&marker2));
    Ok(())
}

#[test]
fn exec_resume_preserves_cli_configuration_overrides() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    let marker = format!("resume-config-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--model")
        .arg("gpt-5.1")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");

    let marker2 = format!("resume-config-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    let output = test
        .cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("--sandbox")
        .arg("workspace-write")
        .arg("--model")
        .arg("gpt-5.1-high")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg(&prompt2)
        .arg("resume")
        .arg("--last")
        .output()
        .context("resume run should succeed")?;

    assert!(output.status.success(), "resume run failed: {output:?}");

    let stderr = String::from_utf8(output.stderr)?;
    assert!(
        stderr.contains("model: gpt-5.1-high"),
        "stderr missing model override: {stderr}"
    );
    if cfg!(target_os = "windows") {
        assert!(
            stderr.contains("sandbox: read-only"),
            "stderr missing downgraded sandbox note: {stderr}"
        );
    } else {
        assert!(
            stderr.contains("sandbox: workspace-write"),
            "stderr missing sandbox override: {stderr}"
        );
    }

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .expect("no resumed session file containing marker2");
    assert_eq!(resumed_path, path, "resume should append to same file");

    let content = std::fs::read_to_string(&resumed_path)?;
    assert!(content.contains(&marker));
    assert!(content.contains(&marker2));
    Ok(())
}

#[test]
fn exec_resume_last_with_prompt_as_positional_after_flag() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/cli_responses_fixture.sse");

    // 1) First run: create a session
    let marker = format!("resume-last-positional-{}", Uuid::new_v4());
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

    // 2) Resume with --last and prompt as separate positional (simulating JSON mode issue)
    let marker2 = format!("resume-last-positional-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    // This simulates: codex exec resume --last "prompt"
    // where clap might parse "prompt" as session_id instead of as --last value
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
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

#[test]
fn exec_resume_last_rejects_real_uuid_as_session_id() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let session_id = Uuid::new_v4().to_string();

    // This should fail because we're providing both --last and a real UUID as session_id
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(&session_id)
        .arg("--last")
        .assert()
        .failure()
        .stderr(contains("cannot be used with"));

    Ok(())
}

#[test]
fn exec_resume_last_rejects_positional_before_flag() -> anyhow::Result<()> {
    // With conflicts_with, clap rejects any positional argument before --last
    // This ensures users use the documented form: codex exec resume --last "prompt"
    let test = test_codex_exec();
    let prompt = "echo test";

    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(prompt) // This will be parsed as session_id by clap
        .arg("--last")
        .assert()
        .failure()
        .stderr(contains("cannot be used with").or(contains("conflicts")));

    Ok(())
}

#[test]
fn exec_resume_rejects_invalid_session_id() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let invalid_id = "sdfsdfgfds2546456";

    // Invalid session ID should fail with an error message
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(invalid_id)
        .arg("2+2")
        .assert()
        .failure()
        .stderr(contains("No saved session found with ID"));

    Ok(())
}

#[test]
fn exec_resume_rejects_nonexistent_valid_uuid() -> anyhow::Result<()> {
    let test = test_codex_exec();
    // Generate a valid UUID that doesn't exist
    let nonexistent_id = Uuid::new_v4().to_string();

    // Valid UUID format but not found should fail with an error message
    test.cmd()
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(env!("CARGO_MANIFEST_DIR"))
        .arg("resume")
        .arg(&nonexistent_id)
        .arg("2+2")
        .assert()
        .failure()
        .stderr(contains("No saved session found with ID"));

    Ok(())
}
