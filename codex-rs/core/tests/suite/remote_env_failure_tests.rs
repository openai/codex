//! Provisioning failures reach the model without terminating its turn.
use super::*;
use codex_protocol::protocol::TurnEnvironmentRequest;
use codex_protocol::protocol::TurnEnvironmentRequests;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[test_case(true; "failed_before_turn")]
#[test_case(false; "failed_while_waiting")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provisioning_failure_reaches_model_and_turn_continues(
    fail_before_turn: bool,
) -> Result<()> {
    const REASON: &str = "This repository is empty. Push an initial commit, then retry.";
    const CALL_ID: &str = "wait-for-failed-environment";
    let server = start_mock_server().await;
    let response_mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_function_call(
                    CALL_ID,
                    "wait_for_environment",
                    &json!({
                        "environment_id": REMOTE_ENVIRONMENT_ID,
                    })
                    .to_string(),
                ),
                ev_completed("resp-1"),
            ]),
            sse(vec![
                ev_assistant_message(
                    "message-1",
                    "I can explain how to initialize the repository.",
                ),
                ev_completed("resp-2"),
            ]),
        ],
    )
    .await;
    let mut builder = test_codex_with_wait_for_environment().with_config(|config| {
        assert!(config.features.enable(Feature::DeferredExecutor).is_ok());
    });
    let test = expect_startup(builder.build(&server)).await;
    let manager = test.thread_manager.environment_manager();
    let provider = Arc::new(FailingNoiseConnectProvider::default());
    manager.materialize_pending_noise_environment(
        REMOTE_ENVIRONMENT_ID.to_string(),
        provider.clone(),
    )?;
    if fail_before_turn {
        manager.report_environment_provisioning_status(
            REMOTE_ENVIRONMENT_ID.to_string(),
            Err(REASON.to_string()),
            provider.clone(),
        )?;
    }
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "inspect the repository".into(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                environments: Some(TurnEnvironmentRequests::new(
                    test.config.cwd.clone(),
                    vec![TurnEnvironmentRequest {
                        environment_id: REMOTE_ENVIRONMENT_ID.to_string(),
                        cwd: PathUri::from_abs_path(&test.config.cwd),
                        workspace_roots: vec![PathUri::from_abs_path(&test.config.cwd)],
                        config: EnvironmentConfigState::FromThread,
                    }],
                )),
                ..Default::default()
            }),
        )
        .await?;
    wait_for_response_request_count(&response_mock, /*expected_count*/ 1).await;
    if !fail_before_turn {
        manager.report_environment_provisioning_status(
            REMOTE_ENVIRONMENT_ID.to_string(),
            Err(REASON.to_string()),
            provider,
        )?;
    }
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    let (output, _) = requests[1]
        .function_call_output_content_and_success(CALL_ID)
        .context("the model should receive the failed wait result")?;
    assert!(
        output.as_deref().is_some_and(|text| text.contains(REASON)),
        "{output:?}"
    );
    let context_request = if fail_before_turn {
        &requests[0]
    } else {
        &requests[1]
    };
    let context = context_request.message_input_texts("user").join("\n");
    assert!(context.contains("<status>failed</status>"), "{context}");
    assert!(context.contains(REASON), "{context}");
    Ok(())
}
