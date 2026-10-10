//! Client-marked function outputs survive compaction and a cold resume in their original role.

use super::*;
use codex_protocol::turn_input::AnnotatedResponseItem;
use codex_protocol::turn_input::ResponseItemAnnotations;
use codex_protocol::turn_input::TurnInput;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn retained_tool_output_survives_compaction_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for enabled in [false, true] {
        let server = start_mock_server().await;
        let mock = mount_sse_sequence(&server, vec![
            sse(vec![ev_assistant_message("accepted", "I will investigate the tests."), ev_completed("accepted-response")]),
            sse(vec![ev_assistant_message("details-accepted", "I will use those task details."), ev_completed("details-response")]),
            sse(vec![ev_completed("instruction-response")]),
            sse(vec![json!({"type":"response.output_item.done", "item":{"type":"compaction", "encrypted_content":"RETAINED_TOOL_OUTPUT_CHECKPOINT"}}), ev_completed("compact-response")]),
            sse(vec![ev_assistant_message("resumed", "The request was to investigate the tests."), ev_completed("resumed-response")]),
        ]).await;
        let configure = move |config: &mut Config| {
            configure_scenario_catalog(config);
            config
                .features
                .set_enabled(Feature::RetainClientToolOutputs, enabled)
                .unwrap();
            config.model_context_window = Some(1_000);
            config.model_auto_compact_token_limit = Some(i64::MAX);
            // Let this scenario exercise the compaction budget rather than ingestion truncation.
            config
                .model_catalog
                .as_mut()
                .unwrap()
                .models
                .iter_mut()
                .find(|model| model.slug == "gpt-6-astra")
                .unwrap()
                .truncation_policy = codex_protocol::openai_models::TruncationPolicyConfig::tokens(
                /*limit*/ 100_000,
            );
        };
        let test = test_codex()
            .with_model("gpt-6-astra")
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(configure)
            .build_with_auto_env(&server)
            .await?;
        let details = "Investigate the failing tests carefully. ".repeat(7_000);
        let latest_instruction = "Report your findings without changing files.";
        for (name, output) in [
            (
                "send_message_to_thread",
                "Investigate the failing tests; report your findings without changing files.",
            ),
            ("task_details", details.as_str()),
            ("send_message_to_thread", latest_instruction),
        ] {
            let request = TurnInputRequest::new(TurnInput::AnnotatedResponseItem(
                AnnotatedResponseItem {
                    item: serde_json::from_value(json!({
                        "type": "function_call_output", "name": name, "namespace": "codex_app", "output": output,
                    }))?,
                    annotations: ResponseItemAnnotations { retain: true },
                },
            ));
            test.codex.start_or_steer_turn(request).await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
        }
        test.codex
            .inject_response_items(vec![serde_json::from_value(json!({
                "type": "function_call_output", "name": "progress", "namespace": "example",
                "output": "Transient progress update. ".repeat(1000),
            }))?])
            .await?;
        test.codex.submit(Op::Compact).await?;
        wait_for_event(&test.codex, |event| {
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        let resumed = test_codex()
            .with_model("gpt-6-astra")
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_config(move |config| {
                configure(config);
                config.model_context_window = Some(200_000);
            })
            .restart_with_auto_env(&server, &test)
            .await?;
        resumed
            .submit_text_turn("What request are you working on?")
            .await?;
        let requests = mock.requests();
        assert_eq!(requests.len(), 5);
        for request in &requests[..3] {
            assert!(request.inputs_of_type("compaction_trigger").is_empty());
        }
        let compact_outputs = requests[3].inputs_of_type("function_call_output");
        assert_eq!(compact_outputs.len(), 4);
        // Emergency trimming removes the trailing log, then stops at the marked instruction.
        assert_eq!(compact_outputs[1]["output"], details);
        if enabled {
            assert_eq!(compact_outputs[2]["output"], latest_instruction);
        } else {
            assert_eq!(
                compact_outputs[2]["output"],
                "Output exceeded the available model context and was truncated"
            );
        }
        assert_eq!(
            compact_outputs[3]["output"],
            "Output exceeded the available model context and was truncated"
        );
        let last = requests.last().unwrap();
        assert!(last.inputs_of_type("compaction_trigger").is_empty());
        assert!(last.body_contains_text("What request are you working on?"));
        let outputs = last.inputs_of_type("function_call_output");
        if enabled {
            // Oversized details and older instructions are dropped; the latest stays whole.
            assert_eq!(outputs, vec![compact_outputs[2].clone()]);
            for output in outputs {
                assert!(output.get("retain").is_none());
                assert!(output.get("client_authored").is_none());
            }
        } else {
            assert!(outputs.is_empty());
        }
        insta::assert_snapshot!(
            if enabled {
                "retained_tool_output_survives_compaction_and_resume"
            } else {
                "retained_tool_output_retention_disabled"
            },
            context_snapshot::format_request_history_snapshot(
                if enabled {
                    "Retained thread instructions stop emergency tool-output trimming. Compaction keeps whole outputs within the shared 64K retention budget, dropping oversized task details and older instructions. A cold resume preserves the latest instruction unchanged."
                } else {
                    "With retention disabled, the client can still send retain=true. Tool outputs use ordinary emergency trimming and are not retained after compaction and cold resume."
                },
                &requests,
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
        resumed.codex.shutdown_and_wait().await?;
    }
    Ok(())
}
