use std::path::Path;
use std::process::Command;

use codex_core::protocol::EventMsg;
use codex_core::protocol::InputItem;
use codex_core::protocol::Op;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

#[allow(clippy::expect_used)]
fn run_git_in(repo_path: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(repo_path)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git command failed: {args:?}");
}

fn init_test_repo(repo: &Path) {
    run_git_in(repo, &["init", "--initial-branch=main"]);
    run_git_in(repo, &["config", "core.autocrlf", "false"]);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_no_snapshot_emits_background_message() {
    // No snapshot has been created in this session. Undo should report that clearly.
    let server = responses::start_mock_server().await;

    let mut builder = test_codex();
    let codex = builder
        .build(&server)
        .await
        .expect("start codex conversation");

    let id = codex
        .codex
        .submit(Op::UndoLastSnapshot)
        .await
        .expect("submit undo");

    // Expect a background event saying there is no snapshot to undo.
    let ev = wait_for_event(&codex.codex, |msg| match msg {
        EventMsg::BackgroundEvent(ev) => ev.message.contains("No snapshot available to undo."),
        _ => false,
    })
    .await;
    match ev {
        EventMsg::BackgroundEvent(ev) => {
            assert!(ev.message.contains("No snapshot available to undo."));
        }
        _ => unreachable!(),
    }

    // Avoid unused id warnings in case diagnostics change.
    assert!(!id.is_empty());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_workspace_root() {
    skip_if_no_network!();

    // Create a git repository with an initial commit.
    let repo = TempDir::new().expect("tempdir");
    let repo_path = repo.path();
    init_test_repo(repo_path);
    std::fs::write(repo_path.join("tracked.txt"), "v1\n").unwrap();
    run_git_in(repo_path, &["add", "."]);
    run_git_in(
        repo_path,
        &[
            "-c",
            "user.name=Tester",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );

    // Start a Codex session rooted at the repo and run a minimal turn to trigger snapshot.
    let server = responses::start_mock_server().await;
    let sse = responses::sse(vec![responses::ev_completed("r1")]);
    responses::mount_sse_once(&server, sse).await;

    let repo_root = repo_path.to_path_buf();
    let mut builder = test_codex().with_config(move |c| {
        c.cwd = repo_root;
    });
    let codex = builder
        .build(&server)
        .await
        .expect("start codex conversation");

    codex
        .codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".to_string(),
            }],
        })
        .await
        .expect("submit input");

    // Wait for the request to reach the mock server to rule out network issues.
    {
        use tokio::time::Duration;
        use tokio::time::sleep;
        let mut tries = 0u32;
        loop {
            let reqs = server.received_requests().await.unwrap();
            if !reqs.is_empty() || tries > 50 {
                break;
            }
            tries += 1;
            sleep(Duration::from_millis(100)).await;
        }
        let reqs = server.received_requests().await.unwrap();
        assert!(
            !reqs.is_empty(),
            "model request was not observed by mock server"
        );
    }

    // Wait until the turn completes.
    let _ = wait_for_event(&codex.codex, |msg| matches!(msg, EventMsg::TaskComplete(_))).await;

    // Change tracked file after the snapshot.
    std::fs::write(repo_path.join("tracked.txt"), "v2\n").unwrap();

    // Request undo and await confirmation.
    let _undo_id = codex
        .codex
        .submit(Op::UndoLastSnapshot)
        .await
        .expect("submit undo");
    let _ = wait_for_event(&codex.codex, |msg| match msg {
        EventMsg::BackgroundEvent(ev) => ev.message.starts_with("Restored workspace to snapshot"),
        _ => false,
    })
    .await;

    // File content should be restored to v1.
    let after = std::fs::read_to_string(repo_path.join("tracked.txt")).unwrap();
    assert_eq!(after, "v1\n");
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn undo_restores_only_within_subdirectory() {
    skip_if_no_network!();

    // Create a git repository with a tracked file in a subdirectory.
    let repo = TempDir::new().expect("tempdir");
    let repo_path = repo.path();
    init_test_repo(repo_path);
    let workspace = repo_path.join("workspace");
    std::fs::create_dir_all(&workspace).unwrap();
    std::fs::write(repo_path.join("root.txt"), "root v1\n").unwrap();
    std::fs::write(workspace.join("nested.txt"), "nested v1\n").unwrap();
    run_git_in(repo_path, &["add", "."]);
    run_git_in(
        repo_path,
        &[
            "-c",
            "user.name=Tester",
            "-c",
            "user.email=test@example.com",
            "commit",
            "-m",
            "initial",
        ],
    );

    // Start a Codex session rooted at the subdirectory and run a turn to trigger a subdir snapshot.
    let server = responses::start_mock_server().await;
    let sse = responses::sse(vec![responses::ev_completed("r1")]);
    responses::mount_sse_once(&server, sse).await;

    let workspace_dir = workspace.clone();
    let mut builder = test_codex().with_config(move |c| {
        c.cwd = workspace_dir;
    });
    let codex = builder
        .build(&server)
        .await
        .expect("start codex conversation");

    codex
        .codex
        .submit(Op::UserInput {
            items: vec![InputItem::Text {
                text: "hello".to_string(),
            }],
        })
        .await
        .expect("submit input");

    // Wait for the request to reach the mock server to rule out network issues.
    {
        use tokio::time::Duration;
        use tokio::time::sleep;
        let mut tries = 0u32;
        loop {
            let reqs = server.received_requests().await.unwrap();
            if !reqs.is_empty() || tries > 50 {
                break;
            }
            tries += 1;
            sleep(Duration::from_millis(100)).await;
        }
        let reqs = server.received_requests().await.unwrap();
        assert!(
            !reqs.is_empty(),
            "model request was not observed by mock server"
        );
    }

    let _ = wait_for_event(&codex.codex, |msg| matches!(msg, EventMsg::TaskComplete(_))).await;

    // Modify files both inside and outside the subdirectory.
    std::fs::write(repo_path.join("root.txt"), "root v2\n").unwrap();
    std::fs::write(workspace.join("nested.txt"), "nested v2\n").unwrap();

    // Undo should restore the snapshot only within the subdirectory (workspace).
    let _ = codex
        .codex
        .submit(Op::UndoLastSnapshot)
        .await
        .expect("submit undo");
    let _ = wait_for_event(&codex.codex, |msg| match msg {
        EventMsg::BackgroundEvent(ev) => ev.message.starts_with("Restored workspace to snapshot"),
        _ => false,
    })
    .await;

    // Verify: nested.txt restored to v1; root.txt remains at v2.
    let nested_after = std::fs::read_to_string(workspace.join("nested.txt")).unwrap();
    assert_eq!(nested_after, "nested v1\n");
    let root_after = std::fs::read_to_string(repo_path.join("root.txt")).unwrap();
    assert_eq!(root_after, "root v2\n");
}
