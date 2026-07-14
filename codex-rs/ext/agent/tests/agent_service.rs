use anyhow::Result;
use codex_agent_extension::AgentInvocation;
use codex_agent_extension::AgentRunner;
use codex_protocol::protocol::EventMsg;
use core_test_support::responses;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use core_test_support::wait_for_event;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn starts_resolved_agent_prompt_in_forked_thread() -> Result<()> {
    skip_if_no_network!(Ok(()));

    let server = responses::start_mock_server().await;
    let response_mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_response_created("agent-response"),
            responses::ev_completed("agent-response"),
        ]),
    )
    .await;
    let test = test_codex().build_with_auto_env(&server).await?;
    let parent_thread_id = test.session_configured.session_id.into();
    let agent_runner = AgentRunner::new(std::sync::Arc::downgrade(&test.thread_manager));

    let agent_run = agent_runner
        .start(
            parent_thread_id,
            AgentInvocation {
                config: test.config.clone(),
                prompt: "Use $example-agent to inspect the current changes.".to_string(),
                parent_trace: None,
            },
        )
        .await?;

    assert_ne!(agent_run.thread_id, parent_thread_id);
    assert_eq!(
        agent_run
            .thread
            .config_snapshot()
            .await
            .forked_from_thread_id,
        Some(parent_thread_id)
    );
    let started = wait_for_event(&agent_run.thread, |event| {
        matches!(event, EventMsg::TurnStarted(_))
    })
    .await;
    let EventMsg::TurnStarted(started) = started else {
        unreachable!("event predicate only matches turn started events");
    };
    assert_eq!(started.turn_id, agent_run.turn_id);
    wait_for_event(&agent_run.thread, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let request = response_mock.single_request();
    assert!(
        request
            .message_input_texts("user")
            .iter()
            .any(|text| text == "Use $example-agent to inspect the current changes.")
    );

    Ok(())
}
