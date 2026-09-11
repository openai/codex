//! Delayed notifications retain their producing model's budget in history and rollouts.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn yielded_notifications_keep_originating_budgets_after_model_switch() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let test = step_settings_test()
        .with_config(|config| {
            config.features.enable(Feature::CodeMode).unwrap();
            for model in &mut config.model_catalog.as_mut().expect("models").models {
                model.tool_mode = Some(ToolMode::CodeModeOnly);
                model.use_responses_lite = false;
                model.experimental_supported_tools = vec!["test_sync_tool".to_string()];
                model.truncation_policy =
                    TruncationPolicyConfig::tokens(if model.slug == MODEL_B { 400 } else { 100 });
            }
        })
        .build_with_auto_env(&server)
        .await?;
    let barrier = r#"await tools.test_sync_tool({barrier: {
        id: "notification-origin", participants: 2, timeout_ms: 60000
    }});"#;
    let a_text = "A_NOTIFICATION diagnostic line\n".repeat(500);
    let b_text = "B_NOTIFICATION diagnostic line\n".repeat(500);
    let initial = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-a"),
                ev_custom_tool_call(
                    "call-a",
                    "exec",
                    &format!(
                        "// @exec: {{\"yield_time_ms\": 1}}\n{barrier}\nnotify({});",
                        serde_json::to_string(&a_text)?,
                    ),
                ),
                ev_completed("resp-a"),
            ]),
            sse_completed("resp-a-done"),
        ],
    )
    .await;
    test.submit_text_turn("start A and leave its cell running")
        .await?;
    let initial_requests = initial.requests();
    assert_eq!(
        initial_requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        [json!(MODEL_A), json!(MODEL_A)],
    );
    let (a_output, _) = initial_requests[1]
        .custom_tool_call_output_content_and_success("call-a")
        .expect("A's output");
    let a_output = a_output.expect("yielded output");
    let cell_id = a_output
        .strip_prefix("Script running with cell ID ")
        .and_then(|rest| rest.lines().next())
        .expect("A's running cell id");
    let responses = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_response_created("resp-b"),
                ev_custom_tool_call(
                    "call-b",
                    "exec",
                    &format!(
                        "// @exec: {{\"yield_time_ms\": 60000}}\n{barrier}\nnotify({});",
                        serde_json::to_string(&b_text)?,
                    ),
                ),
                ev_completed("resp-b"),
            ]),
            sse(vec![
                ev_response_created("resp-wait-a"),
                ev_function_call(
                    "call-wait-a",
                    "wait",
                    &json!({
                        "cell_id": cell_id, "yield_time_ms": 60000,
                    })
                    .to_string(),
                ),
                ev_completed("resp-wait-a"),
            ]),
            sse_completed("resp-b-done"),
        ],
    )
    .await;
    // B's cell releases the barrier, so A cannot notify while A is still active.
    test.codex
        .start_or_steer_turn(
            TurnInputRequest::user_input(vec![UserInput::Text {
                text: "run B and release A".to_string(),
                text_elements: Vec::new(),
            }])
            .with_thread_settings(ThreadSettingsOverrides {
                model: Some(MODEL_B.to_string()),
                ..Default::default()
            }),
        )
        .await?;
    let mut raw_notifications = Vec::new();
    wait_for_event(&test.codex, |event| {
        if let EventMsg::RawResponseItem(event) = event {
            let item = serde_json::to_value(&event.item).expect("raw item");
            if is_notification(&item) {
                raw_notifications.push(item);
            }
        }
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = responses.requests();
    assert_eq!(
        requests
            .iter()
            .map(|request| request.body_json()["model"].clone())
            .collect::<Vec<_>>(),
        [json!(MODEL_B), json!(MODEL_B), json!(MODEL_B)],
    );
    assert_eq!(raw_notifications.len(), 2, "both cells must notify");
    let mut expected = Vec::new();
    let mut expected_saved = Vec::new();
    for raw in &raw_notifications {
        let (text, budget) = match raw["call_id"].as_str().expect("call id") {
            "call-a" => (&a_text, 120),
            "call-b" => (&b_text, 480),
            other => panic!("unexpected notification call: {other}"),
        };
        assert_eq!(raw["name"], "exec");
        assert_eq!(
            raw["output"], *text,
            "raw events retain full notification text"
        );
        let mut bounded = raw.clone();
        bounded["output"] = json!(truncate_text(text, TruncationPolicy::Tokens(budget)));
        expected.push(bounded);
        expected_saved.push((raw.clone(), Some(budget)));
    }
    let live = requests[2]
        .inputs_of_type("custom_tool_call_output")
        .into_iter()
        .filter(is_notification)
        .collect::<Vec<_>>();
    assert_eq!(live, expected);
    assert_eq!(
        live.iter()
            .map(|item| item["call_id"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from(["call-a", "call-b"])
    );

    test.codex.shutdown_and_wait().await?;
    let rollout_path = test.codex.rollout_path().expect("rollout path");
    let history = codex_rollout::RolloutRecorder::get_rollout_history(&rollout_path).await?;
    let saved = history
        .get_rollout_items()
        .iter()
        .filter_map(|item| {
            let RolloutItem::ResponseItem(envelope) = item else {
                return None;
            };
            let item = serde_json::to_value(&envelope.item).expect("saved item");
            is_notification(&item).then(|| {
                (
                    item,
                    envelope
                        .metadata
                        .as_ref()
                        .and_then(|metadata| metadata.history_truncation_token_limit),
                )
            })
        })
        .collect::<Vec<_>>();
    assert_eq!(saved, expected_saved);

    Ok(())
}

fn is_notification(item: &Value) -> bool {
    item["type"] == "custom_tool_call_output"
        && item["output"].as_str().is_some_and(|text| {
            text.starts_with("A_NOTIFICATION") || text.starts_with("B_NOTIFICATION")
        })
}
