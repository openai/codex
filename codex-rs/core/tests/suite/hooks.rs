use std::fs;
use std::path::Path;

use codex_core::protocol::AskForApproval;
use codex_core::protocol::EventMsg;
use codex_core::protocol::Op;
use codex_core::protocol::SandboxPolicy;
use codex_protocol::config_types::ReasoningSummary;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

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
    fs::create_dir_all(&hook_dir).expect("create .codex dir");
    fs::write(hook_dir.join("hook.toml"), contents).expect("write hook.toml");
}

fn turn_start_hook_config() -> String {
    if cfg!(windows) {
        format_hook_section(
            "turn_start",
            "cmd.exe",
            &["/C", "echo hook-start 1>&2"],
        )
    } else {
        format_hook_section("turn_start", "sh", &["-c", "echo hook-start 1>&2"])
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

    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "user input".to_string(),
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

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let request = response_mock.single_request();
    let messages = request.message_input_texts("user");
    assert!(messages
        .iter()
        .any(|text| text.contains("HookInput (TurnStart): hook-start")));
    assert!(messages.iter().any(|text| text.contains("user input")));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn turn_end_hook_stderr_triggers_follow_up_turn() -> anyhow::Result<()> {
    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5.1-codex");
    let test = builder.build(&server).await?;

    write_hook_toml(test.cwd_path(), &turn_end_hook_config());

    let first = sse(vec![
        ev_response_created("resp-1"),
        ev_assistant_message("msg-1", "ok"),
        ev_completed("resp-1"),
    ]);
    let second = sse(vec![
        ev_response_created("resp-2"),
        ev_assistant_message("msg-2", "follow up"),
        ev_completed("resp-2"),
    ]);
    let response_mock = mount_sse_sequence(&server, vec![first, second]).await;

    let session_model = test.session_configured.model.clone();
    test.codex
        .submit(Op::UserTurn {
            items: vec![UserInput::Text {
                text: "user input".to_string(),
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

    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;
    wait_for_event(&test.codex, |ev| matches!(ev, EventMsg::TurnComplete(_))).await;

    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);

    let first_messages = requests[0].message_input_texts("user");
    assert!(
        !first_messages
            .iter()
            .any(|text| text.contains("HookInput (TurnEnd): hook-end"))
    );

    let second_messages = requests[1].message_input_texts("user");
    assert!(second_messages
        .iter()
        .any(|text| text.contains("HookInput (TurnEnd): hook-end")));

    Ok(())
}
