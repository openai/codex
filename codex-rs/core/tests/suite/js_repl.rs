#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use core_test_support::responses;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ResponsesRequest;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_custom_tool_call;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::sse;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use which::which;
use wiremock::MockServer;

fn custom_tool_output_text_and_success(
    req: &ResponsesRequest,
    call_id: &str,
) -> (String, Option<bool>) {
    let (output, success) = req
        .custom_tool_call_output_content_and_success(call_id)
        .expect("custom tool output should be present");
    (output.unwrap_or_default(), success)
}

async fn run_js_repl_turn(
    server: &MockServer,
    prompt: &str,
    calls: &[(&str, &str)],
) -> Result<ResponseMock> {
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
    });
    let test = builder.build(server).await?;

    let mut first_events = vec![ev_response_created("resp-1")];
    for (call_id, js_input) in calls {
        first_events.push(ev_custom_tool_call(call_id, "js_repl", js_input));
    }
    first_events.push(ev_completed("resp-1"));
    responses::mount_sse_once(server, sse(first_events)).await;

    let second_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-2"),
        ]),
    )
    .await;

    test.submit_turn(prompt).await?;
    Ok(second_mock)
}

fn can_run_js_repl_runtime_tests() -> bool {
    // In the base CI-stabilization commit, skip these integration tests only
    // when the js_repl Node runtime is not available. A later stacked commit
    // removes this automatic skip and requires them everywhere.
    if let Some(path) = std::env::var_os("CODEX_JS_REPL_NODE_PATH")
        && std::path::Path::new(&path).exists()
    {
        return true;
    }
    which("node").is_ok()
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_persists_top_level_bindings_and_supports_tla() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if !can_run_js_repl_runtime_tests() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let mut builder = test_codex().with_config(|config| {
        config.features.enable(Feature::JsRepl);
    });
    let test = builder.build(&server).await?;

    responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(
                "call-1",
                "js_repl",
                "let x = await Promise.resolve(41); console.log(x);",
            ),
            ev_completed("resp-1"),
        ]),
    )
    .await;
    let second_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_response_created("resp-2"),
            ev_custom_tool_call("call-2", "js_repl", "console.log(x + 1);"),
            ev_completed("resp-2"),
        ]),
    )
    .await;
    let third_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-3"),
        ]),
    )
    .await;

    test.submit_turn("run js_repl twice").await?;

    let req2 = second_mock.single_request();
    let (first_output, first_success) = custom_tool_output_text_and_success(&req2, "call-1");
    assert_ne!(
        first_success,
        Some(false),
        "first js_repl call failed unexpectedly: {first_output}"
    );
    assert!(first_output.contains("41"));

    let req3 = third_mock.single_request();
    let (second_output, second_success) = custom_tool_output_text_and_success(&req3, "call-2");
    assert_ne!(
        second_success,
        Some(false),
        "second js_repl call failed unexpectedly: {second_output}"
    );
    assert!(second_output.contains("42"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_invoke_builtin_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if !can_run_js_repl_runtime_tests() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "use js_repl to call a tool",
        &[(
            "call-1",
            "const toolOut = await codex.tool(\"list_mcp_resources\", {}); console.log(toolOut.type);",
        )],
    )
    .await?;

    let req = mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "js_repl call failed unexpectedly: {output}"
    );
    assert!(output.contains("function_call_output"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_tool_call_rejects_recursive_js_repl_invocation() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if !can_run_js_repl_runtime_tests() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "use js_repl recursively",
        &[(
            "call-1",
            r#"
try {
  await codex.tool("js_repl", "console.log('recursive')");
  console.log("unexpected-success");
} catch (err) {
  console.log(String(err));
}
"#,
        )],
    )
    .await?;

    let req = mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "js_repl call failed unexpectedly: {output}"
    );
    assert!(
        output.contains("js_repl cannot invoke itself"),
        "expected recursion guard message, got output: {output}"
    );
    assert!(
        !output.contains("unexpected-success"),
        "recursive js_repl call unexpectedly succeeded: {output}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_does_not_expose_process_global() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if !can_run_js_repl_runtime_tests() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "check process visibility",
        &[("call-1", "console.log(typeof process);")],
    )
    .await?;

    let req = mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(false),
        "js_repl call failed unexpectedly: {output}"
    );
    assert!(output.contains("undefined"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_blocks_sensitive_builtin_imports() -> Result<()> {
    skip_if_no_network!(Ok(()));

    if !can_run_js_repl_runtime_tests() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "import a blocked module",
        &[("call-1", "await import(\"node:process\");")],
    )
    .await?;

    let req = mock.single_request();
    let (output, success) = custom_tool_output_text_and_success(&req, "call-1");
    assert_ne!(
        success,
        Some(true),
        "blocked import unexpectedly succeeded: {output}"
    );
    assert!(output.contains("Importing module \"node:process\" is not allowed in js_repl"));

    Ok(())
}
