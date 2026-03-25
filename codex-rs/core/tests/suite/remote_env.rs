use anyhow::Result;
use codex_exec_server::RemoveOptions;
use codex_utils_absolute_path::AbsolutePathBuf;
use core_test_support::get_remote_test_env;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use core_test_support::test_codex::test_env;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::path::PathBuf;
use std::time::SystemTime;
use std::time::UNIX_EPOCH;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn remote_test_env_can_connect_and_use_filesystem() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let test_env = test_env().await?;
    let file_system = test_env.environment().get_filesystem();

    let file_path = remote_test_file_path();
    let file_path_abs = absolute_path(file_path.clone())?;
    let payload = b"remote-test-env-ok".to_vec();

    file_system
        .write_file(&file_path_abs, payload.clone())
        .await?;
    let actual = file_system.read_file(&file_path_abs).await?;
    assert_eq!(actual, payload);

    file_system
        .remove(
            &file_path_abs,
            RemoveOptions {
                recursive: false,
                force: true,
            },
        )
        .await?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn build_remote_aware_uses_remote_environment_for_shell_commands() -> Result<()> {
    let Some(_remote_env) = get_remote_test_env() else {
        return Ok(());
    };

    let server = start_mock_server().await;
    let call_id = "shell-call";
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-1"),
                ev_function_call(
                    call_id,
                    "shell_command",
                    &serde_json::to_string(&json!({
                        "command": "pwd",
                        "timeout_ms": 2_000,
                    }))?,
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_response_created("resp-2"),
                ev_assistant_message("msg-1", "done"),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;

    let mut builder = test_codex();
    let test = builder.build_remote_aware(&server).await?;
    let expected_cwd = test.config.cwd.display().to_string();

    test.submit_turn("print the current working directory")
        .await?;

    let shell_output = responses
        .function_call_output_text(call_id)
        .map(|output| serde_json::from_str::<Value>(&output))
        .unwrap_or_else(|| panic!("expected {call_id} output"));
    let shell_output = shell_output?;
    let exit_code = shell_output["metadata"]["exit_code"].as_i64();
    let stdout = shell_output["output"]
        .as_str()
        .unwrap_or_default()
        .trim()
        .to_string();

    assert!(
        exit_code.is_none_or(|value| value == 0),
        "expected success output, got exit_code={exit_code:?}, stdout={stdout:?}",
    );
    assert_eq!(stdout, expected_cwd);

    Ok(())
}

fn absolute_path(path: PathBuf) -> Result<AbsolutePathBuf> {
    AbsolutePathBuf::try_from(path.clone())
        .map_err(|err| anyhow::anyhow!("invalid absolute path {}: {err}", path.display()))
}

fn remote_test_file_path() -> PathBuf {
    let nanos = match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_nanos(),
        Err(_) => 0,
    };
    PathBuf::from(format!(
        "/tmp/codex-remote-test-env-{}-{nanos}.txt",
        std::process::id()
    ))
}
