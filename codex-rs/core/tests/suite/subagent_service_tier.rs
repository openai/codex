//! Verifies that root service-tier changes reach existing child turns and future work.

use super::agent_control::HostRuntime;
use anyhow::Result;
use codex_core::AgentRuntimeOwnership;
use codex_core::TurnInputRequest;
use codex_core::config::AgentRoleConfig;
use codex_core::config::Config;
use codex_extension_api::ExtensionRegistryBuilder;
use codex_features::Feature;
use codex_protocol::config_types::SERVICE_TIER_DEFAULT_REQUEST_VALUE;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::Op;
use codex_protocol::protocol::ThreadSettingsOverrides;
use codex_protocol::user_input::UserInput;
use core_test_support::responses::ResponseMock;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call_with_namespace;
use core_test_support::responses::ev_response_created;
use core_test_support::responses::mount_sse_once_match;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::submit_thread_settings;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use core_test_support::wait_for_event_match;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::Arc;
use test_case::test_case;

const ROOT_PROMPT: &str = "spawn the service-tier worker";
const CHILD_PROMPT: &str = "pause the service-tier worker";
const FOLLOWUP_PROMPT: &str = "continue the service-tier worker later";
const FRESH_ROOT_PROMPT: &str = "spawn another service-tier worker";
const FRESH_CHILD_PROMPT: &str = "finish the new service-tier worker";
const SPAWN_CALL_ID: &str = "spawn-service-tier-worker";
const FRESH_SPAWN_CALL_ID: &str = "spawn-fresh-service-tier-worker";
const PAUSE_CALL_ID: &str = "pause-service-tier-worker";
const PRIORITY_ROLE: &str = "priority-worker";

fn body_contains(request: &wiremock::Request, text: &str) -> bool {
    let body = match request
        .headers
        .get("content-encoding")
        .and_then(|encoding| encoding.to_str().ok())
    {
        Some(encoding) if encoding.eq_ignore_ascii_case("zstd") => {
            zstd::stream::decode_all(std::io::Cursor::new(&request.body)).ok()
        }
        _ => Some(request.body.clone()),
    };
    body.and_then(|body| String::from_utf8(body).ok())
        .is_some_and(|body| body.contains(text))
}

fn assert_request_service_tier(request: &ResponseMock, expected: Option<&str>) {
    assert_eq!(
        request
            .single_request()
            .body_json()
            .get("service_tier")
            .and_then(serde_json::Value::as_str),
        expected
    );
}

fn configure_priority_role(config: &mut Config) {
    for feature in [Feature::Collab, Feature::MultiAgentV2] {
        config
            .features
            .enable(feature)
            .expect("test config should allow feature update");
    }
    config.model_provider.request_max_retries = Some(0);
    config.model_provider.stream_max_retries = Some(0);
    let role_path = config.codex_home.join("priority-worker.toml");
    std::fs::write(&role_path, "service_tier = \"priority\"\n")
        .expect("priority role should be written");
    config.agent_roles.insert(
        PRIORITY_ROLE.to_string(),
        AgentRoleConfig {
            description: Some("Role with a configured priority tier".to_string()),
            config_file: Some(role_path.to_path_buf()),
            nickname_candidates: None,
        },
    );
}

async fn wait_for_turn_complete(thread: &codex_core::CodexThread) {
    wait_for_event(thread, |event| matches!(event, EventMsg::TurnComplete(_))).await;
}

async fn mount_root_collaboration_call(
    server: &wiremock::MockServer,
    prompt: &'static str,
    call_id: &'static str,
    arguments: serde_json::Value,
) {
    let response_id = format!("root-{call_id}");
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| {
            body_contains(request, prompt) && !body_contains(request, call_id)
        },
        sse(vec![
            ev_response_created(&response_id),
            ev_function_call_with_namespace(
                call_id,
                "collaboration",
                "spawn_agent",
                &arguments.to_string(),
            ),
            ev_completed(&response_id),
        ]),
    )
    .await;

    let completion_id = format!("root-{call_id}-finished");
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| {
            body_contains(request, prompt) && body_contains(request, call_id)
        },
        sse(vec![
            ev_response_created(&completion_id),
            ev_assistant_message(&format!("{completion_id}-message"), "worker started"),
            ev_completed(&completion_id),
        ]),
    )
    .await;
}

async fn mount_completed_child(
    server: &wiremock::MockServer,
    prompt: &'static str,
    root_prompt: &'static str,
) -> ResponseMock {
    mount_sse_once_match(
        server,
        move |request: &wiremock::Request| {
            body_contains(request, prompt) && !body_contains(request, root_prompt)
        },
        sse(vec![
            ev_response_created(prompt),
            ev_assistant_message(&format!("{prompt}-message"), "worker completed"),
            ev_completed(prompt),
        ]),
    )
    .await
}

