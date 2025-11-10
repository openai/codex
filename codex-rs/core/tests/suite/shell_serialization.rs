#![cfg(not(target_os = "windows"))]

use anyhow::Result;
use codex_core::features::Feature;
use codex_core::model_family::find_family_for_model;
use codex_core::protocol::SandboxPolicy;
use core_test_support::assert_regex_match;
use core_test_support::responses::ev_apply_patch_function_call;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_local_shell_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use regex_lite::Regex;
use serde_json::Value;
use serde_json::json;
use std::fs;

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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_stays_json_without_freeform_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.disable(Feature::ApplyPatchFreeform);
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is a model family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-json";
    let args = json!({
        "command": ["/bin/echo", "shell json"],
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the json shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock.last_request().expect("shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    let mut parsed: Value = serde_json::from_str(output)?;
    if let Some(metadata) = parsed.get_mut("metadata").and_then(Value::as_object_mut) {
        // duration_seconds is non-deterministic; remove it for deep equality
        let _ = metadata.remove("duration_seconds");
    }

    assert_eq!(
        parsed
            .get("metadata")
            .and_then(|metadata| metadata.get("exit_code"))
            .and_then(Value::as_i64),
        Some(0),
        "expected zero exit code in unformatted JSON output",
    );
    let stdout = parsed
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert_regex_match(r"(?s)^shell json\n?$", stdout);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_is_structured_with_freeform_apply_patch() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ApplyPatchFreeform);
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-structured";
    let args = json!({
        "command": ["/bin/echo", "freeform shell"],
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the structured shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("structured shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("structured output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected structured shell output to be plain text",
    );
    let expected_pattern = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
freeform shell
?$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_preserves_fixture_json_without_serialization() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.disable(Feature::ApplyPatchFreeform);
        config.model = "gpt-5".to_string();
        config.model_family = find_family_for_model("gpt-5").expect("gpt-5 is a model family");
    });
    let test = builder.build(&server).await?;

    let fixture_path = test.cwd.path().join("fixture.json");
    fs::write(&fixture_path, FIXTURE_JSON)?;
    let fixture_path_str = fixture_path.to_string_lossy().to_string();

    let call_id = "shell-json-fixture";
    let args = json!({
        "command": ["/usr/bin/sed", "-n", "p", fixture_path_str],
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "read the fixture JSON with sed",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock.last_request().expect("shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("shell output string");

    let mut parsed: Value = serde_json::from_str(output)?;
    if let Some(metadata) = parsed.get_mut("metadata").and_then(Value::as_object_mut) {
        let _ = metadata.remove("duration_seconds");
    }

    assert_eq!(
        parsed
            .get("metadata")
            .and_then(|metadata| metadata.get("exit_code"))
            .and_then(Value::as_i64),
        Some(0),
        "expected zero exit code when serialization is disabled",
    );
    let stdout = parsed
        .get("output")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    assert_eq!(
        stdout, FIXTURE_JSON,
        "expected shell output to match the fixture contents"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_structures_fixture_with_serialization() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::ApplyPatchFreeform);
    });
    let test = builder.build(&server).await?;

    let fixture_path = test.cwd.path().join("fixture.json");
    fs::write(&fixture_path, FIXTURE_JSON)?;
    let fixture_path_str = fixture_path.to_string_lossy().to_string();

    let call_id = "shell-structured-fixture";
    let args = json!({
        "command": ["/usr/bin/sed", "-n", "p", fixture_path_str],
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "read the fixture JSON with structured output",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("structured output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("structured output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected structured output to be plain text"
    );
    let (header, body) = output
        .split_once("Output:\n")
        .expect("structured output contains an Output section");
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
async fn shell_output_for_freeform_tool_records_duration() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    #[cfg(target_os = "linux")]
    let sleep_cmd = vec!["/bin/bash", "-c", "sleep 1"];

    #[cfg(target_os = "macos")]
    let sleep_cmd = vec!["/bin/bash", "-c", "sleep 1"];

    #[cfg(windows)]
    let sleep_cmd = "timeout 1";

    let call_id = "shell-structured";
    let args = json!({
        "command": sleep_cmd,
        "timeout_ms": 2_000,
    });
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the structured shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("structured output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("structured output string");

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
        .expect("expected structured shell output to contain wall time seconds");
    assert!(
        wall_time_seconds > 0.5,
        "expected wall time to be greater than zero seconds, got {wall_time_seconds}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_reserializes_truncated_content() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("gpt-5-codex").expect("gpt-5 is a model family");
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-truncated";
    let args = json!({
        "command": ["/bin/sh", "-c", "seq 1 400"],
        "timeout_ms": 5_000,
    });
    let responses = vec![
        sse(vec![
            ev_response_created("resp-1"),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the truncation shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("truncated output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("truncated output string");

    assert!(
        serde_json::from_str::<Value>(output).is_err(),
        "expected truncated shell output to be plain text",
    );
    let truncated_pattern = r#"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Total output lines: 400
Output:
1
2
3
4
5
6
.*
\[\.{3} omitted \d+ of 400 lines \.{3}\]

.*
396
397
398
399
400
$"#;
    assert_regex_match(truncated_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_output_is_structured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "apply-patch-structured";
    let file_name = "structured.txt";
    let patch = format!(
        r#"*** Begin Patch
*** Add File: {file_name}
+from custom tool
*** End Patch
"#
    );
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_custom_tool_call(call_id, "apply_patch", &patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "apply the patch via custom tool",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("apply_patch output request recorded");
    let output_item = req.custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_creates_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "apply-patch-add-file";
    let file_name = "custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Add File: {file_name}\n+custom tool content\n*** End Patch\n"
    );
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_custom_tool_call(call_id, "apply_patch", &patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "apply_patch done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "apply the patch via custom tool to create a file",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("apply_patch output request recorded");
    let output_item = req.custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output);

    let new_file_path = test.cwd.path().join(file_name);
    let created_contents = fs::read_to_string(&new_file_path)?;
    assert_eq!(
        created_contents, "custom tool content\n",
        "expected file contents for {file_name}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_updates_existing_file() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "apply-patch-update-file";
    let file_name = "custom_tool_apply_patch_existing.txt";
    let file_path = test.cwd.path().join(file_name);
    fs::write(&file_path, "before\n")?;
    let patch = format!(
        "*** Begin Patch\n*** Update File: {file_name}\n@@\n-before\n+after\n*** End Patch\n"
    );
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_custom_tool_call(call_id, "apply_patch", &patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "apply_patch update done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "apply the patch via custom tool to update a file",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("apply_patch output request recorded");
    let output_item = req.custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
M {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output);

    let updated_contents = fs::read_to_string(file_path)?;
    assert_eq!(updated_contents, "after\n", "expected updated file content");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_custom_tool_call_reports_failure_output() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "apply-patch-failure";
    let missing_file = "missing_custom_tool_apply_patch.txt";
    let patch = format!(
        "*** Begin Patch\n*** Update File: {missing_file}\n@@\n-before\n+after\n*** End Patch\n"
    );
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_custom_tool_call(call_id, "apply_patch", &patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "apply_patch failure done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "attempt a failing apply_patch via custom tool",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("apply_patch output request recorded");
    let output_item = req.custom_tool_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");

    let expected_output = format!(
        "apply_patch verification failed: Failed to read file to update {}/{missing_file}: No such file or directory (os error 2)",
        test.cwd.path().to_string_lossy()
    );
    assert_eq!(output, expected_output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn apply_patch_function_call_output_is_structured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "apply-patch-function";
    let file_name = "function_apply_patch.txt";
    let patch =
        format!("*** Begin Patch\n*** Add File: {file_name}\n+via function call\n*** End Patch\n");
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_apply_patch_function_call(call_id, &patch),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "apply_patch function done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "apply the patch via function-call apply_patch",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("apply_patch function output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("apply_patch output string");

    let expected_pattern = format!(
        r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
Success. Updated the following files:
A {file_name}
?$"
    );
    assert_regex_match(&expected_pattern, output);

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shell_output_is_structured_for_nonzero_exit() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("gpt-5-codex").expect("gpt-5-codex is a model family");
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "shell-nonzero-exit";
    let args = json!({
        "command": ["/bin/sh", "-c", "exit 42"],
        "timeout_ms": 1_000,
    });
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_function_call(call_id, "shell", &serde_json::to_string(&args)?),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "shell failure handled"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the failing shell command",
        SandboxPolicy::DangerFullAccess,
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
async fn local_shell_call_output_is_structured() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.model = "gpt-5-codex".to_string();
        config.model_family =
            find_family_for_model("gpt-5-codex").expect("gpt-5-codex is a model family");
        config.include_apply_patch_tool = true;
    });
    let test = builder.build(&server).await?;

    let call_id = "local-shell-call";
    let responses = vec![
        sse(vec![
            json!({"type": "response.created", "response": {"id": "resp-1"}}),
            ev_local_shell_call(call_id, "completed", vec!["/bin/echo", "local shell"]),
            ev_completed("resp-1"),
        ]),
        sse(vec![
            ev_assistant_message("msg-1", "local shell done"),
            ev_completed("resp-2"),
        ]),
    ];
    let mock = mount_sse_sequence(&server, responses).await;

    test.submit_turn_with_policy(
        "run the local shell command",
        SandboxPolicy::DangerFullAccess,
    )
    .await?;

    let req = mock
        .last_request()
        .expect("local shell output request recorded");
    let output_item = req.function_call_output(call_id);
    let output = output_item
        .get("output")
        .and_then(Value::as_str)
        .expect("local shell output string");

    let expected_pattern = r"(?s)^Exit code: 0
Wall time: [0-9]+(?:\.[0-9]+)? seconds
Output:
local shell
?$";
    assert_regex_match(expected_pattern, output);

    Ok(())
}
