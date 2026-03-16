#![allow(clippy::expect_used, clippy::unwrap_used)]

use anyhow::Result;
use codex_core::features::Feature;
use codex_protocol::protocol::EventMsg;
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
use core_test_support::wait_for_event_match;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;
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

fn assert_js_repl_ok(req: &ResponsesRequest, call_id: &str, expected_output: &str) {
    let (output, success) = custom_tool_output_text_and_success(req, call_id);
    assert_ne!(
        success,
        Some(false),
        "js_repl call failed unexpectedly: {output}"
    );
    assert!(output.contains(expected_output), "output was: {output}");
}

fn assert_js_repl_err(req: &ResponsesRequest, call_id: &str, expected_output: &str) {
    let (output, success) = custom_tool_output_text_and_success(req, call_id);
    assert_ne!(success, Some(true), "js_repl call should fail: {output}");
    assert!(output.contains(expected_output), "output was: {output}");
}

fn tool_names(body: &serde_json::Value) -> Vec<String> {
    body["tools"]
        .as_array()
        .expect("tools array should be present")
        .iter()
        .map(|tool| {
            tool.get("name")
                .and_then(|value| value.as_str())
                .or_else(|| tool.get("type").and_then(|value| value.as_str()))
                .expect("tool should have a name or type")
                .to_string()
        })
        .collect()
}

fn write_too_old_node_script(dir: &Path) -> Result<std::path::PathBuf> {
    #[cfg(windows)]
    {
        let path = dir.join("old-node.cmd");
        fs::write(&path, "@echo off\r\necho v0.0.1\r\n")?;
        Ok(path)
    }

    #[cfg(unix)]
    {
        let path = dir.join("old-node.sh");
        fs::write(&path, "#!/bin/sh\necho v0.0.1\n")?;
        let mut permissions = fs::metadata(&path)?.permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&path, permissions)?;
        Ok(path)
    }

    #[cfg(not(any(unix, windows)))]
    {
        anyhow::bail!("unsupported platform for js_repl test fixture");
    }
}

async fn run_js_repl_turn(
    server: &MockServer,
    prompt: &str,
    calls: &[(&str, &str)],
) -> Result<ResponseMock> {
    let mut mocks = run_js_repl_sequence(server, prompt, calls).await?;
    Ok(mocks
        .pop()
        .expect("js_repl test should return a request mock"))
}