#[test_case(Some("priority"), None; "disabling fast mode updates active and idle child work")]
#[test_case(Some("priority"), Some("default"); "explicit default updates active and idle child work")]
#[test_case(None, Some("priority"); "enabling fast mode updates active and idle child work")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn root_service_tier_change_updates_existing_subagent(
    initial_service_tier: Option<&str>,
    updated_service_tier: Option<&str>,
) -> Result<()> {
    let server = start_mock_server().await;
    let initial_service_tier_owned = initial_service_tier.map(str::to_string);
    let updated_request_service_tier = updated_service_tier
        .filter(|service_tier| *service_tier != SERVICE_TIER_DEFAULT_REQUEST_VALUE);
    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_config(move |config| {
            config.service_tier = initial_service_tier_owned;
            configure_priority_role(config);
        });
    let test = builder.build_with_auto_env(&server).await?;
    assert!(!test.config.features.enabled(Feature::StepModelSwitching));
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    mount_root_collaboration_call(
        &server,
        ROOT_PROMPT,
        SPAWN_CALL_ID,
        json!({
            "message": CHILD_PROMPT,
            "task_name": "worker",
            "agent_type": PRIORITY_ROLE,
            "fork_turns": "none",
        }),
    )
    .await;

    let initial_child_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, CHILD_PROMPT)
                && !body_contains(request, ROOT_PROMPT)
                && !body_contains(request, PAUSE_CALL_ID)
        },
        sse(vec![
            ev_response_created("child-paused"),
            ev_function_call_with_namespace(
                PAUSE_CALL_ID,
                "collaboration",
                "wait_agent",
                &json!({ "timeout_ms": 30_000 }).to_string(),
            ),
            ev_completed("child-paused"),
        ]),
    )
    .await;
    let continued_child_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| {
            body_contains(request, PAUSE_CALL_ID) && !body_contains(request, ROOT_PROMPT)
        },
        sse(vec![
            ev_response_created("child-continued"),
            ev_assistant_message("child-continued-message", "child continued"),
            ev_completed("child-continued"),
        ]),
    )
    .await;
    test.submit_text_turn(ROOT_PROMPT).await?;
    let child_thread_id = created_threads.recv().await?;
    let child = test.thread_manager.get_thread(child_thread_id).await?;
    let original_child_service_tier = child.config_snapshot().await.service_tier;
    assert_eq!(original_child_service_tier.as_deref(), initial_service_tier);
    wait_for_event_match(child.as_ref(), |event| match event {
        EventMsg::CollabWaitingBegin(request) if request.call_id == PAUSE_CALL_ID => Some(()),
        EventMsg::Error(error) => panic!("child failed before pausing: {}", error.message),
        EventMsg::TurnComplete(completed) => {
            panic!("child completed before pausing: {completed:?}")
        }
        _ => None,
    })
    .await;
    assert_request_service_tier(&initial_child_request, initial_service_tier);

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            service_tier: Some(updated_service_tier.map(str::to_string)),
            ..Default::default()
        },
    )
    .await?;
    assert_eq!(
        child.config_snapshot().await.service_tier,
        original_child_service_tier,
        "the root routing policy must not rewrite child-owned settings"
    );

    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "continue after the service-tier update".to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&child).await;
    assert_request_service_tier(&continued_child_request, updated_request_service_tier);

    let child_compaction_request = mount_sse_once_match(
        &server,
        |request: &wiremock::Request| body_contains(request, "compaction_trigger"),
        sse(vec![
            ev_response_created("child-remote-compaction"),
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "compacted child history",
                },
            }),
            ev_completed("child-remote-compaction"),
        ]),
    )
    .await;
    child.submit(Op::Compact).await?;
    wait_for_turn_complete(&child).await;
    assert_request_service_tier(&child_compaction_request, updated_request_service_tier);

    let future_child_request = mount_completed_child(&server, FOLLOWUP_PROMPT, ROOT_PROMPT).await;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: FOLLOWUP_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&child).await;
    assert_request_service_tier(&future_child_request, updated_request_service_tier);

    mount_root_collaboration_call(
        &server,
        FRESH_ROOT_PROMPT,
        FRESH_SPAWN_CALL_ID,
        json!({
            "message": FRESH_CHILD_PROMPT,
            "task_name": "fresh",
            "agent_type": PRIORITY_ROLE,
            "fork_turns": "none",
        }),
    )
    .await;
    let fresh_child_request =
        mount_completed_child(&server, FRESH_CHILD_PROMPT, FRESH_ROOT_PROMPT).await;
    test.submit_text_turn(FRESH_ROOT_PROMPT).await?;
    let fresh_thread_id = created_threads.recv().await?;
    let fresh_thread = test.thread_manager.get_thread(fresh_thread_id).await?;
    wait_for_turn_complete(&fresh_thread).await;
    assert_request_service_tier(&fresh_child_request, updated_request_service_tier);
    Ok(())
}

