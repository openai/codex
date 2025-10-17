#![cfg(not(target_os = "windows"))]

use std::sync::Arc;
use std::time::Duration;

use codex_core::CodexAuth;
use codex_core::ConversationManager;
use codex_core::CrossSessionSpawnParams;
use codex_core::built_in_model_providers;
use codex_core::config::Config;
use codex_core::cross_session::AssistantMessage;
use codex_core::cross_session::CrossSessionHub;
use codex_core::cross_session::PostUserTurnRequest;
use codex_core::cross_session::RoleOrId;
use codex_core::cross_session::SessionEventStream;
use codex_core::protocol::EventMsg;
use core_test_support::load_default_config_for_test;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use tempfile::TempDir;
use tokio_stream::StreamExt;
use wiremock::MockServer;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cross_session_hub_routes_between_roles() -> anyhow::Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let bodies = vec![
        responses::sse(vec![
            responses::ev_response_created("solver-resp-1"),
            responses::ev_assistant_message("solver-msg-1", "Need direction"),
            responses::ev_completed("solver-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("director-resp-1"),
            responses::ev_assistant_message("director-msg-1", "Proceed iteratively"),
            responses::ev_completed("director-resp-1"),
        ]),
        responses::sse(vec![
            responses::ev_response_created("solver-resp-2"),
            responses::ev_assistant_message("solver-msg-2", "Acknowledged"),
            responses::ev_completed("solver-resp-2"),
        ]),
    ];
    let response_mock = responses::mount_sse_sequence(&server, bodies).await;

    let hub = Arc::new(CrossSessionHub::new());
    let conversation_manager = ConversationManager::with_auth(CodexAuth::from_api_key("dummy-key"));
    let run_id = "run-cross-session".to_string();

    let solver_config = build_config(&server).await?;
    let solver = conversation_manager
        .new_conversation_with_cross_session(
            solver_config,
            CrossSessionSpawnParams {
                hub: Arc::clone(&hub),
                run_id: Some(run_id.clone()),
                role: Some("solver".to_string()),
            },
        )
        .await?;

    let director_config = build_config(&server).await?;
    let director = conversation_manager
        .new_conversation_with_cross_session(
            director_config,
            CrossSessionSpawnParams {
                hub: Arc::clone(&hub),
                run_id: Some(run_id.clone()),
                role: Some("director".to_string()),
            },
        )
        .await?;

    let mut solver_events = hub.stream_events(solver.conversation_id)?;
    let mut director_events = hub.stream_events(director.conversation_id)?;

    let solver_handle = hub
        .post_user_turn(PostUserTurnRequest {
            target: RoleOrId::RunRole {
                run_id: run_id.clone(),
                role: "solver".to_string(),
            },
            text: "kick off plan".to_string(),
            final_output_json_schema: None,
        })
        .await?;
    let solver_first = expect_message(&hub, &solver_handle, "Need direction").await?;

    let director_handle = hub
        .post_user_turn(PostUserTurnRequest {
            target: RoleOrId::RunRole {
                run_id: run_id.clone(),
                role: "director".to_string(),
            },
            text: solver_first.message.message.clone(),
            final_output_json_schema: None,
        })
        .await?;
    let director_first = expect_message(&hub, &director_handle, "Proceed iteratively").await?;

    let solver_followup = hub
        .post_user_turn(PostUserTurnRequest {
            target: RoleOrId::Session(solver.conversation_id),
            text: director_first.message.message.clone(),
            final_output_json_schema: None,
        })
        .await?;
    let solver_reply = expect_message(&hub, &solver_followup, "Acknowledged").await?;

    let solver_event = expect_agent_event(&mut solver_events).await;
    match solver_event {
        EventMsg::AgentMessage(msg) => assert_eq!(msg.message, "Need direction"),
        _ => panic!("expected solver agent message"),
    }

    let director_event = expect_agent_event(&mut director_events).await;
    match director_event {
        EventMsg::AgentMessage(msg) => assert_eq!(msg.message, "Proceed iteratively"),
        _ => panic!("expected director agent message"),
    }

    assert_eq!(solver_first.message.message, "Need direction");
    assert_eq!(director_first.message.message, "Proceed iteratively");
    assert_eq!(solver_reply.message.message, "Acknowledged");
    assert_eq!(response_mock.requests().len(), 3);

    Ok(())
}

async fn build_config(server: &MockServer) -> anyhow::Result<Config> {
    let home = TempDir::new()?;
    let cwd = TempDir::new()?;
    let mut config = load_default_config_for_test(&home);
    config.cwd = cwd.path().to_path_buf();
    let mut provider = built_in_model_providers()["openai"].clone();
    provider.base_url = Some(format!("{}/v1", server.uri()));
    config.model_provider = provider;
    Ok(config)
}

async fn expect_message(
    hub: &CrossSessionHub,
    handle: &codex_core::cross_session::TurnHandle,
    expected: &str,
) -> anyhow::Result<AssistantMessage> {
    let message = hub
        .await_first_assistant(handle, Duration::from_secs(1))
        .await?;
    assert_eq!(message.message.message, expected);
    Ok(message)
}

async fn expect_agent_event(stream: &mut SessionEventStream) -> EventMsg {
    loop {
        let maybe_event = match tokio::time::timeout(Duration::from_secs(1), stream.next()).await {
            Ok(event) => event,
            Err(_) => panic!("event timeout"),
        };

        if let Some(event) = maybe_event {
            let msg = event.event.msg;
            if matches!(msg, EventMsg::AgentMessage(_)) {
                return msg;
            }
        } else {
            panic!("stream ended before agent message");
        }
    }
}
