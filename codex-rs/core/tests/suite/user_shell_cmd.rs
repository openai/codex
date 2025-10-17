use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::Op;
use codex_core::protocol::TurnAbortReason;
use core_test_support::load_default_config_for_test;
use core_test_support::wait_for_event;
use std::path::PathBuf;
use std::process::Command;
use std::process::Stdio;
use tempfile::TempDir;

fn detect_python_executable() -> Option<String> {
    let candidates = ["python3", "python"];
    candidates.iter().find_map(|candidate| {
        Command::new(candidate)
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .ok()
            .and_then(|status| status.success().then(|| (*candidate).to_string()))
    })
}

#[tokio::test]
async fn user_shell_cmd_ls_and_cat_in_temp_dir() {
    let Some(python) = detect_python_executable() else {
        eprintln!("skipping test: python3 not found in PATH");
        return;
    };

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

    // 1) python should list the file
    let list_cmd = format!(
        "{python} -c \"import pathlib; print('\\n'.join(sorted(p.name for p in pathlib.Path('.').iterdir())))\""
    );
    codex
        .submit(Op::RunUserShellCommand { command: list_cmd })
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

    // 2) python should print the file contents verbatim
    let cat_cmd = format!(
        "{python} -c \"import pathlib; print(pathlib.Path('{file_name}').read_text(), end='')\""
    );
    codex
        .submit(Op::RunUserShellCommand { command: cat_cmd })
        .await
        .unwrap();
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::ExecCommandEnd(_))).await;
    let EventMsg::ExecCommandEnd(ExecCommandEndEvent {
        mut stdout,
        exit_code,
        ..
    }) = msg
    else {
        unreachable!()
    };
    assert_eq!(exit_code, 0);
    if cfg!(windows) {
        // Windows' Python writes CRLF line endings; normalize so the assertion remains portable.
        stdout = stdout.replace("\r\n", "\n");
    }
    assert_eq!(stdout, contents);
}

#[tokio::test]
async fn user_shell_cmd_can_be_interrupted() {
    let Some(python) = detect_python_executable() else {
        eprintln!("skipping test: python3 not found in PATH");
        return;
    };
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
    let sleep_cmd = format!("{python} -c \"import time; time.sleep(5)\"");
    codex
        .submit(Op::RunUserShellCommand { command: sleep_cmd })
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
