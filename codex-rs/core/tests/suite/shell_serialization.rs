#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_protocol::models::PermissionProfile;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::skip_if_target_windows;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;

use crate::suite::apply_patch_cli::apply_patch_harness;
use crate::suite::apply_patch_cli::mount_apply_patch;

const FIXTURE_JSON: &str = r#"{
    "description": "This is an example JSON file.",
    "foo": "bar",
    "isTest": true,
    "testNumber": 123,
    "testArray": [1, 2, 3],
    "testObject": {
        "foo": "bar"
    }
}
"#;

fn shell_responses(call_id: &str, command: Vec<&str>) -> Result<Vec<String>> {
    let command = shlex::try_join(command)?;
    let parameters = json!({
        "command": command,
        "timeout_ms": 2_000,
    });
    Ok(vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(
                call_id,
                "shell_command",
                &serde_json::to_string(&parameters)?,
            ),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ])
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_preserves_fixture_json_as_freeform() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;

    let fixture_path = test.cwd.path().join("fixture.json");
    fs::write(&fixture_path, FIXTURE_JSON)?;
    let fixture_path_str = fixture_path.to_string_lossy().to_string();

    let call_id = "shell-freeform-fixture";
    let responses = shell_responses(
        call_id,
        vec!["/usr/bin/sed", "-n", "p", fixture_path_str.as_str()],
    )?;
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile(
        "read the fixture JSON with shell output",
        PermissionProfile::Disabled,
    )
    .await?;

    let req = mock.last_request().expect("shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected shell output to be plain text"
    );
    let (header, body) = output
        .split_once("Output:\n")
        .expect("shell output contains an Output section");
    assert_regex_match(
        r"(?s)^Exit code: 0\nWall time: [0-9]+(?:\.[0-9]+)? seconds$",
        header.trim_end(),
    );
    assert_eq!(
        body, FIXTURE_JSON,
        "expected Output section to include the fixture contents"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_records_duration() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("test-gpt-5-codex");
    let test = builder.build(&server).await?;

    let call_id = "shell-freeform";
    let responses = shell_responses(call_id, vec!["/bin/sh", "-c", "sleep 0.2"])?;
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile("run the shell command", PermissionProfile::Disabled)
        .await?;

    let req = mock.last_request().expect("shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    let expected_pattern = r#"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
$"#;
    assert_regex_match(expected_pattern, output);

    let wall_time_regex = Regex::new(r"(?m)^Wall (?:time|Clock): ([0-9]+(?:\.[0-9]+)?) seconds$")
        .expect("compile wall time regex");
    let wall_time_seconds = wall_time_regex
        .captures(output)
        .and_then(|caps| caps.get(1))
        .and_then(|value| value.as_str().parse::<f32>().ok())
        .expect("expected shell output to contain wall time seconds");
    assert!(
        wall_time_seconds > 0.1,
        "expected wall time to be greater than zero seconds, got {wall_time_seconds}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_creates_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-add-file";
    let file_name = "custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Add File: {file_name}\n+custom tool content\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "apply the patch via custom tool to create a file",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output.as_str());

    let created_contents = harness.read_file_text(file_name).await?;
    assert_eq!(
        created_contents, "custom tool content\n",
        "expected file contents for {file_name}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_updates_existing_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-update-file";
    let file_name = "custom_tool_apply_patch_existing.txt";
    harness.write_file(file_name, "before\n").await?;
    let patch = format!(
        "*** Begin Patch\n*** Update File: {file_name}\n@@\n-before\n+after\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch update done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "apply the patch via custom tool to update a file",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
M {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output.as_str());

    let updated_contents = harness.read_file_text(file_name).await?;
    assert_eq!(updated_contents, "after\n", "expected updated file content");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_reports_failure_output() -> Result<()> {
    // TODO(anp): Remove after apply-patch assertions use target-native paths.
    skip_if_target_windows!(Ok(()), "asserts POSIX apply_patch failure text");
    skip_if_no_network!(Ok(()));

    let harness = apply_patch_harness().await?;

    let call_id = "apply-patch-failure";
    let missing_file = "missing_custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Update File: {missing_file}\n@@\n-before\n+after\n*** End Patch\n"
    );
    mount_apply_patch(&harness, call_id, &patch, "apply_patch failure done").await;

    harness
        .test()
        .submit_turn_with_permission_profile(
            "attempt a failing apply_patch via custom tool",
            PermissionProfile::Disabled,
        )
        .await?;

    let output = harness.apply_patch_output(call_id).await;

    let expected_output = format!(
        "apply_patch verification failed: Failed to read file to update {}/{missing_file}: No such file or directory (os error 2)",
        harness.cwd().to_string_lossy()
    );
    assert_eq!(output, expected_output.as_str());

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_is_freeform_for_nonzero_exit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build(&server).await?;

    let call_id = "shell-nonzero-exit";
    let responses = shell_responses(call_id, vec!["/bin/sh", "-c", "exit 42"])?;
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile(
        "run the failing shell command",
        PermissionProfile::Disabled,
    )
    .await?;

    let req = mock.last_request().expect("shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    let expected_pattern = r"(?s)^Exit code: 42
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
?$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_output_is_freeform() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex();
    let test = builder.build(&server).await?;

    let call_id = "shell-command";
    let args = json!({
        "command": "echo shell command",
        "login": false,
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "shell_command done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile(
        "run the shell_command script in the user's shell",
        PermissionProfile::Disabled,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("shell_command output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell_command output string");

    let expected_pattern = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
shell command
?$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_output_is_not_truncated_under_10k_bytes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.4");
    let test = builder.build(&server).await?;

    let call_id = "shell-command";
    let args = json!({
        "command": "perl -e 'print \"1\" x 10000'",
        "login": false,
        "timeout_ms": 1000,
    });
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "shell_command done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile(
        "run the shell_command script in the user's shell",
        PermissionProfile::Disabled,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("shell_command output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell_command output string");

    let expected_pattern = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
1{10000}$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_command_output_is_not_truncated_over_10k_bytes() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_model("gpt-5.2");
    let test = builder.build(&server).await?;

    let call_id = "shell-command";
    let args = json!({
        "command": "perl -e 'print \"1\" x 10001'",
        "login": false,
        "timeout_ms": 1000,
    });
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell_command", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "shell_command done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_permission_profile(
        "run the shell_command script in the user's shell",
        PermissionProfile::Disabled,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("shell_command output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell_command output string");

    let expected_pattern = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
1*…1 chars truncated…1*$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}
