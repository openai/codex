//! Uploaded audio references remain intact across model requests and user-message history.

use super::super::audio_duration_store::TestAudioDurationStore;
use super::*;
use codex_protocol::models::AudioReference;
use codex_protocol::openai_models::InputModality;
use codex_protocol::turn_input::TurnInputSubmission;
use core_test_support::responses;
use core_test_support::responses::mount_response_sequence;
use pretty_assertions::assert_eq;
use test_case::test_case;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uploaded_audio_survives_follow_up_and_resume() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![
                ev_assistant_message("first", "I heard the recording."),
                ev_completed("first"),
            ]),
            sse(vec![
                ev_assistant_message("second", "The recording contains speech."),
                ev_completed("second"),
            ]),
            sse(vec![
                ev_assistant_message("third", "The same recording is still available."),
                ev_completed("third"),
            ]),
        ],
    )
    .await;
    let builder = || {
        test_codex().with_model_info_override("gpt-5.5", |model_info| {
            model_info.input_modalities.push(InputModality::Audio);
        })
    };
    let test = builder().build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Listen to this recording."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_audio".into(),
                },
            },
        ]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.submit_text_turn("What did you hear?").await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed
        .submit_text_turn("Listen to the same recording again.")
        .await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 3);
    for (index, request) in requests.iter().enumerate() {
        let audio = request
            .input()
            .into_iter()
            .flat_map(|item| item["content"].as_array().cloned().unwrap_or_default())
            .find(|part| part["type"] == "input_audio");
        assert_eq!(
            audio,
            Some(json!({"type": "input_audio", "file_id": "file_audio"})),
            "request {index}",
        );
    }
    insta::assert_snapshot!(
        "uploaded_audio_survives_follow_up",
        context_snapshot::format_request_history_snapshot(
            "An uploaded audio file ID is forwarded to Responses and retained across follow-up and resume.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum AudioAtCompaction {
    Available,
    Measured,
    MissingAtStore,
    MissingAtResponses,
}

#[test_case(AudioAtCompaction::Available, &[true, true, true]; "available")]
#[test_case(AudioAtCompaction::Measured, &[true, true, true]; "measured duration")]
#[test_case(AudioAtCompaction::MissingAtStore, &[true, false, false]; "expired before compaction")]
#[test_case(AudioAtCompaction::MissingAtResponses, &[true, true, false, false]; "expired during compaction")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn uploaded_audio_survives_compaction_and_cold_resume(
    availability: AudioAtCompaction,
    expected_audio: &[bool],
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let mut replies = vec![responses::sse_response(sse(vec![ev_completed("initial")]))];
    if availability == AudioAtCompaction::MissingAtResponses {
        replies.push(wiremock::ResponseTemplate::new(404).set_body_json(json!({
            "error": {"message": "Invalid input: Unknown `file_id` in input content."}
        })));
    }
    replies.extend([
        responses::sse_response(sse(vec![
            json!({
                "type": "response.output_item.done",
                "item": {
                    "type": "compaction",
                    "encrypted_content": "UPLOADED_AUDIO_COMPACTION_CHECKPOINT"
                }
            }),
            ev_completed("compacted"),
        ])),
        responses::sse_response(sse(vec![ev_completed("resumed")])),
    ]);
    let responses = mount_response_sequence(&server, replies).await;
    let store = Arc::new(RecordingFileAttachmentStore::default());
    let measured = availability == AudioAtCompaction::Measured;
    let builder = || {
        let builder = test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio);
            });
        if measured {
            builder.with_image_store(Arc::new(TestAudioDurationStore {
                measure_short_clips: true,
                ..Default::default()
            }))
        } else {
            builder.with_image_store(store.clone())
        }
    };
    let test = builder().build_with_auto_env(&server).await?;
    let ids: Vec<String> = if measured {
        (1..=7).map(|n| format!("file_audio_{n}")).collect()
    } else {
        vec!["file_audio_compacted".into()]
    };
    let mut input = vec![text("Retain this recording through compaction.")];
    input.extend(ids.iter().map(|id| UserInput::Audio {
        audio: AudioReference::File {
            file_id: id.clone(),
        },
    }));
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(input))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if availability == AudioAtCompaction::MissingAtStore {
        store
            .resolve_errors
            .lock()
            .expect("attachment store errors")
            .insert(
                "file_audio_compacted".to_string(),
                AttachmentStoreErrorKind::NotFound,
            );
    }
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    let mut input = vec![text("Use the retained recording after cold resume.")];
    if matches!(
        availability,
        AudioAtCompaction::MissingAtStore | AudioAtCompaction::MissingAtResponses
    ) {
        // Reintroduce the ID to exercise the checkpoint's rejection record.
        input.push(UserInput::Audio {
            audio: AudioReference::File {
                file_id: "file_audio_compacted".into(),
            },
        });
    }
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(input))
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;

    let requests = responses.requests();
    assert_eq!(requests.len(), expected_audio.len());
    let expected: Vec<_> = ids
        .iter()
        .map(|id| json!({"type":"input_audio","file_id":id}))
        .collect();
    for (request, expect_audio) in requests.iter().zip(expected_audio) {
        let audio: Vec<_> = request
            .input()
            .into_iter()
            .flat_map(|item| item["content"].as_array().cloned().unwrap_or_default())
            .filter(|part| part["type"] == "input_audio")
            .collect();
        assert_eq!(
            audio.as_slice(),
            if *expect_audio {
                expected.as_slice()
            } else {
                &[]
            }
        );
        assert!(
            request
                .body_json()
                .to_string()
                .contains("Retain this recording through compaction.")
        );
    }
    assert_eq!(
        requests
            .last()
            .expect("resumed request")
            .inputs_of_type("compaction")[0]["encrypted_content"],
        "UPLOADED_AUDIO_COMPACTION_CHECKPOINT"
    );
    let (snapshot, description) = match availability {
        AudioAtCompaction::Available => (
            "uploaded_audio_compaction_and_cold_resume",
            "The uploaded audio file ID survives remote compaction and a cold resume in canonical message content.",
        ),
        AudioAtCompaction::Measured => (
            "measured_audio_compaction_and_cold_resume",
            "Two measured clips and five unmeasured files fit in the retained budget. All seven audio file IDs survive compaction and cold resume.",
        ),
        AudioAtCompaction::MissingAtStore | AudioAtCompaction::MissingAtResponses => return Ok(()),
    };
    insta::assert_snapshot!(
        snapshot,
        context_snapshot::format_request_history_snapshot(
            description,
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expired_audio_during_required_compaction_after_a_text_only_checkpoint() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let requests = responses::mount_response_handler(&server, |request| {
        let input = request.body_json()["input"].to_string();
        let has_file = input.contains("file_expired");
        if !request.inputs_of_type("compaction_trigger").is_empty() {
            if has_file {
                return wiremock::ResponseTemplate::new(404).set_body_json(json!({
                    "error": {"message": "Invalid input: Unknown `file_id` in input content."}
                }));
            }
            let checkpoint = if input.contains("FIRST_CHECKPOINT") {
                "SECOND_CHECKPOINT"
            } else {
                "FIRST_CHECKPOINT"
            };
            return responses::sse_response(sse(vec![
                json!({"type":"response.output_item.done","item":{
                    "type":"compaction","encrypted_content":checkpoint
                }}),
                ev_completed("compacted"),
            ]));
        }
        let tokens = if has_file { 500_000 } else { 80 };
        responses::sse_response(sse(vec![
            responses::ev_assistant_message("message", "I can continue."),
            responses::ev_completed_with_tokens("answer", tokens),
        ]))
    })
    .await;
    let test = test_codex()
        .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
        .with_config(|config| config.model_auto_compact_token_limit = Some(200_000))
        .with_model_info_override("gpt-5.5", |model| {
            model.input_modalities.push(InputModality::Audio)
        })
        .build_with_auto_env(&server)
        .await?;
    test.submit_text_turn("Begin with text only.").await?;
    test.codex.submit(Op::Compact).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Listen to the recording."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_expired".into(),
                },
            },
        ]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    for prompt in ["Continue after the file expired.", "Continue again."] {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![text(prompt)]))
            .await?;
        let done = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnComplete(done) => Some(done.clone()),
            _ => None,
        })
        .await;
        assert!(done.error.is_none(), "{done:?}");
        assert_eq!(done.last_agent_message.as_deref(), Some("I can continue."));
    }
    let requests = requests.requests();
    let compactions: Vec<_> = requests
        .iter()
        .filter(|r| !r.inputs_of_type("compaction_trigger").is_empty())
        .collect();
    assert_eq!(
        compactions.len(),
        3,
        "initial compaction, missing-file rejection, and single retry"
    );
    for (r, contains_file) in compactions.iter().skip(1).zip([true, false]) {
        let input = r.body_json()["input"].to_string();
        assert!(input.contains("FIRST_CHECKPOINT"));
        assert_eq!(input.contains("file_expired"), contains_file);
    }
    for r in requests.iter().rev().take(2) {
        assert!(r.inputs_of_type("compaction_trigger").is_empty());
        let input = r.body_json()["input"].to_string();
        assert!(input.contains("SECOND_CHECKPOINT"));
        assert!(!input.contains("file_expired"));
    }
    Ok(())
}

