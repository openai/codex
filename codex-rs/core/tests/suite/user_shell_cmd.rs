use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecOutputStream;
use codex_core::protocol::Op;
use codex_core::protocol::TurnAbortReason;
use codex_core::protocol::UserCommandEndEvent;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
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
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::UserCommandEnd(_))).await;
    let EventMsg::UserCommandEnd(UserCommandEndEvent {
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
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::UserCommandEnd(_))).await;
    let EventMsg::UserCommandEnd(UserCommandEndEvent {
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
    let _ = wait_for_event(&codex, |ev| matches!(ev, EventMsg::UserCommandBegin(_))).await;
    codex.submit(Op::Interrupt).await.unwrap();

    // Expect a TurnAborted(Interrupted) notification.
    let msg = wait_for_event(&codex, |ev| matches!(ev, EventMsg::TurnAborted(_))).await;
    let EventMsg::TurnAborted(ev) = msg else {
        unreachable!()
    };
    assert_eq!(ev.reason, TurnAbortReason::Interrupted);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_shell_command_history_is_persisted_and_shared_with_model() -> anyhow::Result<()> {
    let command = if cfg!(target_os = "windows") {
        "$val = $env:CODEX_SANDBOX; if ($null -eq $val -or $val -eq '') { Write-Output 'not-set' } else { Write-Output $val }".to_string()
    } else {
        "if [ -n \"$CODEX_SANDBOX\" ]; then printf %s \"$CODEX_SANDBOX\"; else printf %s not-set; fi"
            .to_string()
    };

    let server = responses::start_mock_server().await;
    let mut builder = core_test_support::test_codex::test_codex();
    let test = builder.build(&server).await?;

    test.codex
        .submit(Op::RunUserShellCommand {
            command: command.clone(),
        })
        .await?;

    let begin_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::UserCommandBegin(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(begin_event.command.last(), Some(&command));

    let delta_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::UserCommandOutputDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(delta_event.stream, ExecOutputStream::Stdout);
    let chunk_text =
        String::from_utf8(delta_event.chunk.clone()).expect("user command chunk is valid utf-8");
    assert_eq!(chunk_text.trim(), "not-set");

    let end_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::UserCommandEnd(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(end_event.exit_code, 0);
    assert_eq!(end_event.stdout.trim(), "not-set");

    let _ = wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let responses = vec![responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "done"),
        responses::ev_completed("resp-1"),
    ])];
    let mock = responses::mount_sse_sequence(&server, responses).await;

    test.submit_turn("follow-up after shell command").await?;

    let request = mock.single_request();
    let items = request.input();

    fn find_user_text(items: &[serde_json::Value], marker: &str) -> Option<String> {
        items.iter().find_map(|item| {
            if item.get("type").and_then(serde_json::Value::as_str) != Some("message") {
                return None;
            }
            if item.get("role").and_then(serde_json::Value::as_str) != Some("user") {
                return None;
            }
            let content = item.get("content")?.as_array()?;
            content.iter().find_map(|span| {
                if span.get("type").and_then(serde_json::Value::as_str) == Some("input_text") {
                    let text = span.get("text").and_then(serde_json::Value::as_str)?;
                    if text.contains(marker) {
                        return Some(text.to_string());
                    }
                }
                None
            })
        })
    }

    let command_message = find_user_text(&items, "<user_shell_command>")
        .expect("command message recorded in request");
    assert!(
        command_message.contains(&command),
        "command message should include shell invocation: {command_message}"
    );

    let output_message = find_user_text(&items, "<user_shell_command_output>")
        .expect("output message recorded in request");
    let payload = output_message
        .strip_prefix("<user_shell_command_output>\n")
        .and_then(|text| text.strip_suffix("\n</user_shell_command_output>"))
        .expect("shell command output payload present");
    let parsed: serde_json::Value =
        serde_json::from_str(payload).expect("parse shell command output payload");
    assert_eq!(
        parsed
            .get("metadata")
            .and_then(|meta| meta.get("exit_code"))
            .and_then(serde_json::Value::as_i64),
        Some(0),
        "expected exit_code metadata to be present and zero",
    );
    let output_text = parsed
        .get("output")
        .and_then(serde_json::Value::as_str)
        .expect("model-facing output string present");
    assert!(
        output_text.contains("not-set"),
        "model-facing output should include stdout content: {output_text:?}"
    );

    Ok(())
}