async fn run_js_repl_sequence(
    server: &MockServer,
    prompt: &str,
    calls: &[(&str, &str)],
) -> Result<Vec<ResponseMock>> {
    anyhow::ensure!(
        !calls.is_empty(),
        "js_repl test must include at least one call"
    );

    let mut builder = test_codex().with_config(|config| {
        config
            .features
            .enable(Feature::JsRepl)
            .expect("test config should allow feature update");
    });
    let test = builder.build(server).await?;

    responses::mount_sse_once(
        server,
        sse(vec![
            ev_response_created("resp-1"),
            ev_custom_tool_call(calls[0].0, "js_repl", calls[0].1),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    let mut mocks = Vec::with_capacity(calls.len());
    for (response_index, (call_id, js_input)) in calls.iter().enumerate().skip(1) {
        let response_id = format!("resp-{}", response_index + 1);
        let mock = responses::mount_sse_once(
            server,
            sse(vec![
                ev_response_created(&response_id),
                ev_custom_tool_call(call_id, "js_repl", js_input),
                ev_completed(&response_id),
            ]),
        )
        .await;
        mocks.push(mock);
    }

    let final_response_id = format!("resp-{}", calls.len() + 1);
    let final_mock = responses::mount_sse_once(
        server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed(&final_response_id),
        ]),
    )
    .await;
    mocks.push(final_mock);

    test.submit_turn(prompt).await?;
    Ok(mocks)
}

async fn assert_failed_cell_followup(
    server: &MockServer,
    prompt: &str,
    failing_cell: &str,
    followup_cell: &str,
    expected_followup_output: &str,
) -> Result<()> {
    let mocks = run_js_repl_sequence(
        server,
        prompt,
        &[("call-1", failing_cell), ("call-2", followup_cell)],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(
        &mocks[1].single_request(),
        "call-2",
        expected_followup_output,
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_is_not_advertised_when_startup_node_is_incompatible() -> Result<()> {
    skip_if_no_network!(Ok(()));
    if std::env::var_os("CODEX_JS_REPL_NODE_PATH").is_some() {
        return Ok(());
    }

    let server = responses::start_mock_server().await;
    let temp = tempdir()?;
    let old_node = write_too_old_node_script(temp.path())?;

    let mut builder = test_codex().with_config(move |config| {
        config
            .features
            .enable(Feature::JsRepl)
            .expect("test config should allow feature update");
        config.js_repl_node_path = Some(old_node);
    });
    let test = builder.build(&server).await?;
    let warning = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::Warning(ev) if ev.message.contains("Disabled `js_repl` for this session") => {
            Some(ev.message.clone())
        }
        _ => None,
    })
    .await;
    assert!(
        warning.contains("Node runtime"),
        "warning should explain the Node compatibility issue: {warning}"
    );

    let request_mock = responses::mount_sse_once(
        &server,
        sse(vec![
            ev_assistant_message("msg-1", "done"),
            ev_completed("resp-1"),
        ]),
    )
    .await;

    test.submit_turn("hello").await?;

    let body = request_mock.single_request().body_json();
    let tools = tool_names(&body);
    assert!(
        !tools.iter().any(|tool| tool == "js_repl"),
        "js_repl should be omitted when startup validation fails: {tools:?}"
    );
    assert!(
        !tools.iter().any(|tool| tool == "js_repl_reset"),
        "js_repl_reset should be omitted when startup validation fails: {tools:?}"
    );
    let instructions = body["instructions"].as_str().unwrap_or_default();
    assert!(
        !instructions.contains("## JavaScript REPL (Node)"),
        "startup instructions should not mention js_repl when it is disabled: {instructions}"
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_persists_top_level_destructured_bindings_and_supports_tla() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl twice",
        &[
            (
                "call-1",
                "const { context: liveContext, session } = await Promise.resolve({ context: 41, session: 1 }); console.log(liveContext + session);",
            ),
            ("call-2", "console.log(liveContext + session);"),
        ],
    )
    .await?;

    assert_js_repl_ok(&mocks[0].single_request(), "call-1", "42");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "42");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_commit_initialized_bindings_only() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl across a failed cell",
        &[
            ("call-1", "const base = 40; console.log(base);"),
            (
                "call-2",
                "const { session } = await Promise.resolve({ session: 2 }); throw new Error(\"boom\"); const late = 99;",
            ),
            ("call-3", "console.log(base + session, typeof late);"),
        ],
    )
    .await?;

    assert_js_repl_ok(&mocks[0].single_request(), "call-1", "40");
    assert_js_repl_err(&mocks[1].single_request(), "call-2", "boom");
    assert_js_repl_ok(&mocks[2].single_request(), "call-3", "42 undefined");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_preserve_initialized_lexical_destructuring_bindings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through partial destructuring failure",
        &[
            (
                "call-1",
                "const { a, b } = { a: 1, get b() { throw new Error(\"boom\"); } };",
            ),
            (
                "call-2",
                "let aValue; try { aValue = a; } catch (error) { aValue = error.name; } let bValue; try { bValue = b; } catch (error) { bValue = error.name; } console.log(aValue, bValue);",
            ),
        ],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "1 ReferenceError");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_link_failures_keep_prior_module_state() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl across a link failure",
        &[
            ("call-1", "const answer = 41; console.log(answer);"),
            ("call-2", "import value from \"./foo\";"),
            ("call-3", "console.log(answer + 1);"),
        ],
    )
    .await?;

    assert_js_repl_ok(&mocks[0].single_request(), "call-1", "41");
    assert_js_repl_err(
        &mocks[1].single_request(),
        "call-2",
        "Top-level static import \"./foo\" is not supported in js_repl",
    );
    assert_js_repl_ok(&mocks[2].single_request(), "call-3", "42");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_do_not_commit_unreached_hoisted_bindings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through hoisted binding failure",
        &[
            (
                "call-1",
                "var early = 1; throw new Error(\"boom\"); var late = 2; function fn() { return 1; }",
            ),
            (
                "call-2",
                "const late = 40; const fn = 1; console.log(early + late + fn);",
            ),
        ],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "42");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_do_not_preserve_hoisted_function_reads_before_declaration()
-> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through unsupported hoisted function reads",
        &[
            (
                "call-1",
                "foo(); throw new Error(\"boom\"); function foo() {}",
            ),
            (
                "call-2",
                "let value; try { foo; value = \"present\"; } catch (error) { value = error.name; } console.log(value);",
            ),
        ],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "ReferenceError");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_preserve_functions_when_declaration_sites_are_reached() -> Result<()>
{
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through supported function declaration persistence",
        &[
            ("call-1", "function foo() {} throw new Error(\"boom\");"),
            ("call-2", "console.log(typeof foo);"),
        ],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "function");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_preserve_prior_binding_writes_without_new_bindings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through failed prior-binding writes",
        &[
            ("call-1", "let x = 1; console.log(x);"),
            ("call-2", "x = 2; throw new Error(\"boom\");"),
            ("call-3", "console.log(x);"),
        ],
    )
    .await?;

    assert_js_repl_ok(&mocks[0].single_request(), "call-1", "1");
    assert_js_repl_err(&mocks[1].single_request(), "call-2", "boom");
    assert_js_repl_ok(&mocks[2].single_request(), "call-3", "2");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_var_persistence_boundaries() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let cases = [
        (
            "run js_repl through supported pre-declaration var writes",
            "x = 5; y = 1; y += 2; z = 1; z++; throw new Error(\"boom\"); var x, y, z;",
            "console.log(x, y, z);",
            "5 3 2",
        ),
        (
            "run js_repl through short-circuited logical var assignments",
            "x &&= 1; y ||= 2; z ??= 3; throw new Error(\"boom\"); var x, y, z;",
            "let xValue; try { xValue = x; } catch (error) { xValue = error.name; } console.log(xValue, y, z);",
            "ReferenceError 2 3",
        ),
        (
            "run js_repl through unsupported shadowed nested var writes",
            "{ let x = 1; x = 2; } throw new Error(\"boom\"); var x;",
            "let value; try { value = x; } catch (error) { value = error.name; } console.log(value);",
            "ReferenceError",
        ),
        (
            "run js_repl through unsupported nested assignment writes",
            "x = (y = 1); throw new Error(\"boom\"); var x, y;",
            "let yValue; try { yValue = y; } catch (error) { yValue = error.name; } console.log(x, yValue);",
            "1 ReferenceError",
        ),
        (
            "run js_repl through unsupported var destructuring recovery",
            "var { a, b } = { a: 1, get b() { throw new Error(\"boom\"); } };",
            "let aValue; try { aValue = a; } catch (error) { aValue = error.name; } let bValue; try { bValue = b; } catch (error) { bValue = error.name; } console.log(aValue, bValue);",
            "ReferenceError ReferenceError",
        ),
    ];

    for (prompt, failing_cell, followup_cell, expected_followup_output) in cases {
        assert_failed_cell_followup(
            &server,
            prompt,
            failing_cell,
            followup_cell,
            expected_followup_output,
        )
        .await?;
    }

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_failed_cells_commit_non_empty_loop_vars_but_skip_empty_loops() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mocks = run_js_repl_sequence(
        &server,
        "run js_repl through failed loop bindings",
        &[
            (
                "call-1",
                "for (var item of [2]) {} for (var emptyItem of []) {} throw new Error(\"boom\");",
            ),
            (
                "call-2",
                "let itemValue; try { itemValue = item; } catch (error) { itemValue = error.name; } let emptyValue; try { emptyValue = emptyItem; } catch (error) { emptyValue = error.name; } console.log(itemValue, emptyValue);",
            ),
        ],
    )
    .await?;

    assert_js_repl_err(&mocks[0].single_request(), "call-1", "boom");
    assert_js_repl_ok(&mocks[1].single_request(), "call-2", "2 ReferenceError");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_keeps_function_to_string_stable() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "run js_repl through function toString",
        &[(
            "call-1",
            "function foo() { return 1; } console.log(foo.toString());",
        )],
    )
    .await?;

    let req = mock.single_request();
    assert_js_repl_ok(&req, "call-1", "function foo() { return 1; }");
    let (output, _) = custom_tool_output_text_and_success(&req, "call-1");
    assert!(!output.contains("__codexInternalMarkCommittedBindings"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_allows_globalthis_shadowing_with_instrumented_bindings() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "run js_repl with shadowed globalThis",
        &[(
            "call-1",
            "const globalThis = {}; const value = 1; console.log(typeof globalThis, value);",
        )],
    )
    .await?;

    let req = mock.single_request();
    assert_js_repl_ok(&req, "call-1", "object 1");

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_can_invoke_builtin_tools() -> Result<()> {
    skip_if_no_network!(Ok(()));

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
async fn js_repl_exposes_codex_path_helpers() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let mock = run_js_repl_turn(
        &server,
        "check codex path helpers",
        &[(
            "call-1",
            "console.log(`cwd:${typeof codex.cwd}:${codex.cwd.length > 0}`); console.log(`home:${codex.homeDir === null || typeof codex.homeDir === \"string\"}`);",
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
    assert!(output.contains("cwd:string:true"));
    assert!(output.contains("home:true"));

    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn js_repl_blocks_sensitive_builtin_imports() -> Result<()> {
    skip_if_no_network!(Ok(()));

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