#[test_case(400, Some("audio_too_large"), "The audio input is too long."; "invalid audio")]
#[test_case(404, None, "Invalid input: Unknown `file_id` in input content."; "unknown file")]
#[test_case(200, Some("audio_too_large"), "The audio input is too long."; "streamed invalid audio")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejected_uploaded_audio_does_not_poison_follow_up_or_resume(
    status: u16,
    code: Option<&str>,
    message: &str,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let error = json!({ "type": "invalid_request_error", "code": code, "message": message });
    let rejection = if status == 200 {
        wiremock::ResponseTemplate::new(status)
            .insert_header("content-type", "text/event-stream")
            .set_body_string(sse(vec![json!({
                "type": "response.failed", "response": {"error": error}
            })]))
    } else {
        wiremock::ResponseTemplate::new(status).set_body_json(json!({"error": error}))
    };
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/responses"))
        .respond_with(rejection)
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    let builder = || {
        test_codex()
            .with_config(|config| config.model_provider.stream_max_retries = Some(0))
            .with_model_info_override("gpt-5.5", |model_info| {
                model_info.input_modalities.push(InputModality::Audio);
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Listen to this recording."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_rejected".into(),
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
    let error = completed.error.as_ref().expect("rejected turn error");
    assert!(
        error
            .message
            .contains("remove or replace the affected audio attachment")
    );
    assert_eq!(
        error.rejected_audio_file_ids,
        vec!["file_rejected".to_string()],
    );
    let mock = mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("follow_up")]),
            sse(vec![ev_completed("resumed")]),
        ],
    )
    .await;
    test.submit_text_turn("Continue without the recording.")
        .await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed.submit_text_turn("Continue after restart.").await?;
    let requests = mock.requests();
    assert_eq!(requests.len(), 2);
    for request in &requests {
        let input = request.input();
        let projected = input
            .iter()
            .find(|item| item["content"][0]["text"] == "Listen to this recording.")
            .expect("the rejected turn's user message remains in history");
        assert_eq!(
            (
                &projected["content"],
                &projected["internal_chat_message_metadata_passthrough"]["content_item_kinds"],
            ),
            (
                &json!([{"type": "input_text", "text": "Listen to this recording."}]),
                &json!(["user.text"]),
            ),
        );
        let content = input
            .into_iter()
            .flat_map(|item| item["content"].as_array().cloned().unwrap_or_default())
            .collect::<Vec<_>>();
        assert!(
            content.iter().all(|part| part["type"] != "input_audio"),
            "failed turn error: {:?}",
            completed.error
        );
    }
    insta::assert_snapshot!(
        "rejected_uploaded_audio_follow_up_and_resume",
        context_snapshot::format_request_history_snapshot(
            "A rejected uploaded recording is omitted only from request projections on follow-up and after resume, preserving surrounding text and aligned annotations without inventing user content.",
            &requests,
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn rejection_is_attributed_against_projection_after_unavailable_image() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let image_store = Arc::new(RecordingFileAttachmentStore::default());
    let responses = mount_response_sequence(
        &server,
        vec![
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![ev_completed("image_turn")])),
            wiremock::ResponseTemplate::new(404).set_body_json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": null,
                    "message": "Invalid input: Unknown `file_id` in input content."
                }
            })),
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![ev_completed("follow_up")])),
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![ev_completed("resumed")])),
        ],
    )
    .await;
    let builder = || {
        test_codex()
            .with_image_store(image_store.clone())
            .with_config(|config| config.model_provider.stream_max_retries = Some(0))
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio);
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Remember this historical image."),
            UserInput::Image {
                image: ImageReference::File {
                    file_id: "file_image_unavailable_later".to_string(),
                },
                detail: None,
            },
        ]))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    image_store
        .resolve_errors
        .lock()
        .expect("resolve errors lock")
        .insert(
            "file_image_unavailable_later".to_string(),
            AttachmentStoreErrorKind::NotFound,
        );

    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Check this recording."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_audio_rejected_after_projection".to_string(),
                },
            },
        ]))
        .await?;
    let rejected = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.clone()),
        _ => None,
    })
    .await;
    assert!(rejected.error.is_some());
    test.submit_text_turn("Continue without unavailable files.")
        .await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed.submit_text_turn("Continue after restart.").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 4);
    assert!(
        requests[0]
            .body_json()
            .to_string()
            .contains("file_image_unavailable_later")
    );
    let submitted = requests[1].body_json().to_string();
    assert!(submitted.contains("[image unavailable]"));
    assert!(!submitted.contains("file_image_unavailable_later"));
    assert!(submitted.contains("file_audio_rejected_after_projection"));
    for request in &requests[2..] {
        let body = request.body_json().to_string();
        assert!(body.contains("[image unavailable]"));
        assert!(!body.contains("file_image_unavailable_later"));
        assert!(!body.contains("file_audio_rejected_after_projection"));
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn ambiguous_multi_audio_rejection_preserves_all_audio_on_follow_up_and_resume() -> Result<()>
{
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let responses = mount_response_sequence(
        &server,
        vec![
            wiremock::ResponseTemplate::new(400).set_body_json(json!({
                "error": {
                    "type": "invalid_request_error",
                    "code": "invalid_audio",
                    "message": "one recording is invalid"
                }
            })),
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![ev_completed("follow_up")])),
            wiremock::ResponseTemplate::new(200)
                .insert_header("content-type", "text/event-stream")
                .set_body_string(sse(vec![ev_completed("resumed")])),
        ],
    )
    .await;
    let builder = || {
        test_codex()
            .with_config(|config| config.model_provider.stream_max_retries = Some(0))
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio);
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Compare both recordings."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_ambiguous_a".to_string(),
                },
            },
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_ambiguous_b".to_string(),
                },
            },
        ]))
        .await?;
    let rejected = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.clone()),
        _ => None,
    })
    .await;
    let error = rejected.error.expect("ambiguous audio rejection");
    assert!(
        error
            .message
            .contains("remove or replace the affected audio attachment")
    );
    assert!(error.rejected_audio_file_ids.is_empty());
    test.submit_text_turn("Retry with the retained recordings.")
        .await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed.submit_text_turn("Retry after restart.").await?;

    let requests = responses.requests();
    assert_eq!(requests.len(), 3);
    for request in requests {
        let body = request.body_json().to_string();
        assert!(body.contains("file_ambiguous_a"));
        assert!(body.contains("file_ambiguous_b"));
    }
    Ok(())
}

