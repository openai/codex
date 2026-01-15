use std::time::Duration;

use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_windows;
use core_test_support::test_codex::TestCodexBuilder;
use core_test_support::test_codex::TestCodexHarness;
use core_test_support::test_codex::test_codex;
use serde_json::json;
use test_case::test_case;

/// Use this timeout if, empirically, a test seems to need more time than the
/// default.
const MEDIUM_TIMEOUT: Duration = Duration::from_secs(5);

fn shell_responses_with_timeout(
    call_id: &str,
    command: &str,
    login: Option<bool>,
    timeout_ms: i64,
) -> Vec<String> {
    let args = json!({
        "command": command,
        "timeout_ms": timeout_ms,
        "login": login,
    });

    #[allow(clippy::expect_used)]
    let arguments = serde_json::to_string(&args).expect("serialize shell command arguments");

    vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell_command", &arguments),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ]
}

fn shell_responses(call_id: &str, command: &str, login: Option<bool>) -> Vec<String> {
    shell_responses_with_timeout(call_id, command, login, 2_000)
}

async fn shell_command_harness_with(
    configure: impl FnOnce(TestCodexBuilder) -> TestCodexBuilder,
) -> Result<TestCodexHarness> {
    let builder = configure(test_codex()).with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    TestCodexHarness::with_builder(builder).await
}

async fn mount_shell_responses(
    harness: &TestCodexHarness,
    call_id: &str,
    command: &str,
    login: Option<bool>,
) {
    mount_sse_sequence(harness.server(), shell_responses(call_id, command, login)).await;
}

async fn mount_shell_responses_with_timeout(
    harness: &TestCodexHarness,
    call_id: &str,
    command: &str,
    login: Option<bool>,
    timeout: Duration,
) {
    mount_sse_sequence(
        harness.server(),
        shell_responses_with_timeout(call_id, command, login, timeout.as_millis() as i64),
    )
    .await;
}

fn assert_shell_command_output(output: &str, expected: &str) -> Result<()> {
    let normalized_output = output
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string();

    let expected_pattern = format!(
        r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\n{expected}\n?$"
    );

    assert_regex_match(&expected_pattern, &normalized_output);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_works() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call";
    mount_shell_responses(&harness, call_id, "echo 'hello, world'", None).await;
    harness.submit("run the echo command").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "hello, world")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_with_login() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call-login-true";
    mount_shell_responses(&harness, call_id, "echo 'hello, world'", Some(true)).await;
    harness.submit("run the echo command with login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "hello, world")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_without_login() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call-login-false";
    mount_shell_responses(&harness, call_id, "echo 'hello, world'", Some(false)).await;
    harness.submit("run the echo command without login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "hello, world")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn multi_line_output_with_login() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call-first-extra-login";
    mount_shell_responses(
        &harness,
        call_id,
        "echo 'first line\nsecond line'",
        Some(true),
    )
    .await;
    harness.submit("run the command with login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "first line\nsecond line")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_output_with_login() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call-second-extra-no-login";
    mount_shell_responses(&harness, call_id, "echo 'hello, world' | cat", None).await;
    harness.submit("run the command without login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "hello, world")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn pipe_output_without_login() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));
    skip_if_windows!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;

    let call_id = "shell-command-call-third-extra-login-false";
    mount_shell_responses(&harness, call_id, "echo 'hello, world' | cat", Some(false)).await;
    harness.submit("run the command without login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "hello, world")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_times_out_with_timeout_ms() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| builder.with_model("gpt-5.1")).await?;
    let call_id = "shell-command-timeout";
    let command = if cfg!(windows) {
        "timeout /t 5"
    } else {
        "sleep 5"
    };
    mount_shell_responses_with_timeout(
        &harness,
        call_id,
        command,
        None,
        Duration::from_millis(200),
    )
    .await;
    harness
        .submit("run a long command with a short timeout")
        .await?;

    let output = harness.function_call_stdout(call_id).await;
    let normalized_output = output
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .trim_end_matches('\n')
        .to_string();
    let expected_pattern = r"(?s)^Exit code: 124\nWall time: [0-9]+(?:\.[0-9]+)? seconds\nOutput:\ncommand timed out after [0-9]+ milliseconds\n?$";
    assert_regex_match(expected_pattern, &normalized_output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(true ; "with_login")]
#[test_case(false ; "without_login")]
async fn unicode_output(login: bool) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| {
        builder.with_model("gpt-5.2").with_config(|config| {
            config.features.enable(Feature::PowershellUtf8);
        })
    })
    .await?;

    let call_id = "unicode_output";
    mount_shell_responses_with_timeout(
        &harness,
        call_id,
        "git -c alias.say='!printf \"%s\" \"naïve_café\"' say",
        Some(login),
        MEDIUM_TIMEOUT,
    )
    .await;
    harness.submit("run the command without login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "naïve_café")?;

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
#[test_case(true ; "with_login")]
#[test_case(false ; "without_login")]
async fn unicode_output_with_newlines(login: bool) -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = shell_command_harness_with(|builder| {
        builder.with_model("gpt-5.2").with_config(|config| {
            config.features.enable(Feature::PowershellUtf8);
        })
    })
    .await?;

    let call_id = "unicode_output";
    mount_shell_responses_with_timeout(
        &harness,
        call_id,
        "echo 'line1\nnaïve café\nline3'",
        Some(login),
        MEDIUM_TIMEOUT,
    )
    .await;
    harness.submit("run the command without login").await?;

    let output = harness.function_call_stdout(call_id).await;
    assert_shell_command_output(&output, "line1\\nnaïve café\\nline3")?;

    Ok(())
}
