#![allow(clippy::unwrap_used, clippy::expect_used)]
use anyhow::Context;
use codex_utils_cargo_bin::find_resource;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::fs::FileTimes;
use std::fs::OpenOptions;
use std::string::ToString;
use std::time::Duration;
use std::time::SystemTime;
use tempfile::TempDir;
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

fn last_user_image_count(path: &std::path::Path) -> usize {
    let content = std::fs::read_to_string(path).unwrap_or_default();
    let mut last_count = 0;
    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        let Ok(item): Result<Value, _> = serde_json::from_str(line) else {
            continue;
        };
        if item.get("type").and_then(|t| t.as_str()) != Some("response_item") {
            continue;
        }
        let Some(payload) = item.get("payload") else {
            continue;
        };
        if payload.get("type").and_then(|t| t.as_str()) != Some("message") {
            continue;
        }
        if payload.get("role").and_then(|r| r.as_str()) != Some("user") {
            continue;
        }
        let Some(content_items) = payload.get("content").and_then(|v| v.as_array()) else {
            continue;
        };
        last_count = content_items
            .iter()
            .filter(|entry| entry.get("type").and_then(|t| t.as_str()) == Some("input_image"))
            .count();
    }
    last_count
}

fn exec_fixture() -> anyhow::Result<std::path::PathBuf> {
    Ok(find_resource!("tests/fixtures/cli_responses_fixture.sse")?)
}

fn exec_repo_root() -> anyhow::Result<std::path::PathBuf> {
    Ok(find_resource!(".")?)
}

#[test]
fn exec_resume_last_appends_to_existing_file() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
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
        .arg(&repo_root)
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
    let fixture = exec_fixture()?;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-json-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
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
        .arg(&repo_root)
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
fn exec_resume_last_respects_cwd_filter_and_all_flag() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;

    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let marker_a = format!("resume-cwd-a-{}", Uuid::new_v4());
    let prompt_a = format!("echo {marker_a}");
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_a.path())
        .arg(&prompt_a)
        .assert()
        .success();

    let marker_b = format!("resume-cwd-b-{}", Uuid::new_v4());
    let prompt_b = format!("echo {marker_b}");
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_b.path())
        .arg(&prompt_b)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let path_a = find_session_file_containing_marker(&sessions_dir, &marker_a)
        .expect("no session file found for marker_a");
    let path_b = find_session_file_containing_marker(&sessions_dir, &marker_b)
        .expect("no session file found for marker_b");

    // Files are ordered by `updated_at`, then by `uuid`.
    // We mutate the mtimes to ensure file_b is the newest file.
    let file_a = OpenOptions::new().write(true).open(&path_a)?;
    file_a.set_times(
        FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(1)),
    )?;
    let file_b = OpenOptions::new().write(true).open(&path_b)?;
    file_b.set_times(
        FileTimes::new().set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(2)),
    )?;

    let marker_b2 = format!("resume-cwd-b-2-{}", Uuid::new_v4());
    let prompt_b2 = format!("echo {marker_b2}");
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_a.path())
        .arg("resume")
        .arg("--last")
        .arg("--all")
        .arg(&prompt_b2)
        .assert()
        .success();

    let resumed_path_all = find_session_file_containing_marker(&sessions_dir, &marker_b2)
        .expect("no resumed session file containing marker_b2");
    assert_eq!(
        resumed_path_all, path_b,
        "resume --last --all should pick newest session"
    );

    let marker_a2 = format!("resume-cwd-a-2-{}", Uuid::new_v4());
    let prompt_a2 = format!("echo {marker_a2}");
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_a.path())
        .arg("resume")
        .arg("--last")
        .arg(&prompt_a2)
        .assert()
        .success();

    let resumed_path_cwd = find_session_file_containing_marker(&sessions_dir, &marker_a2)
        .expect("no resumed session file containing marker_a2");
    assert_eq!(
        resumed_path_cwd, path_a,
        "resume --last should prefer sessions from the same cwd"
    );

    Ok(())
}

#[test]
fn exec_resume_accepts_global_flags_after_subcommand() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;

    // Seed a session.
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("echo seed-resume-session")
        .assert()
        .success();

    // Resume while passing global flags after the subcommand to ensure clap accepts them.
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("resume")
        .arg("--last")
        .arg("--json")
        .arg("--model")
        .arg("gpt-5.2-codex")
        .arg("--config")
        .arg("reasoning_level=xhigh")
        .arg("--dangerously-bypass-approvals-and-sandbox")
        .arg("--skip-git-repo-check")
        .arg("echo resume-with-global-flags-after-subcommand")
        .assert()
        .success();

    Ok(())
}

#[test]
fn exec_resume_by_id_appends_to_existing_file() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session
    let marker = format!("resume-by-id-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
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
        .arg(&repo_root)
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
    let fixture = exec_fixture()?;
    let repo_root = exec_repo_root()?;

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
        .arg(&repo_root)
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
        .arg(&repo_root)
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
fn exec_resume_accepts_images_after_subcommand() -> anyhow::Result<()> {
    let test = test_codex_exec();
    let fixture = exec_fixture()?;
    let repo_root = exec_repo_root()?;

    let marker = format!("resume-image-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(&prompt)
        .assert()
        .success();

    let image_path = test.cwd_path().join("resume_image.png");
    let image_path_2 = test.cwd_path().join("resume_image_2.png");
    let image_bytes: &[u8] = &[
        0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, 0x49, 0x48, 0x44,
        0x52, 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00, 0x00, 0x1F,
        0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0A, 0x49, 0x44, 0x41, 0x54, 0x78, 0x9C, 0x63, 0x00,
        0x01, 0x00, 0x00, 0x05, 0x00, 0x01, 0x0D, 0x0A, 0x2D, 0xB4, 0x00, 0x00, 0x00, 0x00, 0x49,
        0x45, 0x4E, 0x44, 0xAE, 0x42, 0x60, 0x82,
    ];
    std::fs::write(&image_path, image_bytes)?;
    std::fs::write(&image_path_2, image_bytes)?;

    let marker2 = format!("resume-image-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");
    test.cmd()
        .env("CODEX_RS_SSE_FIXTURE", &fixture)
        .env("OPENAI_BASE_URL", "http://unused.local")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("resume")
        .arg("--last")
        .arg("--image")
        .arg(&image_path)
        .arg("--image")
        .arg(&image_path_2)
        .arg(&prompt2)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let resumed_path = find_session_file_containing_marker(&sessions_dir, &marker2)
        .expect("no session file found after resume with images");
    let image_count = last_user_image_count(&resumed_path);
    assert_eq!(
        image_count, 2,
        "resume prompt should include both attached images"
    );

    Ok(())
}
