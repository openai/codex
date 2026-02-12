#![cfg(not(target_os = "windows"))]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::CodexThread;
use codex_core::features::Feature;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::UndoCompletedEvent;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

#[allow(clippy::expect_used)]
async fn undo_harness() -> Result<TestCodexHarness> {
    let builder = test_codex().with_model("gpt-5.1").with_config(|config| {
        config.include_apply_patch_tool = true;
        config.features.enable(Feature::GhostCommit);
    });
    TestCodexHarness::with_builder(builder).await
}

fn git(path: &Path, args: &[&str]) -> Result<()> {
    let status = Command::new("git")
        .args(args)
        .current_dir(path)
        .status()
        .with_context(|| format!("failed to run git {args:?}"))?;
    if status.success() {
        return Ok(());
    }
    let exit_status = status;
    bail!("git {args:?} exited with {exit_status}");
}

fn git_output(path: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .with_context(|| format!("failed to run git {args:?}"))?;
    if !output.status.success() {
        let exit_status = output.status;
        bail!("git {args:?} exited with {exit_status}");
    }
    String::from_utf8(output.stdout).context("stdout was not valid utf8")
}

fn init_git_repo(path: &Path) -> Result<()> {
    // Use a consistent initial branch and config across environments to avoid
    // CI variance (default-branch hints, line ending differences, etc.).
    git(path, &["init", "--initial-branch=main"])?;
    git(path, &["config", "core.autocrlf", "false"])?;
    git(path, &["config", "user.name", "Codex Tests"])?;
    git(path, &["config", "user.email", "codex-tests@example.com"])?;

    // Create README.txt
    let readme_path = path.join("README.txt");
    fs::write(&readme_path, "Test repository initialized by Codex.\n")?;

    // Stage and commit
    git(path, &["add", "README.txt"])?;
    git(path, &["commit", "-m", "Add README.txt"])?;

    Ok(())
}

fn apply_patch_responses(call_id: &str, patch: &str, assistant_msg: &str) -> Vec<String> {
    vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_apply_patch_function_call(call_id, patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", assistant_msg),
            ev_completed("resp-2"),
        ]),
    ]
}

async fn run_apply_patch_turn(
    harness: &TestCodexHarness,
    prompt: &str,
    call_id: &str,
    patch: &str,
    assistant_msg: &str,
) -> Result<()> {
    mount_sse_sequence(
        harness.server(),
        apply_patch_responses(call_id, patch, assistant_msg),
    )
    .await;
    harness.submit(prompt).await
}

async fn invoke_undo(codex: &Arc<CodexThread>) -> Result<UndoCompletedEvent> {
    codex.submit(Op::Undo).await?;
    let event = wait_for_event_match(codex, |msg| match msg {
        EventMsg::UndoCompleted(done) => Some(done.clone()),
        _ => None,
    })
    .await;
    Ok(event)
}

async fn expect_successful_undo(codex: &Arc<CodexThread>) -> Result<UndoCompletedEvent> {
    let event = invoke_undo(codex).await?;
    assert!(
        event.success,
        "expected undo to succeed but failed with message {:?}",
        event.message
    );
    Ok(event)
}

