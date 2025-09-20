#![cfg(unix)]

use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::TurnAbortReason;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_shell_cmd_ls_and_cat_in_temp_dir() {
    unsafe {
        std::env::set_var("CODEX_HERMETIC_TEST_SHELL", "1");
    }
    // No env overrides needed; test build hard-codes a hermetic zsh with empty rc.
    // Create a temporary working directory with a known file.
    let cwd = TempDir::new().unwrap();
    let file_name = "hello.txt";
    let file_path: PathBuf = cwd.path().join(file_name);
    let contents = "hello from bang test\n";
    tokio::fs::write(&file_path, contents)
        .await
        .expect("write temp file");

    // Load config and pin cwd to the temp dir so ls/cat operate there.
    let codex_home = TempDir::new().unwrap();
    let mut config = load_default_config_for_test(&codex_home);
    config.cwd = cwd.path().to_path_buf();

    let conversation_manager =
        ConversationManager::with_auth(codex_core::CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    // 1) ls should list the file
    codex
        .submit(Op::RunUserShellCommand {
            command: "ls".to_string(),
        })
        .await
        .unwrap();
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandEnd(_))).await;
    let EventMsg::ExecCommandEnd(ExecCommandEndEvent {
        stdout, exit_code, ..
    }) = msg
    else {
        unreachable!()
    };
    assert_eq!(exit_code, 0);
    assert!(
        stdout.contains(file_name),
        "ls output should include {file_name}, got: {stdout:?}"
    );

    // 2) cat the file should return exact contents
    codex
        .submit(Op::RunUserShellCommand {
            command: format!("cat {file_name}"),
        })
        .await
        .unwrap();
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandEnd(_))).await;
    let EventMsg::ExecCommandEnd(ExecCommandEndEvent {
        stdout, exit_code, ..
    }) = msg
    else {
        unreachable!()
    };
    assert_eq!(exit_code, 0);
    assert_eq!(stdout, contents);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_shell_cmd_can_be_interrupted() {
    unsafe {
        std::env::set_var("CODEX_HERMETIC_TEST_SHELL", "1");
    }
    // No env overrides needed; test build hard-codes a hermetic zsh with empty rc.
    // Set up isolated config and conversation.
    let codex_home = TempDir::new().unwrap();
    let config = load_default_config_for_test(&codex_home);
    let conversation_manager =
        ConversationManager::with_auth(codex_core::CodexAuth::from_api_key("dummy"));
    let NewConversation {
        conversation: codex,
        ..
    } = conversation_manager
        .new_conversation(config)
        .await
        .expect("create new conversation");

    // Start a long-running command and then interrupt it.
    codex
        .submit(Op::RunUserShellCommand {
            command: "sleep 5".to_string(),
        })
        .await
        .unwrap();

    // Wait until it has started (ExecCommandBegin), then interrupt.
    let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandBegin(_))).await;
    codex.submit(Op::Interrupt).await.unwrap();

    // Expect a TurnAborted(Interrupted) notification.
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
    let EventMsg::TurnAborted(ev) = msg else {
        unreachable!()
    };
    assert_eq!(ev.reason, TurnAbortReason::Interrupted);
}