#[test_case(AgentRuntimeOwnership::Controller, None; "controller")]
#[test_case(AgentRuntimeOwnership::Host, Some("priority"); "host without a tier provider")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn evicted_role_subagent_uses_owned_service_tier_after_reload(
    owner: AgentRuntimeOwnership,
    expected: Option<&str>,
) -> Result<()> {
    let server = start_mock_server().await;
    let mut extensions = ExtensionRegistryBuilder::new();
    if owner == AgentRuntimeOwnership::Host {
        extensions.thread_lifecycle_contributor(Arc::new(HostRuntime));
    }
    let mut builder = test_codex()
        .with_model("gpt-5.6-sol")
        .with_extensions(Arc::new(extensions.build()))
        .with_config(move |config| {
            config.service_tier = Some(match owner {
                AgentRuntimeOwnership::Controller => "priority".to_string(),
                AgentRuntimeOwnership::Host => SERVICE_TIER_DEFAULT_REQUEST_VALUE.to_string(),
            });
            config.multi_agent_v2.max_concurrent_threads_per_session = 2;
            configure_priority_role(config);
        });
    let test = builder.build_with_auto_env(&server).await?;
    let mut created_threads = test.thread_manager.subscribe_thread_created();

    mount_root_collaboration_call(
        &server,
        ROOT_PROMPT,
        SPAWN_CALL_ID,
        json!({
            "message": CHILD_PROMPT,
            "task_name": "original",
            "agent_type": PRIORITY_ROLE,
            "fork_turns": "none",
        }),
    )
    .await;
    let original_request = mount_completed_child(&server, CHILD_PROMPT, ROOT_PROMPT).await;
    test.submit_text_turn(ROOT_PROMPT).await?;
    let original_thread_id = created_threads.recv().await?;
    let original_thread = test.thread_manager.get_thread(original_thread_id).await?;
    wait_for_turn_complete(&original_thread).await;
    assert_request_service_tier(&original_request, Some("priority"));
    drop(original_thread);

    mount_root_collaboration_call(
        &server,
        FRESH_ROOT_PROMPT,
        FRESH_SPAWN_CALL_ID,
        json!({
            "message": FRESH_CHILD_PROMPT,
            "task_name": "replacement",
            "fork_turns": "none",
        }),
    )
    .await;
    mount_completed_child(&server, FRESH_CHILD_PROMPT, FRESH_ROOT_PROMPT).await;
    test.submit_text_turn(FRESH_ROOT_PROMPT).await?;
    let replacement_thread_id = created_threads.recv().await?;
    let replacement_thread = test
        .thread_manager
        .get_thread(replacement_thread_id)
        .await?;
    wait_for_turn_complete(&replacement_thread).await;
    assert!(
        test.thread_manager
            .get_thread(original_thread_id)
            .await
            .is_err()
    );

    submit_thread_settings(
        &test.codex,
        ThreadSettingsOverrides {
            service_tier: Some(None),
            ..Default::default()
        },
    )
    .await?;
    test.thread_manager
        .ensure_multi_agent_v2_child_loaded(original_thread_id)
        .await?;
    let reloaded_thread = test.thread_manager.get_thread(original_thread_id).await?;
    let expected_configured_tier = match owner {
        AgentRuntimeOwnership::Controller => test.codex.config_snapshot().await.service_tier,
        AgentRuntimeOwnership::Host => expected.map(str::to_string),
    };
    assert_eq!(
        reloaded_thread.config_snapshot().await.service_tier,
        expected_configured_tier,
    );

    let reloaded_request = mount_completed_child(&server, FOLLOWUP_PROMPT, ROOT_PROMPT).await;
    reloaded_thread
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: FOLLOWUP_PROMPT.to_string(),
            text_elements: Vec::new(),
        }]))
        .await?;
    wait_for_turn_complete(&reloaded_thread).await;
    assert_request_service_tier(&reloaded_request, expected);
    reloaded_thread.shutdown_and_wait().await?;
    test.codex.shutdown_and_wait().await?;

    Ok(())
}
