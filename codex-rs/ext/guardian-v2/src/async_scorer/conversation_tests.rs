use super::*;
use crate::async_scorer::authorization::ScoreAuthorization;
use crate::async_scorer::conversation::ConversationBackend;
use crate::async_scorer::conversation::ConversationRequest;
use crate::async_scorer::sampler::LunaSampler;
use crate::async_scorer::sampler::LunaSamplerError;
use crate::async_scorer::sampler::LunaSamplingRequest;
use crate::async_scorer::sampler::tests::sample_request;
use crate::async_scorer::sampler::tests::sampler_config;
use crate::async_scorer::transcript::ContextInput;
use codex_extension_api::ThreadStopInput;
use codex_guardian_context::ContextTarget;
use core_test_support::context_snapshot::ContextSnapshotOptions;
use core_test_support::context_snapshot::SnapshotEntry;
use core_test_support::context_snapshot::format_context_snapshot;
use core_test_support::streaming_sse::StreamingSseChunk;
use core_test_support::streaming_sse::start_streaming_sse_server;
use pretty_assertions::assert_eq;

async fn pending_request(
    fixture: &GuardianFailureFixture,
    label: &str,
) -> Result<(
    ConversationRequest,
    tokio::sync::oneshot::Receiver<Result<String, LunaSamplerError>>,
)> {
    let thread = Arc::clone(&fixture.test.codex);
    let config = thread
        .thread_extension_data()
        .get::<GuardianV2Config>()
        .unwrap();
    let history = thread.conversation_history_snapshot().await;
    let instructions = config.render_classifier_instructions(TEST_GUARDIAN_POLICY, "");
    let evidence = config.transcript.collect_context(ContextInput {
        target: ContextTarget::Async,
        history: history.as_ref(),
        root_conversation: &[],
        trusted_user_answers: &[],
        planned_action: None,
        permissions: None,
        previous_reviews: None,
        trusted_tool: None,
        trusted_skill_paths: &[],
        node_repl_images: None,
    })?;
    let (ready, score) = tokio::sync::oneshot::channel();
    let authorization = ScoreAuthorization::current(&thread, &Default::default()).await;
    Ok((
        ConversationRequest {
            evidence,
            reset_token_limit: config.async_classifier_conversation_token_limit,
            authorization,
            thread,
            ready,
            metrics: None,
            sampling: LunaSamplingRequest {
                instructions,
                input: Vec::new(),
                ..sample_request(label)
            },
        },
        score,
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_reset_limit_rebuilds_history_without_rejecting_fresh_evidence() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (limit, reused) in [(1, false), (100_000, true)] {
        let server = responses::start_mock_server().await;
        let fixture = GuardianFailureFixture::with_config(&format!(
            "[features.guardianv2]\nasync_classifier_mode = 'conversation'\nasync_classifier_conversation_token_limit = {limit}"
        ))
        .await?;
        let sampler = Arc::new(LunaSampler::new(sampler_config(server.uri())));
        let backend = ConversationBackend::new(sampler);
        let events = responses::sse(vec![
            ev_assistant_message("score", "low"),
            ev_completed("score"),
        ]);
        let mock = responses::mount_sse_sequence(&server, vec![events; 2]).await;
        for index in 0..2 {
            let (request, score) = pending_request(&fixture, "parent").await?;
            backend.reserve(|| index).1.unwrap().run(request).await;
            assert_eq!(score.await??, "low");
        }
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        let input = requests[1].input();
        assert_eq!(input.iter().any(|item| item["id"] == "score"), reused);
        assert_eq!(
            serde_json::to_string(&input)?.contains("TRANSCRIPT DELTA START"),
            reused
        );
        drop(backend);
        fixture.test.codex.shutdown_and_wait().await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn earlier_observation_keeps_the_last_available_slot() -> Result<()> {
    let sampler = Arc::new(LunaSampler::new(sampler_config(
        "http://localhost".to_owned(),
    )));
    let backend = Arc::new(ConversationBackend::new(sampler));
    let occupied = (0..15)
        .map(|index| backend.reserve(|| index).1.expect("available slot"))
        .collect::<Vec<_>>();
    let (observed, first_observed) = tokio::sync::oneshot::channel();
    let (release_first, release) = std::sync::mpsc::channel();
    let first_backend = Arc::clone(&backend);
    let first = tokio::task::spawn_blocking(move || {
        first_backend.reserve(|| {
            observed.send(()).expect("first observation receiver");
            release.recv().expect("release first observation");
            15
        })
    });
    first_observed.await?;

    let (started, later_started) = tokio::sync::oneshot::channel();
    let (admitted, later_admitted) = tokio::sync::oneshot::channel();
    let later = tokio::task::spawn_blocking(move || {
        started.send(()).expect("later admission receiver");
        let reservation = backend.reserve(|| 16);
        let _ = admitted.send(());
        reservation
    });
    later_started.await?;
    // The later admission must wait for the earlier observation to reserve its slot.
    let later_waited = tokio::time::timeout(Duration::from_millis(250), later_admitted)
        .await
        .is_err();
    // Always release the blocking task before assertions, even if admission was reordered.
    release_first.send(())?;
    let (first_index, first_reservation) = first.await?;
    let (later_index, later_reservation) = later.await?;
    assert!(later_waited);
    assert_eq!((first_index, later_index), (15, 16));
    assert!(first_reservation.is_ok());
    assert!(matches!(
        later_reservation,
        Err(LunaSamplerError::QueueFull)
    ));
    drop(occupied);
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_preparation_does_not_block_newer_requests_and_capacity_is_bounded() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    let sampler = Arc::new(LunaSampler::new(sampler_config(server.uri())));
    fixture
        .test
        .codex
        .thread_extension_data()
        .insert(ConversationBackend::new(sampler));
    let events = ["C", "B", "A", "D"].map(|label| {
        responses::sse(vec![
            ev_assistant_message(&format!("score-{label}"), "low"),
            ev_completed(label),
        ])
    });
    let mock = responses::mount_sse_sequence(&server, events.to_vec()).await;
    let backend = fixture
        .test
        .codex
        .thread_extension_data()
        .get::<ConversationBackend>()
        .unwrap();
    let store = fixture.test.codex.thread_extension_data();
    let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
    let mut reservations = (0..16)
        .map(|index| backend.reserve(|| index).1.unwrap())
        .collect::<Vec<_>>();
    fixture.score_tool(ToolName::plain("read_file")).await;
    let cached = progress.inspect(/*call_id*/ None);
    assert!(cached.has_unscored_failure);
    assert_eq!(cached.action_risk, Some(1.0));
    assert_eq!(
        cached_approval(
            &fixture.registry,
            store,
            "review action",
            /*metrics*/ None
        )
        .await,
        None
    );
    let c = reservations.remove(/*index*/ 2);
    let b = reservations.remove(/*index*/ 1);
    let a = reservations.remove(/*index*/ 0);
    drop(reservations);
    // C must complete while A and B are still preparing. Their later completions
    // must neither read C's future evidence nor replace its retained history.
    for (reservation, label) in [(c, "C"), (b, "B"), (a, "A")] {
        let (request, score) = pending_request(&fixture, label).await?;
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, reservation.run(request)).await?;
        assert_eq!(score.await??, "low");
    }
    let (request, score) = pending_request(&fixture, "D").await?;
    backend.reserve(|| 3).1.unwrap().run(request).await;
    assert_eq!(score.await??, "low");
    let requests = mock.requests();
    let turns = requests
        .iter()
        .map(|request| {
            request.body_json()["client_metadata"]["parent_turn_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(turns, ["C", "B", "A", "D"]);
    let retained = requests
        .iter()
        .map(|request| {
            request
                .input()
                .into_iter()
                .filter_map(|item| {
                    item["id"]
                        .as_str()
                        .filter(|id| id.starts_with("score-"))
                        .map(str::to_owned)
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    assert_eq!(
        retained,
        [vec![], vec![], vec![], vec!["score-C".to_owned()]]
    );
    fixture.test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_authorization_and_stopped_generations_cannot_start() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    let sampler = Arc::new(LunaSampler::new(sampler_config(server.uri())));
    fixture
        .test
        .codex
        .thread_extension_data()
        .insert(ConversationBackend::new(sampler));
    let thread_store = fixture.test.codex.thread_extension_data();
    let backend = thread_store.get::<ConversationBackend>().unwrap();
    let blocker = backend.reserve(|| 0).1.unwrap();
    let stale = backend.reserve(|| 0).1.unwrap();
    let (request, score) = pending_request(&fixture, "stale").await?;
    fixture
        .test
        .codex
        .inject_response_items(vec![user_instruction(
            "Stop. Do not inspect any more files.",
        )])
        .await?;
    stale.run(request).await;
    drop(blocker);
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));
    let stopped = backend.reserve(|| 0).1.unwrap();
    let (request, score) = pending_request(&fixture, "stopped").await?;
    drop(backend);
    fixture.registry.thread_lifecycle_contributors()[0]
        .on_thread_stop(ThreadStopInput {
            session_store: &fixture.session_store,
            thread_store,
        })
        .await;
    stopped.run(request).await;
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));
    assert!(
        server
            .received_requests()
            .await
            .unwrap()
            .iter()
            .all(|request| request.method.as_str() != "POST")
    );
    fixture.test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn concurrent_requests_retain_only_the_newest_completed_history() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for older_outcome in ["completed", "cancelled"] {
        let fixture = GuardianFailureFixture::with_config(
            "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
        )
        .await?;
        let mut streams = ["seed", "older", "newer", "unretained", "probe"]
            .map(|label| {
                vec![StreamingSseChunk {
                    gate: None,
                    body: responses::sse(vec![
                        responses::ev_output_text_delta("low"),
                        ev_assistant_message(
                            label,
                            if label == "unretained" {
                                "lowhigh"
                            } else {
                                "low"
                            },
                        ),
                        ev_completed(label),
                    ]),
                }]
            })
            .into_iter()
            .collect::<Vec<_>>();
        let (finish_older, older_completion) = tokio::sync::oneshot::channel();
        let mut finish_older = Some(finish_older);
        streams[1] = vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![responses::ev_output_text_delta("low")]),
            },
            StreamingSseChunk {
                gate: Some(older_completion),
                body: responses::sse(vec![
                    ev_assistant_message("older", "low"),
                    ev_completed("older"),
                ]),
            },
        ];
        let (server, _) = start_streaming_sse_server(streams).await;
        let sampler = Arc::new(LunaSampler::new(sampler_config(format!(
            "{}/v1",
            server.uri()
        ))));
        let backend = ConversationBackend::new(sampler);
        let (request, score) = pending_request(&fixture, "seed").await?;
        backend.reserve(|| 0).1.unwrap().run(request).await;
        assert_eq!(score.await??, "low");

        let mut older = None;
        for (index, label) in [(1, "older"), (2, "newer")] {
            fixture
                .test
                .codex
                .inject_response_items(vec![ResponseItem::Message {
                    id: None,
                    role: "assistant".to_owned(),
                    content: vec![ContentItem::OutputText {
                        text: format!("Observed action {index}"),
                    }],
                    phase: Some(MessagePhase::Commentary),
                    internal_chat_message_metadata_passthrough: None,
                }])
                .await?;
            let (request, score) = pending_request(&fixture, label).await?;
            let task = tokio::spawn(backend.reserve(|| index).1.unwrap().run(request));
            assert_eq!(
                tokio::time::timeout(ASYNC_TEST_TIMEOUT, score).await???,
                "low"
            );
            if index == 1 {
                older = Some(task);
            } else {
                tokio::time::timeout(ASYNC_TEST_TIMEOUT, task).await??;
            }
        }
        let older = older.unwrap();
        assert!(
            !older.is_finished(),
            "newer must finish while older is still draining"
        );

        // A newer response with invalid retained output must not replace good history.
        let (request, score) = pending_request(&fixture, "unretained").await?;
        backend.reserve(|| 3).1.unwrap().run(request).await;
        assert_eq!(score.await??, "low");
        if older_outcome == "completed" {
            finish_older.take().unwrap().send(()).unwrap();
            tokio::time::timeout(ASYNC_TEST_TIMEOUT, older).await??;
        } else {
            older.abort();
            let error = tokio::time::timeout(ASYNC_TEST_TIMEOUT, older)
                .await?
                .unwrap_err();
            assert!(error.is_cancelled());
        }
        drop(finish_older);
        let (request, score) = pending_request(&fixture, "probe").await?;
        backend.reserve(|| 4).1.unwrap().run(request).await;
        assert_eq!(score.await??, "low");

        let requests = server
            .requests()
            .await
            .iter()
            .map(|body| serde_json::from_slice::<serde_json::Value>(body))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let retained = requests
            .iter()
            .map(|request| {
                request["input"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .filter_map(|item| item["id"].as_str())
                    .filter(|id| ["seed", "older", "newer", "unretained", "probe"].contains(id))
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        assert_eq!(
            retained,
            [
                vec![],
                vec!["seed"],
                vec!["seed"],
                vec!["seed", "newer"],
                vec!["seed", "newer"]
            ]
        );
        let seed = requests[0]["input"].as_array().unwrap();
        let newer = requests[2]["input"].as_array().unwrap();
        let probe = requests[4]["input"].as_array().unwrap();
        assert_eq!(&newer[..seed.len()], seed);
        assert_eq!(&probe[..newer.len()], newer);
        let delta = serde_json::to_string(&newer[seed.len()..])?;
        assert!(delta.contains("Observed action 1"));
        assert!(delta.contains("Observed action 2"));
        assert!(delta.contains("TRANSCRIPT DELTA START"));
        let entries = requests
            .iter()
            .zip(["seed", "older", "newer", "unretained", "probe"])
            .map(|(request, label)| SnapshotEntry::body(request).labeled(label))
            .collect::<Vec<_>>();
        // Completing or cancelling the older request must leave the same context.
        insta::assert_snapshot!(
            "concurrent_requests_retain_only_the_newest_completed_history",
            format_context_snapshot(
                "Concurrent reviews fork the latest completed history. The newer review finishes first; invalid output and the older review cannot replace its history.",
                &entries,
                &ContextSnapshotOptions::default().include_request_settings(),
            )
        );
        fixture.test.codex.shutdown_and_wait().await?;
        server.shutdown().await;
    }
    Ok(())
}
