#![cfg(not(target_os = "windows"))]

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_core::CodexConversation;
use codex_core::config::Config;
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
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;

async fn undo_harness() -> Result<TestCodexHarness> {
    TestCodexHarness::with_config(|config: &mut Config| {
        config.include_apply_patch_tool = true;
        config.features.enable(Feature::GhostCommit);
    })
    .await
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
    git(path, &["init"])?;
    git(path, &["config", "user.name", "Codex Tests"])?;
    git(path, &["config", "user.email", "codex-tests@example.com"])?;
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

async fn expect_successful_undo(codex: &Arc<CodexConversation>) -> Result<UndoCompletedEvent> {
    codex.submit(Op::Undo).await?;
    let event = wait_for_event_match(codex, |msg| match msg {
        EventMsg::UndoCompleted(done) => Some(done.clone()),
        _ => None,
    })
    .await;
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
    let status_after = git_output(harness.cwd(), &["status", "--short", "--ignored"])?;
    assert!(status_after.contains("?? notes.txt"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_ignored_file_edit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let gitignore = harness.path(".gitignore");
    fs::write(&gitignore, "ignored.log\n")?;
    git(harness.cwd(), &["add", ".gitignore"])?;
    git(harness.cwd(), &["commit", "-m", "add gitignore"])?;

    let ignored = harness.path("ignored.log");
    fs::write(&ignored, "initial\n")?;
    let status_before = git_output(harness.cwd(), &["status", "--short", "--ignored=matching"])?;
    assert!(status_before.contains("!! ignored.log"));

    let patch =
        "*** Begin Patch\n*** Update File: ignored.log\n@@\n-initial\n+turn edit\n*** End Patch";
    run_apply_patch_turn(&harness, "edit ignored", "undo-ignored-edit", patch, "done").await?;

    assert_eq!(fs::read_to_string(&ignored)?, "turn edit\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&ignored)?, "initial\n");
    let status_after = git_output(harness.cwd(), &["status", "--short", "--ignored=matching"])?;
    assert!(status_after.contains("!! ignored.log"));

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
async fn undo_preserves_manual_edits_between_turns() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = undo_harness().await?;
    init_git_repo(harness.cwd())?;

    let call_id_one = "undo-manual-turn-one";
    let add_patch = "*** Begin Patch\n*** Add File: notes.md\n+original content\n*** End Patch";
    run_apply_patch_turn(&harness, "create notes", call_id_one, add_patch, "done").await?;

    let notes = harness.path("notes.md");
    assert_eq!(fs::read_to_string(&notes)?, "original content\n");
    fs::write(&notes, "manual changes\n")?;
    assert_eq!(fs::read_to_string(&notes)?, "manual changes\n");

    let call_id_two = "undo-manual-turn-two";
    let update_patch = "*** Begin Patch\n*** Update File: notes.md\n@@\n-manual changes\n+turn rewrite\n*** End Patch";
    run_apply_patch_turn(&harness, "update notes", call_id_two, update_patch, "done").await?;
    assert_eq!(fs::read_to_string(&notes)?, "turn rewrite\n");

    let codex = Arc::clone(&harness.test().codex);
    let completed = expect_successful_undo(&codex).await?;
    assert!(completed.success, "undo failed: {:?}", completed.message);

    assert_eq!(fs::read_to_string(&notes)?, "manual changes\n");

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
