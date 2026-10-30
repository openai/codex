//! New uploaded-audio inputs stay inactive until the runtime integration is installed.

use super::*;
use codex_protocol::models::AudioReference;
use codex_protocol::openai_models::InputModality;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_steered_audio_is_rejected_before_continuation_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let (release, gate) = tokio::sync::oneshot::channel();
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("active"),
                    ev_assistant_message("working", "I am working on your request."),
                ]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: sse(vec![ev_completed("active")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![ev_completed("follow_up")]),
        }],
        vec![StreamingSseChunk {
            gate: None,
            body: sse(vec![ev_completed("resumed")]),
        }],
    ])
    .await;
    let config_server = start_mock_server().await;
    let base_url = format!("{}/v1", streaming.uri());
    let builder = || {
        let base_url = base_url.clone();
        test_codex()
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio);
            })
            .with_config(move |config| {
                config.model_provider.base_url = Some(base_url);
                config.model_provider.supports_websockets = false;
                config
                    .features
                    .disable(Feature::InstantInterrupt)
                    .expect("queue steering");
                config
                    .features
                    .disable(Feature::EnableRequestCompression)
                    .expect("record request bodies");
            })
    };
    let test = builder().build_with_auto_env(&config_server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text("Start working.")]))
        .await?;
    let turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::ItemCompleted(event)
            if matches!(&event.item, TurnItem::AgentMessage(message) if message.id == "working"))
    })
    .await;
    test.codex
        .steer_turn(
            TurnInputRequest::user_input(vec![
                text("Keep this queued text."),
                UserInput::Audio {
                    audio: AudioReference::File {
                        file_id: "file_unsupported_steering".into(),
                    },
                },
            ]),
            turn_id,
        )
        .await?;
    release.send(()).expect("release the active response");
    let completed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.clone()),
        _ => None,
    })
    .await;
    assert!(completed.error.is_some());
    assert_eq!(streaming.requests().await.len(), 1);
    test.submit_text_turn("Continue without audio.").await?;
    let resumed = builder()
        .restart_with_auto_env(&config_server, &test)
        .await?;
    resumed.submit_text_turn("Continue after restart.").await?;
    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<serde_json::Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 3);
    for request in &requests {
        assert!(!request["input"].as_array().unwrap().iter().any(|item| {
            item["content"]
                .as_array()
                .is_some_and(|content| content.iter().any(|part| part["type"] == "input_audio"))
        }));
    }
    for request in &requests[1..] {
        assert!(
            request.to_string().contains("Keep this queued text."),
            "rejecting the attachment must preserve unrelated queued input"
        );
    }
    let entries = requests.iter().map(SnapshotEntry::body).collect::<Vec<_>>();
    insta::assert_snapshot!(
        "unsupported_steered_audio_follow_up_and_resume",
        context_snapshot::format_context_snapshot(
            "Unsupported audio queued during an active turn is never forwarded or persisted; the next turn and resumed thread continue without it.",
            &entries,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unsupported_uploaded_audio_fails_before_inference_without_poisoning_follow_up()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(&server, vec![sse(vec![ev_completed("follow_up")])]).await;
    let test = test_codex()
        .with_model_info_override("gpt-5.5", |model| {
            model.input_modalities.push(InputModality::Audio);
        })
        .build_with_auto_env(&server)
        .await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Listen to this recording."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_unsupported".into(),
                },
            },
        ]))
        .await?;
    let EventMsg::TurnComplete(completed) = wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await
    else {
        unreachable!()
    };
    assert!(completed.error.is_some());
    assert!(
        mock.requests().is_empty(),
        "unsupported audio must not reach inference"
    );
    test.submit_text_turn("Continue without the attachment.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 1);
    assert!(!requests[0].input().iter().any(|item| {
        item["content"]
            .as_array()
            .is_some_and(|parts| parts.iter().any(|part| part["type"] == "input_audio"))
    }));
    assert!(
        serde_json::to_string(&requests[0].input())
            .unwrap()
            .contains("Listen to this recording."),
        "rejecting the attachment must preserve surrounding text"
    );
    Ok(())
}
