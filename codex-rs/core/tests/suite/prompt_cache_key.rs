use std::time::Duration;

use anyhow::Result;
use anyhow::anyhow;
use codex_features::Feature;
use codex_login::CodexAuth;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;

const ROOT_PROMPT: &str = "delegate the cache audit";
const CHILD_TASK: &str = "inspect the repository";
const SPAWN_CALL_ID: &str = "spawn-worker";
const COLLABORATION_NAMESPACE: &str = "collaboration";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    serde_json::from_slice::<Value>(&request.body).is_ok_and(|body| body.to_string().contains(text))
}

fn request_has_input_type(request: &wiremock::Request, input_type: &str) -> bool {
    serde_json::from_slice::<Value>(&request.body)
        .ok()
        .and_then(|body| body.get("input").and_then(Value::as_array).cloned())
        .is_some_and(|items| {
            items
                .iter()
                .any(|item| item.get("type").and_then(Value::as_str) == Some(input_type))
        })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn api_key_subagent_uses_session_id_as_prompt_cache_key() -> Result<()> {
    let server = start_mock_server().await;
    let spawn_args = serde_json::to_string(&json!({
        "message": CHILD_TASK,
        "task_name": "worker",
    }))?;
    let root_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, ROOT_PROMPT)
                && !request_has_input_type(request, "agent_message")
                && !body_contains(request, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("root-response-1"),
            ev_function_call_with_namespace(
                SPAWN_CALL_ID,
                COLLABORATION_NAMESPACE,
                "spawn_agent",
                &spawn_args,
            ),
            ev_completed("root-response-1"),
        ]),
    )
    .await;
    let child_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, CHILD_TASK) && !body_contains(request, SPAWN_CALL_ID)
        },
        sse(vec![
            ev_response_created("child-response"),
            ev_assistant_message("child-message", "inspection complete"),
            ev_completed("child-response"),
        ]),
    )
    .await;
    mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, SPAWN_CALL_ID),
        sse(vec![
            ev_response_created("root-response-2"),
            ev_assistant_message("root-message", "worker finished"),
            ev_completed("root-response-2"),
        ]),
    )
    .await;

    let mut builder = test_codex()
        .with_auth(CodexAuth::from_api_key("dummy"))
        .with_config(|config| {
            config
                .features
                .enable(Feature::Collab)
                .expect("test config should allow feature update");
            config
                .features
                .enable(Feature::MultiAgentV2)
                .expect("test config should allow feature update");
        });
    let test = builder.build(&server).await?;
    let expected_session_id = test.session_configured.session_id.to_string();
    test.submit_turn(ROOT_PROMPT).await?;

    let root_request = root_request
        .requests()
        .into_iter()
        .next()
        .expect("root request");
    let root_thread_id = root_request.header("thread-id").expect("root thread ID");
    let child_request = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(request) = child_request.requests().into_iter().find(|request| {
                request
                    .header("thread-id")
                    .is_some_and(|thread_id| thread_id != root_thread_id)
            }) {
                break request;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .map_err(|_| anyhow!("timed out waiting for the child request"))?;
    let child_thread_id = child_request.header("thread-id").expect("child thread ID");

    assert_eq!(
        json!({
            "differentThreadIds": root_thread_id != child_thread_id,
            "root": {
                "sessionId": root_request.header("session-id"),
                "threadId": &root_thread_id,
                "clientRequestId": root_request.header("x-client-request-id"),
                "promptCacheKey": root_request.body_json()["prompt_cache_key"].clone(),
            },
            "child": {
                "sessionId": child_request.header("session-id"),
                "threadId": &child_thread_id,
                "clientRequestId": child_request.header("x-client-request-id"),
                "promptCacheKey": child_request.body_json()["prompt_cache_key"].clone(),
            },
        }),
        json!({
            "differentThreadIds": true,
            "root": {
                "sessionId": &expected_session_id,
                "threadId": &root_thread_id,
                "clientRequestId": &root_thread_id,
                "promptCacheKey": &expected_session_id,
            },
            "child": {
                "sessionId": &expected_session_id,
                "threadId": &child_thread_id,
                "clientRequestId": &child_thread_id,
                "promptCacheKey": &expected_session_id,
            },
        })
    );

    Ok(())
}

#[tokio::test]
async fn ephemeral_fork_shares_cache_routing_but_keeps_session_identity() -> Result<()> {
    use codex_core::ForkSnapshot;
    use codex_core::StartThreadOptions;
    use core_test_support::responses::mount_sse_sequence;

    let server = start_mock_server().await;
    let requests = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("parent")]),
            sse(vec![ev_completed("fork")]),
        ],
    )
    .await;
    let mut test = test_codex().build_with_auto_env(&server).await?;
    test.submit_text_turn("parent").await?;
    test.codex.flush_rollout().await?;
    let parent_session = test.session_configured.session_id.to_string();
    let mut config = test.config.clone();
    config.ephemeral = true;
    let fork = test
        .thread_manager
        .fork_thread(
            ForkSnapshot::TruncateBeforeNthUserMessage(usize::MAX),
            StartThreadOptions {
                environments: Some(test.codex.environment_selections().await),
                ..StartThreadOptions::new(config)
            },
            test.codex.rollout_path().expect("parent rollout"),
        )
        .await?;
    let fork_session = fork.session_configured.session_id.to_string();
    assert_ne!(fork_session, parent_session);
    test.codex = fork.thread;
    test.submit_text_turn("side").await?;
    let requests = requests.requests();
    let body = requests[1].body_json();
    let metadata: Value = serde_json::from_str(
        body["client_metadata"]["x-codex-turn-metadata"]
            .as_str()
            .expect("turn metadata"),
    )?;
    assert_eq!(
        json!({
            "cache": body["prompt_cache_key"],
            "route": requests[1].header("session-id"),
            "session": metadata["session_id"],
            "thread": requests[1].header("thread-id"),
            "tools": body["tools"],
        }),
        json!({
            "cache": parent_session, "route": parent_session,
            "session": fork_session, "thread": fork.thread_id.to_string(),
            "tools": requests[0].body_json()["tools"],
        }),
    );
    Ok(())
}
