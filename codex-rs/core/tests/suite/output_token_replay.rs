//! Replay is OpenAI-only; stored ciphertext survives opt-out and provider changes.

use anyhow::Result;
use codex_core::config::Config;
use codex_features::Feature;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_function_call;
use core_test_support::responses::ev_reasoning_item;
use core_test_support::responses::mount_sse_sequence;
use core_test_support::responses::sse;
use core_test_support::responses::start_mock_server;
use core_test_support::responses::strip_metadata_from_json;
use core_test_support::responses::strip_response_item_ids_from_json;
use core_test_support::skip_if_no_network;
use core_test_support::test_codex::test_codex;
use pretty_assertions::assert_eq;
use serde_json::json;

#[test_case::test_case(false, false; "disabled other provider")]
#[test_case::test_case(false, true; "disabled openai")]
#[test_case::test_case(true, false; "enabled other provider")]
#[test_case::test_case(true, true; "enabled openai")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn output_token_replay_respects_provider_and_preserves_history(
    enabled: bool,
    openai_provider: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut reasoning = ev_reasoning_item("reasoning", &["I will check the result."], &[]);
    reasoning["item"]["content"] = json!(null);
    let mut commentary = ev_assistant_message("commentary", "I will record the completed step.");
    commentary["item"]["phase"] = json!("commentary");
    commentary["item"]["encrypted_content"] = json!("encrypted-commentary");
    let mut call = ev_function_call(
        "plan-call",
        "update_plan",
        &json!({"plan": [{"step": "Check the result", "status": "completed"}]}).to_string(),
    );
    call["item"]["encrypted_content"] = json!("encrypted-call");
    call["item"]["status"] = json!("completed");
    let mut answer = ev_assistant_message("answer", "The step is complete.");
    answer["item"]["encrypted_content"] = json!("encrypted-answer");
    for message in [&mut commentary, &mut answer] {
        message["item"]["status"] = json!("completed");
        message["item"]["content"][0]["annotations"] = json!([]);
        message["item"]["content"][0]["logprobs"] = json!([]);
    }
    commentary["item"]["content"][0]["annotations"] = json!([{
        "type": "url_citation",
        "url": "https://example.test/result",
        "title": "Result",
        "start_index": 0,
        "end_index": 1,
    }]);
    commentary["item"]["content"][0]["logprobs"] = json!([{
        "token": "I", "logprob": -0.25, "bytes": [73], "top_logprobs": [],
    }]);
    let expected_items = [
        reasoning["item"].clone(),
        commentary["item"].clone(),
        call["item"].clone(),
        answer["item"].clone(),
    ];
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                reasoning,
                commentary,
                call,
                ev_completed("plan-response"),
            ]),
            sse(vec![answer, ev_completed("answer-response")]),
            sse(vec![
                ev_assistant_message("resumed", "Still complete."),
                ev_completed("resumed"),
            ]),
        ],
    )
    .await;
    let configure = move |config: &mut Config| {
        config.update_plan_enabled = true;
        config
            .features
            .set_enabled(Feature::OutputTokenReplay, enabled)
            .expect("configure token replay");
        if !openai_provider {
            config.model_provider.name = "mock".to_string();
        }
    };
    let mut builder = test_codex().with_config(configure);
    let test = builder.build_with_auto_env(&server).await?;
    test.submit_text_turn("Record that the result was checked.")
        .await?;

    // Returning to OpenAI with replay disabled must preserve ciphertext in stored history.
    builder = builder.with_config(move |config| {
        configure(config);
        config.model_provider.name = "OpenAI".to_string();
        config
            .features
            .disable(Feature::OutputTokenReplay)
            .expect("disable token replay");
    });
    let resumed = builder.restart_with_auto_env(&server, &test).await?;
    resumed
        .submit_text_turn("Is the step still complete?")
        .await?;

    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    let expected_include = if enabled && openai_provider {
        json!(["reasoning.encrypted_content", "output.encrypted_content"])
    } else {
        json!(["reasoning.encrypted_content"])
    };
    assert_eq!(requests[0].body_json()["include"], expected_include);
    assert_eq!(requests[1].body_json()["include"], expected_include);
    assert_eq!(
        requests[2].body_json()["include"],
        json!(["reasoning.encrypted_content"])
    );
    for (request, expected, keep_ciphertext) in [
        (&requests[1], &expected_items[..3], openai_provider),
        (&requests[2], &expected_items[..], true),
    ] {
        let body = request.body_json();
        let replayed = body["input"]
            .as_array()
            .expect("request input")
            .iter()
            .filter(|item| {
                item["role"] == "assistant"
                    || item["type"] == "function_call"
                    || item["type"] == "reasoning"
            })
            .cloned()
            .collect::<Vec<_>>();
        let mut expected = json!(expected);
        if !keep_ciphertext {
            for item in expected.as_array_mut().expect("expected items") {
                if item["type"] != "reasoning" {
                    item.as_object_mut()
                        .expect("expected item")
                        .remove("encrypted_content");
                }
            }
        }
        assert_eq!(
            strip_metadata_from_json(strip_response_item_ids_from_json(json!(replayed))),
            strip_response_item_ids_from_json(expected),
        );
    }
    Ok(())
}
