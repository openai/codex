use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex_exec::test_codex_exec;
use pretty_assertions::assert_eq;
use serde_json::json;

const PARENT_PROMPT: &str = "spawn a child and wait for it";
const CHILD_PROMPT: &str = "child: finish first";
const SPAWN_CALL_ID: &str = "spawn-call";
const WAIT_CALL_ID: &str = "wait-call";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    std::str::from_utf8(&request.body).is_ok_and(|body| body.contains(text))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ignores_unrelated_turn_completion_before_backfilling_primary_turn() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let spawn_args = json!({
        "message": CHILD_PROMPT,
        "task_name": "worker",
        "fork_turns": "none",
    })
    .to_string();
    let _parent_turn = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, PARENT_PROMPT),
        responses::sse(vec![
            responses::ev_response_created("resp-parent-1"),
            responses::ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                "collaboration",
                "spawn_agent",
                &spawn_args,
            ),
            responses::ev_completed("resp-parent-1"),
        ]),
    )
    .await;
    let _child_turn = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, CHILD_PROMPT) && !body_contains(request, SPAWN_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resp-child"),
            responses::ev_assistant_message("msg-child", "child done"),
            responses::ev_completed("resp-child"),
        ]),
    )
    .await;
    let _parent_wait = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, SPAWN_CALL_ID) && !body_contains(request, WAIT_CALL_ID)
        },
        responses::sse(vec![
            responses::ev_response_created("resp-parent-2"),
            responses::ev_function_call_with_namespace(
                WAIT_CALL_ID,
                "collaboration",
                "wait_agent",
                "{}",
            ),
            responses::ev_completed("resp-parent-2"),
        ]),
    )
    .await;
    let _parent_completion = responses::mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, WAIT_CALL_ID),
        responses::sse(vec![
            responses::ev_response_created("resp-parent-3"),
            responses::ev_assistant_message("msg-parent", "parent done"),
            responses::ev_completed("resp-parent-3"),
        ]),
    )
    .await;

    let test = test_codex_exec();
    let mock_provider = format!(
        "model_providers.mock_provider={{name=\"Mock provider for test\",base_url=\"{}/v1\",wire_api=\"responses\",supports_websockets=false}}",
        server.uri()
    );
    let output = test
        .cmd()
        .env(
            "RUST_LOG",
            "codex_app_server::message_processor=trace,codex_app_server::outgoing_message=trace",
        )
        .arg("--skip-git-repo-check")
        .arg("--json")
        .arg("-c")
        .arg(mock_provider)
        .args([
            "-c",
            "model_provider=\"mock_provider\"",
            "-c",
            "features.multi_agent=true",
            "-c",
            "features.multi_agent_v2=true",
            "-c",
            "features.enable_request_compression=false",
        ])
        .arg(PARENT_PROMPT)
        .output()?;
    assert!(output.status.success(), "exec run failed: {output:?}");

    let stdout = String::from_utf8(output.stdout)?;
    assert!(
        stdout.contains("parent done"),
        "primary completion was not processed: {stdout}"
    );
    let stderr = String::from_utf8(output.stderr)?;
    let lines = stderr.lines().collect::<Vec<_>>();
    let turn_completions = lines
        .iter()
        .enumerate()
        .filter_map(|(index, line)| {
            line.contains("app-server event: turn/completed")
                .then_some(index)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        turn_completions.len(),
        2,
        "expected the child completion before the primary completion: {stderr}"
    );

    let [child_completion, primary_completion] = turn_completions.as_slice() else {
        unreachable!("checked turn/completed count")
    };
    assert_eq!(
        lines[*child_completion + 1..*primary_completion]
            .iter()
            .filter(|line| line.contains("app-server typed request"))
            .count(),
        0,
        "the unrelated completion must not issue thread/read: {stderr}"
    );
    assert_eq!(
        lines[*primary_completion + 1..]
            .iter()
            .filter(|line| line.contains("app-server typed request"))
            .count(),
        2,
        "the primary completion should issue thread/read and thread/unsubscribe: {stderr}"
    );

    assert_eq!(
        lines
            .iter()
            .filter(|line| line.contains("app-server typed request"))
            .count(),
        5,
        "only the primary completion should issue an extra request: {stderr}"
    );

    Ok(())
}