async fn expect_failed_undo(codex: &Arc<CodexThread>) -> Result<UndoCompletedEvent> {
    let event = invoke_undo(codex).await?;
    assert!(
        !event.success,
        "expected undo to fail but succeeded with message {:?}",
        event.message
    );
    assert_eq!(
        event.message.as_deref(),
        Some("No ghost snapshot available to undo.")
    );
    Ok(event)
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_removes_new_file_created_during_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let call_id = "undo-create-file";
    let patch = "*** Begin Patch\n*** Add File: new_file.txt\n+from turn\n*** End Patch";
    run_apply_patch_turn(&harness, "create file", call_id, patch, "ok").await?;

    let new_path = harness.path("new_file.txt");
    assert_eq!(fs::read_to_string(&new_path)?, "from turn\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert!(!new_path.exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_tracked_file_edit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let tracked = harness.path("tracked.txt");
    fs::write(&tracked, "before\n")?;
    git(harness.cwd(), &["add", "tracked.txt"])?;
    git(harness.cwd(), &["commit", "-m", "track file"])?;

    let patch = "*** Begin Patch\n*** Update File: tracked.txt\n@@\n-before\n+after\n*** End Patch";
    run_apply_patch_turn(
        &harness,
        "update tracked file",
        "undo-tracked-edit",
        patch,
        "done",
    )
    .await?;
    println!(
        "apply_patch output: {}",
        harness.function_call_stdout("undo-tracked-edit").await
    );

    assert_eq!(fs::read_to_string(&tracked)?, "after\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&tracked)?, "before\n");
    let status = git_output(harness.cwd(), &["status", "--short"])?;
    assert_eq!(status, "");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_untracked_file_edit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;
    git(harness.cwd(), &["commit", "--allow-empty", "-m", "init"])?;

    let notes = harness.path("notes.txt");
    fs::write(&notes, "original\n")?;
    let status_before = git_output(harness.cwd(), &["status", "--short", "--ignored"])?;
    assert!(status_before.contains("?? notes.txt"));

    let patch =
        "*** Begin Patch\n*** Update File: notes.txt\n@@\n-original\n+modified\n*** End Patch";
    run_apply_patch_turn(
        &harness,
        "edit untracked",
        "undo-untracked-edit",
        patch,
        "done",
    )
    .await?;

    assert_eq!(fs::read_to_string(&notes)?, "modified\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&notes)?, "original\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_reverts_only_latest_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let call_id_one = "undo-turn-one";
    let add_patch = "*** Begin Patch\n*** Add File: story.txt\n+first version\n*** End Patch";
    run_apply_patch_turn(&harness, "create story", call_id_one, add_patch, "done").await?;
    let story = harness.path("story.txt");
    assert_eq!(fs::read_to_string(&story)?, "first version\n");

    let call_id_two = "undo-turn-two";
    let update_patch = "*** Begin Patch\n*** Update File: story.txt\n@@\n-first version\n+second version\n*** End Patch";
    run_apply_patch_turn(&harness, "revise story", call_id_two, update_patch, "done").await?;
    assert_eq!(fs::read_to_string(&story)?, "second version\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&story)?, "first version\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_does_not_touch_unrelated_files() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let tracked_constant = harness.path("stable.txt");
    fs::write(&tracked_constant, "stable\n")?;
    let target = harness.path("target.txt");
    fs::write(&target, "start\n")?;
    let gitignore = harness.path(".gitignore");
    fs::write(&gitignore, "ignored-stable.log\n")?;
    git(
        harness.cwd(),
        &["add", "stable.txt", "target.txt", ".gitignore"],
    )?;
    git(harness.cwd(), &["commit", "-m", "seed tracked"])?;

    let preexisting_untracked = harness.path("scratch.txt");
    fs::write(&preexisting_untracked, "scratch before\n")?;
    let ignored = harness.path("ignored-stable.log");
    fs::write(&ignored, "ignored before\n")?;

    let full_patch = "*** Begin Patch\n*** Update File: target.txt\n@@\n-start\n+edited\n*** Add File: temp.txt\n+ephemeral\n*** End Patch";
    run_apply_patch_turn(
        &harness,
        "modify target",
        "undo-unrelated",
        full_patch,
        "done",
    )
    .await?;
    let temp = harness.path("temp.txt");
    assert_eq!(fs::read_to_string(&target)?, "edited\n");
    assert_eq!(fs::read_to_string(&temp)?, "ephemeral\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&tracked_constant)?, "stable\n");
    assert_eq!(fs::read_to_string(&target)?, "start\n");
    assert_eq!(
        fs::read_to_string(&preexisting_untracked)?,
        "scratch before\n"
    );
    assert_eq!(fs::read_to_string(&ignored)?, "ignored before\n");
    assert!(!temp.exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_sequential_turns_consumes_snapshots() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let story = harness.path("story.txt");
    fs::write(&story, "initial\n")?;
    git(harness.cwd(), &["add", "story.txt"])?;
    git(harness.cwd(), &["commit", "-m", "seed story"])?;

    run_apply_patch_turn(
        &harness,
        "first change",
        "seq-turn-1",
        "*** Begin Patch\n*** Update File: story.txt\n@@\n-initial\n+turn one\n*** End Patch",
        "ok",
    )
    .await?;
    assert_eq!(fs::read_to_string(&story)?, "turn one\n");

    run_apply_patch_turn(
        &harness,
        "second change",
        "seq-turn-2",
        "*** Begin Patch\n*** Update File: story.txt\n@@\n-turn one\n+turn two\n*** End Patch",
        "ok",
    )
    .await?;
    assert_eq!(fs::read_to_string(&story)?, "turn two\n");

    run_apply_patch_turn(
        &harness,
        "third change",
        "seq-turn-3",
        "*** Begin Patch\n*** Update File: story.txt\n@@\n-turn two\n+turn three\n*** End Patch",
        "ok",
    )
    .await?;
    assert_eq!(fs::read_to_string(&story)?, "turn three\n");

    let codex = Arc::clone(&harness.test().codex);
    expect_successful_undo(&codex).await?;
    assert_eq!(fs::read_to_string(&story)?, "turn two\n");

    expect_successful_undo(&codex).await?;
    assert_eq!(fs::read_to_string(&story)?, "turn one\n");

    expect_successful_undo(&codex).await?;
    assert_eq!(fs::read_to_string(&story)?, "initial\n");

    expect_failed_undo(&codex).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_without_snapshot_reports_failure() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    let codex = Arc::clone(&harness.test().codex);

    expect_failed_undo(&codex).await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_moves_and_renames() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let source = harness.path("rename_me.txt");
    fs::write(&source, "original\n")?;
    git(harness.cwd(), &["add", "rename_me.txt"])?;
    git(harness.cwd(), &["commit", "-m", "add rename target"])?;

    let patch = "*** Begin Patch\n*** Update File: rename_me.txt\n*** Move to: relocated/renamed.txt\n@@\n-original\n+renamed content\n*** End Patch";
    run_apply_patch_turn(&harness, "rename file", "undo-rename", patch, "done").await?;

    let destination = harness.path("relocated/renamed.txt");
    assert!(!source.exists());
    assert_eq!(fs::read_to_string(&destination)?, "renamed content\n");

    let codex = Arc::clone(&harness.test().codex);
    expect_successful_undo(&codex).await?;

    assert_eq!(fs::read_to_string(&source)?, "original\n");
    assert!(!destination.exists());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_does_not_touch_ignored_directory_contents() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let gitignore = harness.path(".gitignore");
    fs::write(&gitignore, "logs/\n")?;
    git(harness.cwd(), &["add", ".gitignore"])?;
    git(harness.cwd(), &["commit", "-m", "ignore logs directory"])?;

    let logs_dir = harness.path("logs");
    fs::create_dir_all(&logs_dir)?;
    let preserved = logs_dir.join("persistent.log");
    fs::write(&preserved, "keep me\n")?;

    run_apply_patch_turn(
        &harness,
        "write log",
        "undo-log",
        "*** Begin Patch\n*** Add File: logs/session.log\n+ephemeral log\n*** End Patch",
        "ok",
    )
    .await?;

    let new_log = logs_dir.join("session.log");
    assert_eq!(fs::read_to_string(&new_log)?, "ephemeral log\n");

    let codex = Arc::clone(&harness.test().codex);
    expect_successful_undo(&codex).await?;

    assert!(new_log.exists());
    assert_eq!(fs::read_to_string(&preserved)?, "keep me\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_overwrites_manual_edits_after_turn() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let tracked = harness.path("tracked.txt");
    fs::write(&tracked, "baseline\n")?;
    git(harness.cwd(), &["add", "tracked.txt"])?;
    git(harness.cwd(), &["commit", "-m", "baseline tracked"])?;

    run_apply_patch_turn(
        &harness,
        "modify tracked",
        "undo-manual-overwrite",
        "*** Begin Patch\n*** Update File: tracked.txt\n@@\n-baseline\n+turn change\n*** End Patch",
        "ok",
    )
    .await?;
    assert_eq!(fs::read_to_string(&tracked)?, "turn change\n");

    fs::write(&tracked, "manual edit\n")?;
    assert_eq!(fs::read_to_string(&tracked)?, "manual edit\n");

    let codex = Arc::clone(&harness.test().codex);
    expect_successful_undo(&codex).await?;

    assert_eq!(fs::read_to_string(&tracked)?, "baseline\n");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_preserves_unrelated_staged_changes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    // create a file for user to mess with
    let user_file = harness.path("user_file.txt");
    fs::write(&user_file, "user content v1\n")?;
    git(harness.cwd(), &["add", "user_file.txt"])?;
    git(harness.cwd(), &["commit", "-m", "add user file"])?;

    // AI turn: modifies a DIFFERENT file (creating ghost commit of baseline)
    let ai_file = harness.path("ai_file.txt");
    fs::write(&ai_file, "ai content v1\n")?;
    git(harness.cwd(), &["add", "ai_file.txt"])?;
    git(harness.cwd(), &["commit", "-m", "add ai file"])?; // baseline

    let patch = "*** Begin Patch\n*** Update File: ai_file.txt\n@@\n-ai content v1\n+ai content v2\n*** End Patch";
    run_apply_patch_turn(&harness, "modify ai file", "undo-staging-test", patch, "ok").await?;
    assert_eq!(fs::read_to_string(&ai_file)?, "ai content v2\n");

    // NOW: User modifies user_file AND stages it
    fs::write(&user_file, "user content v2 (staged)\n")?;
    git(harness.cwd(), &["add", "user_file.txt"])?;

    // Verify status before undo
    let status_before = git_output(harness.cwd(), &["status", "--porcelain"])?;
    assert!(status_before.contains("M  user_file.txt")); // M in index

    // UNDO
    let codex = Arc::clone(&harness.test().codex);
    // checks that undo succeeded
    expect_successful_undo(&codex).await?;

    // AI file should be reverted
    assert_eq!(fs::read_to_string(&ai_file)?, "ai content v1\n");

    // User file should STILL be staged with v2
    let status_after = git_output(harness.cwd(), &["status", "--porcelain"])?;

    // We expect 'M' in the first column (index modified).
    // The second column will likely be 'M' because the worktree was reverted to v1 while index has v2.
    // So "MM user_file.txt" is expected.
    if !status_after.contains("MM user_file.txt") && !status_after.contains("M  user_file.txt") {
        bail!("Status should contain staged change (M in first col), but was: '{status_after}'");
    }

    // Disk content is reverted to v1 (snapshot state)
    assert_eq!(fs::read_to_string(&user_file)?, "user content v1\n");

    // But we can get v2 back from index
    git(harness.cwd(), &["checkout", "user_file.txt"])?;
    assert_eq!(
        fs::read_to_string(&user_file)?,
        "user content v2 (staged)\n"
    );

    Ok(())
}
