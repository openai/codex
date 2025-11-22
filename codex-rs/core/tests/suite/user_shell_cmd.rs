use anyhow::Context;
use codex_core::ConversationManager;
use codex_core::NewConversation;
use codex_core::protocol::EventMsg;
use codex_core::protocol::ExecCommandEndEvent;
use codex_core::protocol::ExecCommandSource;
use codex_core::protocol::ExecOutputStream;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_core::protocol::TurnAbortReason;
use core_test_support::assert_regex_match;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use regex_lite::escape;
use std::path::PathBuf;
use tempfile::TempDir;

#[tokio::test]
async fn user_shell_cmd_ls_and_cat_in_temp_dir() {
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

    // 1) shell command should list the file
    let list_cmd = "ls".to_string();
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

    // 2) shell command should print the file contents verbatim
    let cat_cmd = format!("cat {file_name}");
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
        // Windows shells emit CRLF line endings; normalize so the assertion remains portable.
        stdout = stdout.replace("\r\n", "\n");
    }
    assert_eq!(stdout, contents);
}

#[tokio::test]
async fn user_shell_cmd_can_be_interrupted() {
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
    let sleep_cmd = "sleep 5".to_string();
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_shell_command_history_is_persisted_and_shared_with_model() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let mut builder = core_test_support::test_codex::test_codex();
    let test = builder.build(&server).await?;

    #[cfg(windows)]
    let command = r#"$val = $env:CODEX_SANDBOX; if ([string]::IsNullOrEmpty($val)) { $val = 'not-set' } ; [System.Console]::Write($val)"#.to_string();
    #[cfg(not(windows))]
    let command = r#"sh -c "printf '%s' \"${CODEX_SANDBOX:-not-set}\"""#.to_string();

    test.codex
        .submit(Op::RunUserShellCommand {
            command: command.clone(),
        })
        .await?;

    let begin_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::ExecCommandBegin(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(begin_event.source, ExecCommandSource::UserShell);
    let matches_last_arg = begin_event.command.last() == Some(&command);
    let matches_split = shlex::split(&command).is_some_and(|split| split == begin_event.command);
    assert!(
        matches_last_arg || matches_split,
        "user command begin event should include the original command; got: {:?}",
        begin_event.command
    );

    let delta_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::ExecCommandOutputDelta(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(delta_event.stream, ExecOutputStream::Stdout);
    let chunk_text =
        String::from_utf8(delta_event.chunk.clone()).expect("user command chunk is valid utf-8");
    assert_eq!(chunk_text.trim(), "not-set");

    let end_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::ExecCommandEnd(event) => Some(event.clone()),
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

    let command_message = request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains("<user_shell_command>"))
        .expect("command message recorded in request");
    let command_message = command_message.replace("\r\n", "\n");
    let escaped_command = escape(&command);
    let expected_pattern = format!(
        r"(?m)\A<user_shell_command>\n<command>\n{escaped_command}\n</command>\n<result>\nExit code: 0\nDuration: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\nnot-set\n</result>\n</user_shell_command>\z"
    );
    assert_regex_match(&expected_pattern, &command_message);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[cfg(not(target_os = "windows"))] // TODO: unignore on windows
async fn user_shell_command_output_is_truncated_in_history() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let builder = core_test_support::test_codex::test_codex();
    let test = builder
        .with_config(|config| {
            config.tool_output_token_limit = Some(100);
        })
        .build(&server)
        .await?;

    #[cfg(windows)]
    let command = r#"for ($i=1; $i -le 400; $i++) { Write-Output $i }"#.to_string();
    #[cfg(not(windows))]
    let command = "seq 1 400".to_string();

    test.codex
        .submit(Op::RunUserShellCommand {
            command: command.clone(),
        })
        .await?;

    let end_event = wait_for_event_match(&test.codex, |ev| match ev {
        EventMsg::ExecCommandEnd(event) => Some(event.clone()),
        _ => None,
    })
    .await;
    assert_eq!(end_event.exit_code, 0);

    let _ = wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TaskComplete(_))).await;

    let responses = vec![responses::sse(vec![
        responses::ev_response_created("resp-1"),
        responses::ev_assistant_message("msg-1", "done"),
        responses::ev_completed("resp-1"),
    ])];
    let mock = responses::mount_sse_sequence(&server, responses).await;

    test.submit_turn("follow-up after shell command").await?;

    let request = mock.single_request();
    let command_message = request
        .message_input_texts("user")
        .into_iter()
        .find(|text| text.contains("<user_shell_command>"))
        .expect("command message recorded in request");
    let command_message = command_message.replace("\r\n", "\n");

    let head = (1..=69).map(|i| format!("{i}\n")).collect::<String>();
    let tail = (352..=400).map(|i| format!("{i}\n")).collect::<String>();
    let truncated_body =
        format!("Total output lines: 400\n\n{head}70…273 tokens truncated…351\n{tail}");
    let escaped_command = escape(&command);
    let escaped_truncated_body = escape(&truncated_body);
    let expected_pattern = format!(
        r"(?m)\A<user_shell_command>\n<command>\n{escaped_command}\n</command>\n<result>\nExit code: 0\nDuration: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\n{escaped_truncated_body}\n</result>\n</user_shell_command>\z"
    );
    assert_regex_match(&expected_pattern, &command_message);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn user_shell_command_is_truncated_only_once() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;

    let mut builder = test_codex()
        .with_model("gpt-5.1-codex")
        .with_config(|config| {
            config.tool_output_token_limit = Some(100);
        });
    let fixture = builder.build(&server).await?;

    let call_id = "user-shell-double-truncation";
    let args = if cfg!(windows) {
        serde_json::json!({
            "command": "for ($i=1; $i -le 2000; $i++) { Write-Output $i }",
            "timeout_ms": 5_000,
        })
    } else {
        serde_json::json!({
            "command": "seq 1 2000",
            "timeout_ms": 5_000,
        })
    };

    mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let mock2 = mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    fixture
        .submit_turn_with_policy(
            "trigger big shell_command output",
            SandboxPolicy::DangerFullAccess,
        )
        .await?;

    let output = mock2
        .single_request()
        .function_call_output_text(call_id)
        .context("function_call_output present for shell_command call")?;

    let truncation_headers = output.matches("Total output lines:").count();

    assert_eq!(
        truncation_headers, 1,
        "shell_command output should carry only one truncation header: {output}"
    );

    Ok(())
}