#[test_case(AttachmentStoreErrorKind::NotFound, true; "store confirms B expired")]
#[test_case(AttachmentStoreErrorKind::Backend, false; "transient storage error preserves B")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unknown_file_after_two_recordings_were_accepted(
    storage_error: AttachmentStoreErrorKind,
    confirmed: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let expired = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let reject = Arc::clone(&expired);
    let requests = responses::mount_response_handler(&server, move |r| {
        if reject.load(std::sync::atomic::Ordering::Relaxed)
            && r.body_json()["input"].to_string().contains("file_b")
        {
            return wiremock::ResponseTemplate::new(404).set_body_json(json!({
                "error": {"message": "Invalid input: Unknown `file_id` in input content."}
            }));
        }
        responses::sse_response(sse(vec![
            ev_assistant_message("answer", "I can continue."),
            ev_completed("completed"),
        ]))
    })
    .await;
    let store = Arc::new(RecordingFileAttachmentStore::default());
    let builder = || {
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_image_store(store.clone())
            .with_config(|config| config.model_provider.stream_max_retries = Some(0))
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio)
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    for id in ["file_a", "file_b"] {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Audio {
                audio: AudioReference::File { file_id: id.into() },
            }]))
            .await?;
        let done = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnComplete(done) => Some(done.clone()),
            _ => None,
        })
        .await;
        assert!(done.error.is_none(), "{done:?}");
    }
    // Each new recording gets one duration lookup. Recovery probes start after the 404.
    {
        let resolves = store.resolves.lock().expect("store lookups");
        assert_eq!(resolves.get("file_a"), Some(&1));
        assert_eq!(resolves.get("file_b"), Some(&1));
    }
    store
        .resolve_errors
        .lock()
        .expect("store")
        .insert("file_b".into(), storage_error);
    expired.store(true, std::sync::atomic::Ordering::Relaxed);
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Recall the recordings.",
        )]))
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(done) => done.error.clone(),
        _ => None,
    })
    .await;
    {
        let resolves = store
            .resolves
            .lock()
            .expect("store lookups after rejection");
        assert_eq!(resolves.get("file_a"), Some(&2));
        assert_eq!(resolves.get("file_b"), Some(&2));
    }
    assert_eq!(
        error.rejected_audio_file_ids,
        if confirmed {
            vec!["file_b"]
        } else {
            Vec::new()
        }
    );
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text(
            "Continue in this chat.",
        )]))
        .await?;
    let follow_up = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(done) => Some(done.clone()),
        _ => None,
    })
    .await;
    assert_eq!(follow_up.error.is_none(), confirmed);
    if confirmed {
        assert_eq!(
            follow_up.last_agent_message.as_deref(),
            Some("I can continue.")
        );
    }
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed
        .submit_text_turn("Continue after reopening.")
        .await?;
    let all = requests.requests();
    assert_eq!(all.len(), 5);
    for request in &all[3..] {
        let input = request.body_json()["input"].to_string();
        assert!(input.contains("file_a"));
        assert_eq!(input.contains("file_b"), !confirmed);
    }
    Ok(())
}

