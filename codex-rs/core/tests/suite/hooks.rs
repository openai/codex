use std::fs;
use std::path::Path;
use std::time::Duration;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::HookKind;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::TestCodex;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;
use tokio::time::sleep;

fn format_hook_section(section: &str, command: &str, args: &[&str]) -> String {
    let args = args
        .iter()
        .map(|arg| format!("{arg:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{section}]\ncommand = {command:?}\nargs = [{args}]\n")
}

fn write_hook_toml(cwd: &Path, contents: &str) {
    let hook_dir = cwd.join(".codex");
    fs::create_dir_all(&hook_dir).unwrap_or_else(|err| panic!("create .codex dir: {err}"));
    fs::write(hook_dir.join("hook.toml"), contents)
        .unwrap_or_else(|err| panic!("write hook.toml: {err}"));
}

fn turn_start_hook_config() -> String {
    if cfg!(windows) {
        format_hook_section("turn_start", "cmd.exe", &["/C", "echo hook-start 1>&2"])
    } else {
        format_hook_section("turn_start", "sh", &["-c", "echo hook-start 1>&2"])
    }
}

fn turn_start_hook_stdout_config() -> String {
    if cfg!(windows) {
        format_hook_section("turn_start", "cmd.exe", &["/C", "echo hook-start"])
    } else {
        format_hook_section("turn_start", "sh", &["-c", "echo hook-start"])
    }
}

fn turn_start_hook_whitespace_config() -> String {
    if cfg!(windows) {
        format_hook_section("turn_start", "cmd.exe", &["/C", "echo. 1>&2"])
    } else {
        format_hook_section("turn_start", "sh", &["-c", "printf '   ' 1>&2"])
    }
}

fn turn_start_hook_failure_config() -> String {
    if cfg!(windows) {
        format_hook_section(
            "turn_start",
            "cmd.exe",
            &["/C", "echo hook-fail 1>&2 & exit /b 42"],
        )
    } else {
        format_hook_section("turn_start", "sh", &["-c", "echo hook-fail 1>&2; exit 42"])
    }
}

fn turn_start_hook_multiline_config() -> String {
    if cfg!(windows) {
        format_hook_section(
            "turn_start",
            "cmd.exe",
            &["/C", "echo line1 1>&2 & echo line2 1>&2"],
        )
    } else {
        format_hook_section("turn_start", "sh", &["-c", "printf 'line1\nline2\n' 1>&2"])
    }
}

fn turn_end_hook_config() -> String {
    if cfg!(windows) {
        format_hook_section(
            "turn_end",
            "cmd.exe",
            &[
                "/C",
                "if not exist .codex/hook-end.fired (echo hook-end 1>&2 & type nul > .codex/hook-end.fired)",
            ],
        )
    } else {
        format_hook_section(
            "turn_end",
            "sh",
            &[
                "-c",
                "if [ ! -f .codex/hook-end.fired ]; then echo hook-end 1>&2; touch .codex/hook-end.fired; fi",
            ],
        )
    }
}

fn turn_end_hook_stdout_config() -> String {
    if cfg!(windows) {
        format_hook_section("turn_end", "cmd.exe", &["/C", "echo hook-end"])
    } else {
        format_hook_section("turn_end", "sh", &["-c", "echo hook-end"])
    }
}

fn turn_end_hook_whitespace_config() -> String {
    if cfg!(windows) {
        format_hook_section("turn_end", "cmd.exe", &["/C", "echo. 1>&2"])
    } else {
        format_hook_section("turn_end", "sh", &["-c", "printf '   ' 1>&2"])
    }
}

fn invalid_hook_config() -> String {
    "turn_start =".to_string()
}

async fn submit_user_turn(test: &TestCodex, prompt: &str) -> anyhow::Result<()> {
    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: prompt.to_string(),
                text_elements: Vec::new(),
            }],
            final_output_json_schema: None,
            cwd: test.cwd_path().to_path_buf(),
            approval_policy: AskForApproval::Never,
            sandbox_policy: SandboxPolicy::DangerFullAccess,
            model: session_model,
            effort: None,
            summary: ReasoningSummary::Auto,
            collaboration_mode: None,
            personality: None,
        })
        .await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_start_hook_stderr_is_sent_with_user_input() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_start_hook_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(
        messages
            .iter()
            .any(|text| text.contains("HookInput (TurnStart): hook-start"))
    );
    assert!(messages.iter().any(|text| text.contains("user input")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_start_hook_stdout_is_ignored() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_start_hook_stdout_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnStart)"))
    );
    assert!(messages.iter().any(|text| text.contains("user input")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_start_hook_whitespace_is_ignored() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_start_hook_whitespace_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnStart)"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_start_hook_emits_event_with_exit_code_and_command() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_start_hook_failure_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let _response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;

    let hook_event = wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::HookInput(_))).await;
    let hook_event = match hook_event {
        EventMsg::HookInput(hook_event) => hook_event,
        _ => unreachable!("HookInput event expected"),
    };
    assert_eq!(hook_event.hook, HookKind::TurnStart);
    assert_eq!(hook_event.exit_code, 42);
    assert!(hook_event.stderr.contains("hook-fail"));
    if cfg!(windows) {
        assert_eq!(hook_event.command[0], "cmd.exe");
        assert_eq!(
            hook_event.command[1..],
            ["/C", "echo hook-fail 1>&2 & exit /b 42"]
        );
    } else {
        assert_eq!(hook_event.command[0], "sh");
        assert_eq!(
            hook_event.command[1..],
            ["-c", "echo hook-fail 1>&2; exit 42"]
        );
    }

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_start_hook_multiline_stderr_is_trimmed() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_start_hook_multiline_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let _response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;

    let hook_event = wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::HookInput(_))).await;
    let hook_event = match hook_event {
        EventMsg::HookInput(hook_event) => hook_event,
        _ => unreachable!("HookInput event expected"),
    };
    assert_eq!(hook_event.hook, HookKind::TurnStart);
    let normalized = hook_event.stderr.replace("\r\n", "\n");
    let normalized = normalized
        .lines()
        .map(str::trim_end)
        .collect::<Vec<_>>()
        .join("\n");
    assert_eq!(normalized, "line1\nline2");
    assert_eq!(hook_event.exit_code, 0);

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_end_hook_stderr_triggers_follow_up_turn() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_end_hook_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;

    let hook_event = wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::HookInput(_))).await;
    let hook_event = match hook_event {
        EventMsg::HookInput(hook_event) => hook_event,
        _ => unreachable!("HookInput event expected"),
    };
    assert_eq!(hook_event.hook, HookKind::TurnEnd);
    assert!(hook_event.stderr.contains("hook-end"));

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);
    let first_messages = requests[0].message_input_texts("user");
    assert!(
        !first_messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd): hook-end"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_end_hook_stdout_does_not_trigger_follow_up_turn() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_end_hook_stdout_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    sleep(Duration::from_millis(200)).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);
    let messages = requests[0].message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd)"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_end_hook_whitespace_does_not_trigger_follow_up_turn() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_end_hook_whitespace_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    sleep(Duration::from_millis(200)).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 1);
    let messages = requests[0].message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd)"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn missing_hook_config_is_noop() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnStart)"))
    );
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd)"))
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn invalid_hook_config_is_ignored() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &invalid_hook_config());

    let response_body = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let response_mock = mount_sse_once(&server, response_body).await;

    submit_user_turn(&test, "user input").await?;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnStart)"))
    );
    assert!(
        !messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd)"))
    );

    Ok(())
}
