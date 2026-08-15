#![allow(clippy::unwrap_used)]
use anyhow::Context;
use codex_core::config::ConfigBuilder;
use codex_core::init_state_db;
use codex_protocol::ThreadId;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::Value;
use std::process::Stdio;
use std::string::ToString;
use std::time::Duration;
use tempfile::TempDir;
use uuid::Uuid;
use walkdir::WalkDir;
use wiremock::MockServer;

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

fn exec_repo_root() -> anyhow::Result<std::path::PathBuf> {
    Ok(codex_utils_cargo_bin::repo_root()?)
}

fn exec_sse_response(index: usize) -> String {
    let response_id = format!("resp-exec-{index}");
    let message_id = format!("msg-exec-{index}");
    responses::sse(vec![
        responses::ev_response_created(&response_id),
        responses::ev_assistant_message(&message_id, "exec response"),
        responses::ev_completed(&response_id),
    ])
}

async fn mount_exec_responses(
    server: &MockServer,
    count: usize,
) -> core_test_support::responses::ResponseMock {
    responses::mount_sse_sequence(server, (0..count).map(exec_sse_response).collect()).await
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_falls_back_to_legacy_history_when_thread_store_cannot_paginate() -> anyhow::Result<()>
{
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 1).await;
    let store_id = Uuid::new_v4();

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-c")
        .arg(format!(
            "experimental_thread_store={{type=\"in_memory\",id=\"{store_id}\"}}"
        ))
        .arg("continue without paginated history")
        .assert()
        .success();

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_appends_to_existing_file() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let response_mock = responses::mount_sse_sequence(
        &server,
        vec![
            responses::sse(vec![
                responses::ev_response_created("resp-exec-0"),
                responses::ev_assistant_message("msg-exec-0", "exec response"),
                responses::ev_completed_with_tokens("resp-exec-0", /*total_tokens*/ 7),
            ]),
            exec_sse_response(/*index*/ 1),
        ],
    )
    .await;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd_with_server(&server)
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
    let content = std::fs::read_to_string(&path)?;
    let meta: Value = serde_json::from_str(
        content
            .lines()
            .next()
            .expect("rollout should contain session metadata"),
    )?;
    assert_eq!(meta["payload"]["history_mode"], "paginated");

    // 2) Second run: resume the most recent file with a new marker.
    let marker2 = format!("resume-last-2-{}", Uuid::new_v4());
    let prompt2 = format!("echo {marker2}");

    let output = test
        .cmd_with_server(&server)
        .env("RUST_LOG", "codex_app_server::outgoing_message=trace")
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(&prompt2)
        .arg("resume")
        .arg("--last")
        .output()
        .context("resume run should succeed")?;
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "resume failed: {stderr}");
    assert_eq!(
        stderr
            .matches("app-server event: thread/tokenUsage/updated")
            .count(),
        2,
        "paginated resume should replay restored token usage before the new turn: {stderr}"
    );

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
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let resumed_request = requests[1].body_json().to_string();
    assert!(resumed_request.contains(&marker));
    assert!(resumed_request.contains(&marker2));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_repairs_rollout_missing_from_state_db() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let repo_root = exec_repo_root()?;

    let marker = format!("resume-last-repair-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(format!("echo {marker}"))
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let path = find_session_file_containing_marker(&sessions_dir, &marker)
        .expect("no session file found after first run");
    let thread_id = ThreadId::from_string(&extract_conversation_id(&path))?;
    let config = ConfigBuilder::default()
        .codex_home(test.home_path().to_path_buf())
        .build()
        .await?;
    let state_db = init_state_db(&config)
        .await
        .expect("state DB should initialize");
    assert_eq!(state_db.delete_thread(thread_id).await?, 1);
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;

    let resumed_marker = format!("resume-last-repaired-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("resume")
        .arg("--last")
        .arg(format!("echo {resumed_marker}"))
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &resumed_marker)
        .expect("no resumed session file after SQLite repair");
    assert_eq!(resumed_path, path);
    assert!(state_db.get_thread(thread_id).await?.is_some());
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_trusts_usable_state_db_candidate() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 3).await;
    let repo_root = exec_repo_root()?;
    let sessions_dir = test.home_path().join("sessions");

    let older_marker = format!("resume-last-indexed-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(format!("echo {older_marker}"))
        .assert()
        .success();
    let older_path = find_session_file_containing_marker(&sessions_dir, &older_marker)
        .expect("no indexed session file after first run");

    let newer_marker = format!("resume-last-unindexed-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(format!("echo {newer_marker}"))
        .assert()
        .success();
    let newer_path = find_session_file_containing_marker(&sessions_dir, &newer_marker)
        .expect("no unindexed session file after second run");
    let newer_thread_id = ThreadId::from_string(&extract_conversation_id(&newer_path))?;

    let config = ConfigBuilder::default()
        .codex_home(test.home_path().to_path_buf())
        .build()
        .await?;
    let state_db = init_state_db(&config)
        .await
        .expect("state DB should initialize");
    assert_eq!(state_db.delete_thread(newer_thread_id).await?, 1);
    state_db
        .mark_backfill_complete(/*last_watermark*/ None)
        .await?;

    let resumed_marker = format!("resume-last-authoritative-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("resume")
        .arg("--last")
        .arg(format!("echo {resumed_marker}"))
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &resumed_marker)
        .expect("no resumed session file after SQLite lookup");
    assert_eq!(
        (
            resumed_path,
            state_db
                .get_thread(newer_thread_id)
                .await?
                .map(|metadata| metadata.id),
        ),
        (older_path, None),
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_skips_mismatched_state_db_candidate() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 3).await;
    let repo_root = exec_repo_root()?;
    let sessions_dir = test.home_path().join("sessions");

    let older_marker = format!("resume-last-valid-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(format!("echo {older_marker}"))
        .assert()
        .success();
    let older_path = find_session_file_containing_marker(&sessions_dir, &older_marker)
        .expect("no valid session file after first run");

    let newer_marker = format!("resume-last-mismatched-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg(format!("echo {newer_marker}"))
        .assert()
        .success();
    let newer_path = find_session_file_containing_marker(&sessions_dir, &newer_marker)
        .expect("no mismatched session file after second run");
    let newer_thread_id = ThreadId::from_string(&extract_conversation_id(&newer_path))?;

    let config = ConfigBuilder::default()
        .codex_home(test.home_path().to_path_buf())
        .build()
        .await?;
    let state_db = init_state_db(&config)
        .await
        .expect("state DB should initialize");
    let mut mismatched = state_db
        .get_thread(newer_thread_id)
        .await?
        .expect("newer thread should be indexed");
    mismatched.rollout_path = older_path.clone();
    state_db.upsert_thread(&mismatched).await?;

    let resumed_marker = format!("resume-last-valid-resumed-{}", Uuid::new_v4());
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(&repo_root)
        .arg("resume")
        .arg("--last")
        .arg(format!("echo {resumed_marker}"))
        .assert()
        .success();

    let resumed_path = find_session_file_containing_marker(&sessions_dir, &resumed_marker)
        .expect("no resumed session file after skipping mismatched SQLite row");
    assert_eq!(resumed_path, older_path);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_accepts_prompt_after_flag_in_json_mode() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session with a unique marker in the content.
    let marker = format!("resume-last-json-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd_with_server(&server)
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

    test.cmd_with_server(&server)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_last_respects_cwd_filter_and_all_flag() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 5).await;

    let dir_a = TempDir::new()?;
    let dir_b = TempDir::new()?;

    let marker_a = format!("resume-cwd-a-{}", Uuid::new_v4());
    let prompt_a = format!("echo {marker_a}");
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_a.path())
        .arg(&prompt_a)
        .assert()
        .success();

    let marker_b = format!("resume-cwd-b-{}", Uuid::new_v4());
    let prompt_b = format!("echo {marker_b}");
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_b.path())
        .arg(&prompt_b)
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    find_session_file_containing_marker(&sessions_dir, &marker_a)
        .expect("no session file found for marker_a");
    let path_b = find_session_file_containing_marker(&sessions_dir, &marker_b)
        .expect("no session file found for marker_b");

    // `updated_at` is second-granularity, so ensure the touch lands in a later second
    // than the initial session creation on fast CI (especially Windows).
    std::thread::sleep(std::time::Duration::from_millis(1100));

    // Make thread B deterministically newest according to rollout metadata.
    let session_id_b = extract_conversation_id(&path_b);
    let marker_b_touch = format!("resume-cwd-b-touch-{}", Uuid::new_v4());
    let prompt_b_touch = format!("echo {marker_b_touch}");
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(dir_b.path())
        .arg("resume")
        .arg(&session_id_b)
        .arg(&prompt_b_touch)
        .assert()
        .success();

    // `resume --last` sorts by `updated_at`, which is second-granularity. Sleep so
    // the upcoming `resume --last --all` write lands in a later second and becomes
    // deterministically newest (instead of tying and falling back to UUID order).
    std::thread::sleep(std::time::Duration::from_millis(1100));

    let marker_b2 = format!("resume-cwd-b-2-{}", Uuid::new_v4());
    let prompt_b2 = format!("echo {marker_b2}");
    test.cmd_with_server(&server)
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
    test.cmd_with_server(&server)
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
    // The `--all` resume above appends a new turn to `path_b` while running from `dir_a`, so the
    // session's latest cwd now matches `dir_a`. A subsequent `resume --last` should therefore pick
    // the newest matching session (`path_b`).
    assert_eq!(
        resumed_path_cwd, path_b,
        "resume --last should prefer sessions whose latest turn context matches the current cwd"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_accepts_global_flags_after_subcommand() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;

    // Seed a session.
    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("echo seed-resume-session")
        .assert()
        .success();

    // Resume while passing global flags after the subcommand to ensure clap accepts them.
    let base = format!("{}/v1", server.uri());
    let base_config = format!("openai_base_url={}", serde_json::to_string(&base)?);
    test.cmd()
        .arg("resume")
        .arg("--last")
        .arg("--config")
        .arg(base_config)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_includes_output_schema_in_request() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let response_mock = mount_exec_responses(&server, /*count*/ 2).await;

    let schema_contents = serde_json::json!({
        "type": "object",
        "properties": {
            "answer": { "type": "string" }
        },
        "required": ["answer"],
        "additionalProperties": false
    });
    let schema_path = test.cwd_path().join("schema.json");
    std::fs::write(&schema_path, serde_json::to_vec_pretty(&schema_contents)?)?;

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("echo seed-resume-session")
        .assert()
        .success();

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("resume")
        .arg("--last")
        .arg("--json")
        .arg("--output-schema")
        .arg(&schema_path)
        .arg("echo resume-with-schema")
        .assert()
        .success();

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let payload: Value = requests[1].body_json();
    let text = payload.get("text").expect("request missing text field");
    let format = text
        .get("format")
        .expect("request missing text.format field");
    assert_eq!(
        format,
        &serde_json::json!({
            "name": "codex_output_schema",
            "type": "json_schema",
            "strict": true,
            "schema": schema_contents,
        })
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_by_id_appends_to_existing_file() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let repo_root = exec_repo_root()?;

    // 1) First run: create a session
    let marker = format!("resume-by-id-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd_with_server(&server)
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

    test.cmd_with_server(&server)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_preserves_cli_configuration_overrides() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let repo_root = exec_repo_root()?;

    let marker = format!("resume-config-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd_with_server(&server)
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
        .cmd_with_server(&server)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_resume_accepts_images_after_subcommand() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let _response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let repo_root = exec_repo_root()?;

    let marker = format!("resume-image-{}", Uuid::new_v4());
    let prompt = format!("echo {marker}");

    test.cmd_with_server(&server)
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
    test.cmd_with_server(&server)
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn exec_fork_creates_distinct_threads_with_and_without_a_prompt() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let test = test_codex_exec();
    let server = MockServer::start().await;
    let response_mock = mount_exec_responses(&server, /*count*/ 2).await;
    let source_marker = format!("fork-source-{}", Uuid::new_v4());

    test.cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg(format!("echo {source_marker}"))
        .assert()
        .success();

    let sessions_dir = test.home_path().join("sessions");
    let source_path = find_session_file_containing_marker(&sessions_dir, &source_marker)
        .expect("source thread should have a rollout");
    let source_id = extract_conversation_id(&source_path);
    let original_source = std::fs::read_to_string(&source_path)?;

    for (args, expected_error) in [
        (
            vec!["--image", "unused.png"],
            "Forking with images requires a prompt",
        ),
        (
            vec!["--output-schema", "unused.json"],
            "Forking with output options requires a prompt",
        ),
        (
            vec!["--output-last-message", "unused.md"],
            "Forking with output options requires a prompt",
        ),
        (vec!["--ephemeral"], "Ephemeral forks require a prompt"),
    ] {
        let output = test
            .cmd_with_server(&server)
            .arg("--skip-git-repo-check")
            .arg("fork")
            .arg(&source_id)
            .args(args)
            .output()?;
        assert!(!output.status.success());
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected_error),
            "fork failed without the expected error: {output:?}"
        );
    }

    let mut promptless_command = test.cmd_with_server(&server);
    promptless_command
        .arg("--skip-git-repo-check")
        .arg("fork")
        .arg(&source_id)
        .arg("--json");
    let mut child_command = tokio::process::Command::new(promptless_command.get_program());
    child_command
        .args(promptless_command.get_args())
        .envs(
            promptless_command
                .get_envs()
                .filter_map(|(key, value)| value.map(|value| (key, value))),
        )
        .current_dir(test.cwd_path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = child_command.spawn()?;
    let _open_stdin = child.stdin.take().expect("stdin should be piped");
    let promptless_output =
        tokio::time::timeout(Duration::from_secs(/*secs*/ 10), child.wait_with_output())
            .await
            .context("promptless fork should not wait for stdin to close")??;
    assert!(
        promptless_output.status.success(),
        "promptless fork failed: {}",
        String::from_utf8_lossy(&promptless_output.stderr)
    );
    let promptless_events = String::from_utf8(promptless_output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(promptless_events.len(), 1);
    assert_eq!(promptless_events[0]["type"], "thread.started");
    let promptless_thread_id = promptless_events[0]["thread_id"]
        .as_str()
        .expect("promptless fork should emit its new thread id");
    assert_ne!(promptless_thread_id, source_id);
    assert_eq!(response_mock.requests().len(), 1);

    let source_name = format!("fork-named-{}", Uuid::new_v4());
    let config = ConfigBuilder::default()
        .codex_home(test.home_path().to_path_buf())
        .build()
        .await?;
    let state_db = init_state_db(&config)
        .await
        .expect("state DB should initialize");
    assert!(
        state_db
            .update_thread_title(ThreadId::from_string(&source_id)?, &source_name)
            .await?
    );

    let fork_marker = format!("fork-prompt-{}", Uuid::new_v4());
    let fork_output = test
        .cmd_with_server(&server)
        .arg("--skip-git-repo-check")
        .arg("-C")
        .arg(test.home_path())
        .arg("fork")
        .arg(&source_name)
        .arg("--json")
        .arg("-")
        .write_stdin(format!("echo {fork_marker}"))
        .output()?;
    assert!(
        fork_output.status.success(),
        "fork with prompt failed: {}",
        String::from_utf8_lossy(&fork_output.stderr)
    );
    let fork_events = String::from_utf8(fork_output.stdout)?
        .lines()
        .map(serde_json::from_str::<Value>)
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(fork_events[0]["type"], "thread.started");
    let fork_thread_id = fork_events[0]["thread_id"]
        .as_str()
        .expect("fork should emit its new thread id");
    assert_ne!(fork_thread_id, source_id);
    assert_ne!(fork_thread_id, promptless_thread_id);

    let fork_path = find_session_file_containing_marker(&sessions_dir, &fork_marker)
        .expect("forked thread should have a separate rollout");
    assert_ne!(fork_path, source_path);
    assert_eq!(extract_conversation_id(&fork_path), fork_thread_id);
    let fork_contents = std::fs::read_to_string(&fork_path)?;
    let fork_meta: Value = serde_json::from_str(
        fork_contents
            .lines()
            .next()
            .expect("fork rollout should contain session metadata"),
    )?;
    assert_eq!(fork_meta["payload"]["forked_from_id"], source_id);
    assert_eq!(fork_meta["payload"]["history_base"]["thread_id"], source_id);
    assert!(!fork_contents.contains(&source_marker));
    assert!(fork_contents.contains(&fork_marker));
    assert_eq!(std::fs::read_to_string(&source_path)?, original_source);

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let fork_request = requests[1].body_json().to_string();
    assert!(fork_request.contains(&source_marker));
    assert!(fork_request.contains(&fork_marker));

    Ok(())
}