#[test_case(false, false; "accepted recording in a previous turn")]
#[test_case(false, true; "accepted recording before cold resume")]
#[test_case(true, false; "first recording after opaque text-only compaction")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn persistently_invalid_recording_is_removed_on_follow_up_and_resume(
    compact_text: bool,
    resume_before_rejection: bool,
) -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let request_count = std::sync::atomic::AtomicUsize::new(0);
    let requests = responses::mount_response_handler(&server, move |request| {
        let index = request_count.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let body = request.body_json();
        let input = body["input"].to_string();
        if input.contains("file_bad_b") {
            return wiremock::ResponseTemplate::new(400).set_body_json(json!({
                "error": {"code": "audio_too_short", "message": "The recording is too short."}
            }));
        }
        if compact_text && index == 1 {
            return responses::sse_response(sse(vec![
                json!({"type": "response.output_item.done", "item": {
                    "type": "compaction", "encrypted_content": "TEXT_ONLY_CHECKPOINT"
                }}),
                ev_completed("compacted"),
            ]));
        }
        responses::sse_response(sse(vec![ev_completed("accepted")]))
    })
    .await;
    let builder = || {
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio)
            })
    };
    let recording = |id: &str| UserInput::Audio {
        audio: AudioReference::File { file_id: id.into() },
    };
    let test = builder().build_with_auto_env(&server).await?;
    let mut initial = vec![text("Start this conversation.")];
    if !compact_text {
        initial.push(recording("file_good_a"));
    }
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(initial))
        .await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    if compact_text {
        test.codex.submit(Op::Compact).await?;
        let compacted = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnComplete(done) => Some(done.clone()),
            _ => None,
        })
        .await;
        assert!(compacted.error.is_none(), "{compacted:?}");
    }
    let test = if resume_before_rejection {
        builder().restart_with_auto_env(&server, &test).await?
    } else {
        test
    };
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Check this new recording."),
            recording("file_bad_b"),
        ]))
        .await?;
    let error = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(done) => done.error.clone(),
        _ => None,
    })
    .await;
    assert_eq!(error.rejected_audio_file_ids, ["file_bad_b"]);
    test.submit_text_turn("Continue without the rejected upload.")
        .await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed
        .submit_text_turn("Continue after reopening.")
        .await?;
    let requests = requests.requests();
    let rejected = requests.len() - 3;
    assert!(
        requests[rejected].body_json()["input"]
            .to_string()
            .contains("file_bad_b")
    );
    for request in &requests[rejected..] {
        let input = request.body_json()["input"].to_string();
        assert!(input.contains(if compact_text {
            "TEXT_ONLY_CHECKPOINT"
        } else {
            "file_good_a"
        }));
    }
    for request in &requests[rejected + 1..] {
        assert!(
            !request.body_json()["input"]
                .to_string()
                .contains("file_bad_b")
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn paginated_resume_filters_audio_rejected_before_and_after_compaction() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let rejection = wiremock::ResponseTemplate::new(400).set_body_json(json!({
        "error": {"type": "invalid_request_error", "code": "invalid_audio", "message": "bad recording"}
    }));
    let requests = mount_response_sequence(
        &server,
        vec![
            rejection.clone(),
            responses::sse_response(sse(vec![
                json!({"type": "response.output_item.done", "item": {
                    "type": "compaction", "encrypted_content": "PAGINATED_AUDIO_CHECKPOINT"
                }}),
                ev_completed("compacted"),
            ])),
            rejection,
            responses::sse_response(sse(vec![ev_completed("resumed")])),
        ],
    )
    .await;
    let builder = || {
        test_codex()
            .with_auth(CodexAuth::create_dummy_chatgpt_auth_for_testing())
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio)
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    let recording = |id: &str| UserInput::Audio {
        audio: AudioReference::File { file_id: id.into() },
    };
    for id in ["file_A", "file_B"] {
        test.codex
            .start_or_steer_turn(TurnInputRequest::user_input(vec![
                text("Check the recording."),
                recording(id),
            ]))
            .await?;
        let error = wait_for_event_match(&test.codex, |event| match event {
            EventMsg::TurnComplete(completed) => completed.error.clone(),
            _ => None,
        })
        .await;
        assert_eq!(error.rejected_audio_file_ids, [id]);
        if id == "file_A" {
            test.codex.submit(Op::Compact).await?;
            wait_for_event(&test.codex, |event| {
                matches!(event, EventMsg::TurnComplete(_))
            })
            .await;
        }
    }
    test.codex.shutdown_and_wait().await?;
    let path = test
        .session_configured
        .rollout_path
        .clone()
        .expect("rollout path");
    // The bounded CCA page contains the session header, checkpoint and suffix.
    // Remove A's failed turn entirely, so only the checkpoint can remember its ID.
    let mut page = String::new();
    let mut at_checkpoint = false;
    for line in fs::read_to_string(&path)?.lines() {
        let record = codex_rollout::parse_rollout_line(line)?;
        if let RolloutItem::Compacted(compacted) = &record.item {
            assert_eq!(
                compacted
                    .resume_metadata
                    .as_ref()
                    .expect("resume metadata")
                    .rejected_audio_file_ids
                    .as_ref(),
                &std::collections::BTreeSet::from(["file_A".to_string()])
            );
            at_checkpoint = true;
        }
        if at_checkpoint || matches!(record.item, RolloutItem::SessionMeta(_)) {
            page.push_str(line);
            page.push('\n');
        }
    }
    assert!(at_checkpoint);
    fs::write(&path, page)?;
    let resumed = builder().resume(&server, test.home.clone(), path).await?;
    resumed
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Check these recordings again."),
            recording("file_A"),
            recording("file_B"),
            recording("file_C"),
        ]))
        .await?;
    wait_for_event(&resumed.codex, |event| {
        matches!(event, EventMsg::TurnComplete(_))
    })
    .await;
    let requests = requests.requests();
    assert_eq!(requests.len(), 4);
    let body = requests[3].body_json().to_string();
    assert!(body.contains("PAGINATED_AUDIO_CHECKPOINT"));
    assert!(body.contains("file_C"));
    assert!(!body.contains("file_A"));
    assert!(!body.contains("file_B"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn instant_interrupt_preserves_delayed_http_audio_rejection() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = start_mock_server().await;
    let builder = || {
        test_codex()
            .with_config(|config| {
                config
                    .features
                    .enable(Feature::InstantInterrupt)
                    .expect("enable InstantInterrupt");
                config.model_provider.supports_websockets = false;
            })
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio)
            })
    };
    let test = builder().build_with_auto_env(&server).await?;
    let received = Arc::new(tokio::sync::Notify::new());
    let started = Arc::clone(&received);
    let (release, gate) = std::sync::mpsc::channel();
    let gate = std::sync::Mutex::new(gate);
    let rejection = wiremock::ResponseTemplate::new(400).set_body_json(json!({
        "error": {
            "type": "invalid_request_error",
            "code": "invalid_audio",
            "message": "bad recording"
        }
    }));
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/v1/responses"))
        .respond_with(move |_: &wiremock::Request| {
            started.notify_one();
            gate.lock()
                .expect("HTTP response gate")
                .recv_timeout(Duration::from_secs(15))
                .expect("steer B before rejecting A");
            rejection.clone()
        })
        .up_to_n_times(1)
        .expect(1)
        .mount(&server)
        .await;
    let recording = |id: &str| UserInput::Audio {
        audio: AudioReference::File { file_id: id.into() },
    };
    test.codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![recording("file_A")]))
        .await?;
    let turn_id = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnStarted(event) => Some(event.turn_id.clone()),
        _ => None,
    })
    .await;
    tokio::time::timeout(Duration::from_secs(15), received.notified()).await?;
    test.codex
        .steer_turn(
            TurnInputRequest::user_input(vec![recording("file_B")]),
            turn_id,
        )
        .await?;
    release.send(())?;
    let completed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        completed
            .error
            .expect("terminal rejection of A")
            .rejected_audio_file_ids,
        ["file_A"]
    );
    mount_sse_sequence(
        &server,
        vec![
            sse(vec![ev_completed("live")]),
            sse(vec![ev_completed("resumed")]),
        ],
    )
    .await;
    test.submit_text_turn("Continue with the replacement.")
        .await?;
    let resumed = builder().restart_with_auto_env(&server, &test).await?;
    resumed.submit_text_turn("Use the same recording.").await?;
    let requests = responses::received_responses_requests(&server).await;
    assert_eq!(requests.len(), 3);
    for (request, expected_id) in requests.iter().zip(["file_A", "file_B", "file_B"]) {
        let parts: Vec<_> = request
            .input()
            .iter()
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter(|part| part["type"] == "input_audio")
            .cloned()
            .collect();
        assert_eq!(
            parts,
            [json!({"type": "input_audio", "file_id": expected_id})]
        );
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn provider_rejection_preserves_a_different_queued_audio_file() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let (release, gate) = tokio::sync::oneshot::channel();
    let failure = json!({
        "type": "response.failed",
        "response": {
            "error": {
                "type": "invalid_request_error",
                "code": "invalid_audio",
                "message": "invalid recording"
            }
        }
    });
    let (streaming, _completions) = start_streaming_sse_server(vec![
        vec![
            StreamingSseChunk {
                gate: None,
                body: sse(vec![
                    ev_response_created("active"),
                    ev_assistant_message("working", "I am checking the recording."),
                ]),
            },
            StreamingSseChunk {
                gate: Some(gate),
                body: sse(vec![failure]),
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
                config.model_provider.stream_max_retries = Some(0);
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
        .start_or_steer_turn(TurnInputRequest::user_input(vec![
            text("Check submitted recording A."),
            UserInput::Audio {
                audio: AudioReference::File {
                    file_id: "file_a_rejected".into(),
                },
            },
        ]))
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
                text("Keep queued recording B."),
                UserInput::Audio {
                    audio: AudioReference::File {
                        file_id: "file_b_queued".into(),
                    },
                },
            ]),
            turn_id,
        )
        .await?;
    release.send(()).expect("release rejection");
    let completed = wait_for_event_match(&test.codex, |event| match event {
        EventMsg::TurnComplete(completed) => Some(completed.clone()),
        _ => None,
    })
    .await;
    assert_eq!(
        completed
            .error
            .as_ref()
            .map(|error| error.rejected_audio_file_ids.clone()),
        Some(vec!["file_a_rejected".to_string()])
    );

    test.submit_text_turn("Continue with queued recording B.")
        .await?;
    let resumed = builder()
        .restart_with_auto_env(&config_server, &test)
        .await?;
    resumed
        .submit_text_turn("Continue after restart with recording B.")
        .await?;

    let requests = streaming
        .requests()
        .await
        .iter()
        .map(|body| serde_json::from_slice::<serde_json::Value>(body))
        .collect::<serde_json::Result<Vec<_>>>()?;
    assert_eq!(requests.len(), 3);
    let audio_ids = |request: &serde_json::Value| {
        request["input"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .filter_map(|part| part["file_id"].as_str().map(str::to_string))
            .collect::<Vec<_>>()
    };
    assert_eq!(audio_ids(&requests[0]), vec!["file_a_rejected".to_string()]);
    for request in &requests[1..] {
        assert_eq!(audio_ids(request), vec!["file_b_queued".to_string()]);
    }
    Ok(())
}

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
                model
                    .input_modalities
                    .retain(|modality| *modality != InputModality::Audio);
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
            model
                .input_modalities
                .retain(|modality| *modality != InputModality::Audio);
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn interrupt_during_audio_lookup_preserves_steered_batch_on_follow_up_and_resume()
-> Result<()> {
    skip_if_no_network!(Ok(()));
    let store = Arc::new(TestAudioDurationStore::default());
    let (release, gate) = tokio::sync::oneshot::channel();
    let (streaming, _) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: Some(gate),
            body: sse(vec![ev_completed("active")]),
        }],
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
    let builder = || {
        let base_url = format!("{}/v1", streaming.uri());
        test_codex()
            .with_image_store(store.clone())
            .with_model_info_override("gpt-5.5", |model| {
                model.input_modalities.push(InputModality::Audio)
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
                    .expect("inspect request bodies");
            })
    };
    let test = builder().build_with_auto_env(&config_server).await?;
    let TurnInputSubmission::Started { turn_id, .. } = test
        .codex
        .start_or_steer_turn(TurnInputRequest::user_input(vec![text("Start working.")]))
        .await?
    else {
        anyhow::bail!("expected an active turn");
    };
    let timeout = Duration::from_secs(10);
    tokio::time::timeout(timeout, streaming.wait_for_request_count(/*count*/ 1)).await?;
    test.codex
        .steer_turn(
            TurnInputRequest::user_input(vec![
                text("Keep this recording."),
                UserInput::Audio {
                    audio: AudioReference::File {
                        file_id: "file_audio_5".into(),
                    },
                },
            ]),
            turn_id.clone(),
        )
        .await?;
    test.codex
        .steer_turn(
            TurnInputRequest::user_input(vec![text("Keep the rest of the batch.")]),
            turn_id,
        )
        .await?;
    release.send(()).expect("release active response");
    tokio::time::timeout(timeout, store.slow_lookup_started.notified()).await?;
    test.codex.submit(Op::Interrupt).await?;
    wait_for_event(&test.codex, |event| {
        matches!(event, EventMsg::TurnAborted(_))
    })
    .await;

    test.submit_text_turn("Follow up.").await?;
    let resumed = builder()
        .restart_with_auto_env(&config_server, &test)
        .await?;
    resumed.submit_text_turn("Continue after restart.").await?;
    let requests = streaming.requests().await;
    assert_eq!(requests.len(), 3);
    for body in &requests[1..] {
        let request: serde_json::Value = serde_json::from_slice(body)?;
        let parts = request["input"]
            .as_array()
            .expect("request input")
            .iter()
            .filter(|item| item["role"] == "user")
            .flat_map(|item| item["content"].as_array().into_iter().flatten())
            .collect::<Vec<_>>();
        for expected in [
            json!({"type":"input_text", "text":"Keep this recording."}),
            json!({"type":"input_audio", "file_id":"file_audio_5"}),
            json!({"type":"input_text", "text":"Keep the rest of the batch."}),
        ] {
            assert!(
                parts.contains(&&expected),
                "missing {expected} on follow-up or resume"
            );
        }
    }
    Ok(())
}
