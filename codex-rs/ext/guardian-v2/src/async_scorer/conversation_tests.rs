use super::*;
use crate::async_scorer::authorization::ScoreAuthorization;
use crate::async_scorer::conversation::ConversationBackend;
use crate::async_scorer::conversation::ConversationRequest;
use crate::async_scorer::sampler::LunaSampler;
use crate::async_scorer::sampler::LunaSamplerError;
use crate::async_scorer::sampler::LunaSamplingRequest;
use crate::async_scorer::sampler::SampledScore;
use crate::async_scorer::sampler::tests::sample_request;
use crate::async_scorer::sampler::tests::sampler_config;
use crate::async_scorer::score::PendingScore;
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
    impl std::future::Future<
        Output = Result<Result<String, LunaSamplerError>, tokio::sync::oneshot::error::RecvError>,
    >,
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
    let pending_score = PendingScore {
        progress: thread
            .thread_extension_data()
            .get::<GuardianV2ScoreProgress>()
            .unwrap(),
        index: 0,
        max_lag: config.max_tool_call_lag,
    };
    Ok((
        ConversationRequest {
            evidence,
            reset_token_limit: config.async_classifier_conversation_token_limit,
            authorization,
            thread,
            ready,
            metrics: None,
            pending_score,
            sampled_at: std::time::SystemTime::now(),
            sampling: LunaSamplingRequest {
                instructions,
                input: Vec::new(),
                ..sample_request(label)
            },
        },
        async move { score.await.map(|result| result.map(SampledScore::accept)) },
    ))
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn configured_reset_limit_rebuilds_history_without_rejecting_fresh_evidence() -> Result<()> {
    use crate::async_scorer::decisions::DecisionsSampler;
    use codex_http_client::HttpClientBuilder;

    skip_if_no_network!(Ok(()));
    for (limit, reused) in [(1, false), (100_000, true)] {
        let server = responses::start_mock_server().await;
        let fixture = GuardianFailureFixture::with_config(&format!(
            "[features]\nguardianv2_decisions_comparison = true\n[features.guardianv2]\nasync_classifier_mode = 'conversation'\nasync_classifier_conversation_token_limit = {limit}"
        ))
        .await?;
        let store = fixture.test.codex.thread_extension_data();
        let metrics = Arc::new(RecordingMetrics::default());
        store.insert(GuardianV2ScoreProgress::new(Some(metrics.clone())));
        store.insert(LunaSampler::new(sampler_config(server.uri())));
        store.insert(ConversationBackend::new(
            store.get::<LunaSampler>().unwrap(),
        ));
        let backend = store.get::<ConversationBackend>().unwrap();
        let events = responses::sse(vec![
            ev_assistant_message("score", "low"),
            ev_completed("score"),
        ]);
        let mock = responses::mount_sse_sequence(&server, vec![events; 2]).await;
        let (finish_decisions, decisions_gate) = tokio::sync::oneshot::channel();
        let (decisions_server, _) = start_streaming_sse_server(vec![vec![StreamingSseChunk {
            gate: Some(decisions_gate),
            body: json!({
                "answers":[{"type":"choice", "name":"guardian_risk", "choice":"high"}]
            })
            .to_string(),
        }]])
        .await;
        store.insert(DecisionsSampler::new(
            HttpClientBuilder::new().build_direct()?,
            "synthetic-key".into(),
            format!("{}/v1/responses", decisions_server.uri()),
        )?);
        let (mut request, score) = pending_request(&fixture, "parent").await?;
        request.metrics = Some(metrics.clone());
        let reservation = backend.reserve(|| 0).1.unwrap();
        let prepared = reservation.prepare(request).await.unwrap();
        assert!(mock.requests().is_empty());
        // Commit the seed history before triggering the next review through production.
        prepared.run().await;
        assert_eq!(score.await??, "low");
        fixture.score_tool(ToolName::plain("read_file")).await;
        let published = tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            loop {
                if let Some(score) = cached_score(store)
                    && score.sampled_at.is_some()
                {
                    break score;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        assert_eq!(
            published.scores,
            BTreeMap::from([("action_risk".into(), 0.0)])
        );
        tokio::time::timeout(
            ASYNC_TEST_TIMEOUT,
            decisions_server.wait_for_request_count(/*count*/ 1),
        )
        .await?;
        finish_decisions.send(()).unwrap();
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            loop {
                if metrics.0.lock().unwrap().iter().any(|sample| matches!(sample,
                    RecordedMetric::Counter(name, _, _) if name == "codex.guardian_v2.decisions_comparison.comparison")) {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await?;
        // Success rules out a timeout before Guardian published.
        assert!(metrics.0.lock().unwrap().contains(&RecordedMetric::Counter(
            "codex.guardian_v2.decisions_comparison".into(),
            1,
            vec![
                ("outcome".into(), "success".into()),
                ("reason".into(), "none".into()),
            ],
        )));
        let requests = mock.requests();
        assert_eq!(requests.len(), 2);
        let expected_buckets = requests
            .iter()
            .map(|request| {
                let input: Vec<ResponseItem> =
                    serde_json::from_value(request.body_json()["input"].clone()).unwrap();
                let tokens = input
                    .iter()
                    .map(codex_guardian_context::estimate_input_tokens)
                    .sum();
                RecordedMetric::Histogram(
                    codex_guardian_context::REQUEST_TOKENS_METRIC.to_owned(),
                    i64::try_from(tokens).unwrap(),
                    vec![
                        ("target".to_owned(), "async".to_owned()),
                        ("component".to_owned(), "total".to_owned()),
                        (
                            "token_bucket".to_owned(),
                            codex_guardian_context::request_token_bucket(tokens).to_owned(),
                        ),
                    ],
                )
            })
            .collect::<Vec<_>>();
        let buckets = metrics
            .0
            .lock()
            .unwrap()
            .iter()
            .filter(|sample| {
                matches!(sample, RecordedMetric::Histogram(name, _, tags)
                if name == codex_guardian_context::REQUEST_TOKENS_METRIC
                    && tags.iter().any(|(key, _)| key == "token_bucket"))
            })
            .cloned()
            .collect::<Vec<_>>();
        assert_eq!(buckets, expected_buckets);
        let input = requests[1].input();
        assert_eq!(input.iter().any(|item| item["id"] == "score"), reused);
        assert_eq!(
            serde_json::to_string(&input)?.contains("TRANSCRIPT DELTA START"),
            reused
        );
        let evidence = input[2..]
            .iter()
            .map(|item| json!({"role":item["role"], "content":item["content"]}))
            .collect::<Vec<_>>();
        let decisions_requests = decisions_server.requests().await;
        assert_eq!(decisions_requests.len(), 1);
        let body: serde_json::Value = serde_json::from_slice(&decisions_requests[0])?;
        assert_eq!(body["input"], json!(evidence));
        drop(backend);
        fixture.test.codex.shutdown_and_wait().await?;
        decisions_server.shutdown().await;
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
    let (request, score) = pending_request(&fixture, "dropped").await?;
    let reservation = reservations.pop().unwrap();
    let prepared = reservation.prepare(request).await.unwrap();
    assert!(matches!(
        backend.reserve(|| 16).1,
        Err(LunaSamplerError::QueueFull)
    ));
    drop(prepared);
    assert!(score.await.is_err());
    reservations.push(backend.reserve(|| 15).1.unwrap());
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
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
            let prepared = reservation.prepare(request).await.unwrap();
            prepared.run().await;
        })
        .await?;
        assert_eq!(score.await??, "low");
    }
    let (request, score) = pending_request(&fixture, "D").await?;
    let reservation = backend.reserve(|| 3).1.unwrap();
    let prepared = reservation.prepare(request).await.unwrap();
    prepared.run().await;
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
async fn stale_authorization_expiry_and_stopped_generations_reject_preparation() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    // Preparation would fail with InputTooLarge if it ran before the expiry check.
    let mut config = sampler_config(server.uri());
    config.max_input_tokens = 0;
    let sampler = Arc::new(LunaSampler::new(config));
    fixture
        .test
        .codex
        .thread_extension_data()
        .insert(ConversationBackend::new(sampler));
    let thread_store = fixture.test.codex.thread_extension_data();
    let backend = thread_store.get::<ConversationBackend>().unwrap();
    let (mut request, score) = pending_request(&fixture, "expired").await?;
    request.pending_score.max_lag = 0;
    let progress = Arc::clone(&request.pending_score.progress);
    let (index, expired) = backend.reserve(|| observe_unscored_call(&progress, thread_store));
    request.pending_score.index = index;
    let expired = expired.unwrap();
    let current_index = observe_unscored_call(&progress, thread_store);
    assert!(expired.prepare(request).await.is_none());
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));

    let blocker = backend.reserve(|| 0).1.unwrap();
    let stale = backend.reserve(|| 0).1.unwrap();
    let (mut request, score) = pending_request(&fixture, "stale").await?;
    request.pending_score.index = current_index;
    fixture
        .test
        .codex
        .inject_response_items(vec![user_instruction(
            "Stop. Do not inspect any more files.",
        )])
        .await?;
    assert!(stale.prepare(request).await.is_none());
    drop(blocker);
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));
    let stopped = backend.reserve(|| 0).1.unwrap();
    let (mut request, score) = pending_request(&fixture, "stopped").await?;
    request.pending_score.index = current_index;
    drop(backend);
    fixture.registry.thread_lifecycle_contributors()[0]
        .on_thread_stop(ThreadStopInput {
            session_store: &fixture.session_store,
            thread_store,
        })
        .await;
    assert!(stopped.prepare(request).await.is_none());
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
async fn prepared_reviews_recheck_expiry_authorization_and_stopped_generations() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let server = responses::start_mock_server().await;
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    let config = sampler_config(server.uri());
    let sampler = Arc::new(LunaSampler::new(config));
    fixture
        .test
        .codex
        .thread_extension_data()
        .insert(ConversationBackend::new(sampler));
    let thread_store = fixture.test.codex.thread_extension_data();
    let backend = thread_store.get::<ConversationBackend>().unwrap();
    let (mut request, score) = pending_request(&fixture, "expired").await?;
    request.pending_score.max_lag = 0;
    let progress = Arc::clone(&request.pending_score.progress);
    let (index, expired) = backend.reserve(|| observe_unscored_call(&progress, thread_store));
    request.pending_score.index = index;
    let expired = expired.unwrap();
    let prepared = expired.prepare(request).await.unwrap();
    let current_index = observe_unscored_call(&progress, thread_store);
    prepared.run().await;
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));

    let blocker = backend.reserve(|| 0).1.unwrap();
    let stale = backend.reserve(|| 0).1.unwrap();
    let (mut request, score) = pending_request(&fixture, "stale").await?;
    request.pending_score.index = current_index;
    let prepared = stale.prepare(request).await.unwrap();
    fixture
        .test
        .codex
        .inject_response_items(vec![user_instruction(
            "Stop. Do not inspect any more files.",
        )])
        .await?;
    prepared.run().await;
    drop(blocker);
    assert!(matches!(score.await?, Err(LunaSamplerError::Superseded)));
    let stopped = backend.reserve(|| 0).1.unwrap();
    let (mut request, score) = pending_request(&fixture, "stopped").await?;
    request.pending_score.index = current_index;
    let prepared = stopped.prepare(request).await.unwrap();
    drop(backend);
    fixture.registry.thread_lifecycle_contributors()[0]
        .on_thread_stop(ThreadStopInput {
            session_store: &fixture.session_store,
            thread_store,
        })
        .await;
    prepared.run().await;
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
async fn preflight_skips_only_after_score_and_history_are_superseded() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for (retain_newer, publish_newer, risk) in [
        (true, false, 0.0),
        (false, true, 0.0),
        (true, true, 0.0),
        (true, true, 1.0),
    ] {
        let fixture = GuardianFailureFixture::with_config(
            "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
        )
        .await?;
        let store = fixture.test.codex.thread_extension_data();
        let mut cached = cached_score(store).unwrap();
        cached.scores.insert("action_risk".to_owned(), risk);
        set_cached_score(store, cached);
        let server = responses::start_mock_server().await;
        let mut events = vec![responses::sse(vec![
            responses::ev_output_text_delta("low"),
            ev_assistant_message("newer", if retain_newer { "low" } else { "lowhigh" }),
            ev_completed("newer"),
        ])];
        if !(retain_newer && publish_newer) {
            events.push(responses::sse(vec![
                ev_assistant_message("older", "low"),
                ev_completed("older"),
            ]));
        }
        let mock = responses::mount_sse_sequence(&server, events).await;
        let backend =
            ConversationBackend::new(Arc::new(LunaSampler::new(sampler_config(server.uri()))));
        let (request, older_score) = pending_request(&fixture, "older").await?;
        let older = backend
            .reserve(|| 0)
            .1
            .unwrap()
            .prepare(request)
            .await
            .unwrap();
        let (request, newer_score) = pending_request(&fixture, "newer").await?;
        let authorization = request.authorization.clone();
        backend
            .reserve(|| 1)
            .1
            .unwrap()
            .prepare(request)
            .await
            .unwrap()
            .run()
            .await;
        assert_eq!(newer_score.await??, "low");
        let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
        if publish_newer {
            seed_cached_score(&progress, store, /*index*/ 1, authorization);
        }
        let before = progress.inspect(/*call_id*/ None);
        let approval_before = cached_approval(
            &fixture.registry,
            store,
            r#"{"tool":"mcp_tool_call","server":"node_repl"}"#,
            /*metrics*/ None,
        )
        .await;
        assert_eq!(
            approval_before,
            if risk == 0.0 {
                Some(ReviewDecision::Approved)
            } else {
                None
            }
        );
        older.run().await;
        if retain_newer && publish_newer {
            assert!(matches!(
                older_score.await?,
                Err(LunaSamplerError::Superseded)
            ));
            assert_eq!(mock.requests().len(), 1);
        } else {
            assert_eq!(older_score.await??, "low");
            let requests = mock.requests();
            assert_eq!(requests.len(), 2);
            assert!(requests[1].input().iter().all(|item| item["id"] != "newer"));
        }
        // Skipping inference must not publish an approval, age coverage, or fail closed.
        assert_eq!(progress.inspect(/*call_id*/ None), before);
        assert_eq!(
            cached_approval(
                &fixture.registry,
                store,
                r#"{"tool":"mcp_tool_call","server":"node_repl"}"#,
                /*metrics*/ None
            )
            .await,
            approval_before
        );
        fixture.test.codex.shutdown_and_wait().await?;
    }
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn delayed_classification_skips_after_newer_score_and_history_complete() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    let server = responses::start_mock_server().await;
    let mock = responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_output_text_delta("low"),
            ev_assistant_message("newer", "low"),
            ev_completed("newer"),
        ]),
    )
    .await;
    let store = fixture.test.codex.thread_extension_data();
    store.insert(LunaSampler::new(sampler_config(server.uri())));
    store.insert(ConversationBackend::new(
        store.get::<LunaSampler>().unwrap(),
    ));
    let backend = store.get::<ConversationBackend>().unwrap();
    // Leave two slots for real classifications. The newer request releases its slot
    // only after committing history, so admission below provides a completion barrier.
    let held = (0..14)
        .map(|_| backend.reserve(|| 0).1.unwrap())
        .collect::<Vec<_>>();
    let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
    let mut updates = progress.updates.subscribe();
    let metrics = store.get::<RecordingMetrics>().unwrap();
    let turn_store = ExtensionData::new("turn-1");
    let tool_name = ToolName::plain("read_file");
    let payload = ToolPayload::Function {
        arguments: r#"{"path":"README.md"}"#.to_owned(),
    };
    let history = fixture.test.codex.conversation_history_snapshot().await;
    let observe = |call_id, permissions| {
        fixture.registry.tool_lifecycle_contributors()[0].on_tool_start(ToolStartInput {
            permissions,
            session_store: &fixture.session_store,
            thread_store: store,
            turn_store: &turn_store,
            turn_id: "turn-1",
            root_turn_id: None,
            call_id,
            originating_item_id: None,
            tool_name: &tool_name,
            mcp_tool: None,
            payload: &payload,
            conversation_history: Arc::clone(&history),
            source: ToolCallSource::Direct,
        })
    };
    let (entered, waiting) = tokio::sync::oneshot::channel();
    let (release, delayed) = tokio::sync::oneshot::channel();
    let older = observe(
        "older",
        Box::pin(async {
            entered.send(()).unwrap();
            delayed.await.unwrap();
            Some(Default::default())
        }),
    );
    tokio::pin!(older);
    tokio::select! {
        result = waiting => result?,
        () = &mut older => panic!("older observation must wait for permissions"),
    }
    observe("newer", Box::pin(async { Some(Default::default()) })).await;
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        while cached_score(store)
            .and_then(|score| score.call_id)
            .as_deref()
            != Some("newer")
        {
            updates.changed().await.unwrap();
        }
        loop {
            match backend.reserve(|| 0).1 {
                Ok(completed) => break drop(completed),
                Err(LunaSamplerError::QueueFull) => tokio::task::yield_now().await,
                Err(error) => panic!("unexpected admission failure: {error}"),
            }
        }
    })
    .await?;
    let score_before = cached_score(store);
    let progress_before = progress.inspect(/*call_id*/ None);
    release.send(()).unwrap();
    older.await;
    // Wait for the real Classification::run task to finish its superseded path.
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        loop {
            let finished = metrics.0.lock().unwrap().iter().any(|metric| {
                matches!(metric, RecordedMetric::Counter(name, _, tags)
                    if name == CLASSIFICATION_METRIC
                        && tags.iter().any(|(key, value)| key == "outcome" && value == "superseded"))
            });
            if finished {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await?;
    assert_eq!(mock.requests().len(), 1);
    assert_eq!(cached_score(store), score_before);
    assert_eq!(progress.inspect(/*call_id*/ None), progress_before);
    drop(held);
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
        let metrics = Arc::new(RecordingMetrics::default());
        let mut config = sampler_config(format!("{}/v1", server.uri()));
        config.metrics = Some(metrics.clone());
        let sampler = Arc::new(LunaSampler::new(config));
        let backend = ConversationBackend::new(sampler);
        let (request, score) = pending_request(&fixture, "seed").await?;
        let reservation = backend.reserve(|| 0).1.unwrap();
        let prepared = reservation.prepare(request).await.unwrap();
        prepared.run().await;
        assert_eq!(score.await??, "low");

        let mut older = None;
        for (index, label) in [(1, "older"), (2, "newer")] {
            fixture
                .test
                .codex
                .inject_response_items(vec![ResponseItem::Message {
                    status: None,
                    encrypted_content: None,
                    id: None,
                    role: "assistant".to_owned(),
                    content: vec![ContentItem::OutputText {
                        annotations: None,
                        logprobs: None,
                        text: format!("Observed action {index}"),
                    }],
                    phase: Some(MessagePhase::Commentary),
                    internal_chat_message_metadata_passthrough: None,
                }])
                .await?;
            let (request, score) = pending_request(&fixture, label).await?;
            let reservation = backend.reserve(|| index).1.unwrap();
            let prepared = reservation.prepare(request).await.unwrap();
            let task = tokio::spawn(prepared.run());
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
        let reservation = backend.reserve(|| 3).1.unwrap();
        let prepared = reservation.prepare(request).await.unwrap();
        prepared.run().await;
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
        assert_eq!(
            metrics
                .0
                .lock()
                .unwrap()
                .iter()
                .filter_map(|metric| match metric {
                    RecordedMetric::Counter(name, _, tags)
                        if name == "codex.guardian_v2.sampling.retry_outcome" =>
                        tags.iter()
                            .find(|(key, _)| key == "outcome")
                            .map(|(_, value)| value.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec!["success"; 4],
        );
        drop(finish_older);
        let (request, score) = pending_request(&fixture, "probe").await?;
        let reservation = backend.reserve(|| 4).1.unwrap();
        let prepared = reservation.prepare(request).await.unwrap();
        prepared.run().await;
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

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expiry_preserves_the_completed_checkpoint_and_transcript_cursor() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = GuardianFailureFixture::with_config(
        "[features.guardianv2]\nasync_classifier_mode = 'conversation'",
    )
    .await?;
    let store = fixture.test.codex.thread_extension_data();
    let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
    let (release_expired, finish_expired) = tokio::sync::oneshot::channel();
    let (server, _completions) = start_streaming_sse_server(vec![
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                ev_assistant_message("checkpoint", "low"),
                ev_completed("checkpoint"),
            ]),
        }],
        vec![
            StreamingSseChunk {
                gate: None,
                body: responses::sse(vec![
                    responses::ev_response_created("expired"),
                    responses::ev_output_text_delta("low"),
                    ev_assistant_message("uncommitted", "low"),
                ]),
            },
            StreamingSseChunk {
                gate: Some(finish_expired),
                body: responses::sse(vec![ev_completed("expired")]),
            },
        ],
        vec![StreamingSseChunk {
            gate: None,
            body: responses::sse(vec![
                ev_assistant_message("resumed", "low"),
                ev_completed("resumed"),
            ]),
        }],
    ])
    .await;
    let backend = ConversationBackend::new(Arc::new(LunaSampler::new(sampler_config(format!(
        "{}/v1",
        server.uri()
    )))));
    let (mut request, score) = pending_request(&fixture, "checkpoint").await?;
    let (index, reservation) = backend.reserve(|| observe_unscored_call(&progress, store));
    request.pending_score.index = index;
    let reservation = reservation.unwrap();
    let prepared = reservation.prepare(request).await.unwrap();
    prepared.run().await;
    assert_eq!(score.await??, "low");

    // This evidence is beyond the committed cursor. A cancelled review must not
    // advance that cursor or cause its evidence to disappear from the next delta.
    fixture
        .test
        .codex
        .inject_response_items(vec![serde_json::from_value(
            responses::ev_function_call(
                "observed-after-checkpoint",
                "read_file",
                r#"{"path":"evidence-after-checkpoint"}"#,
            )["item"]
                .clone(),
        )?])
        .await?;
    let (mut request, early_score) = pending_request(&fixture, "expired").await?;
    let (index, reservation) = backend.reserve(|| observe_unscored_call(&progress, store));
    request.pending_score.index = index;
    let reservation = reservation.unwrap();
    let prepared = reservation.prepare(request).await.unwrap();
    let expired = tokio::spawn(prepared.run());
    assert_eq!(early_score.await??, "low");
    // Keep completion gated even after the early score, then expire the request.
    for _ in 0..3 {
        observe_unscored_call(&progress, store);
    }
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, expired).await??;
    let (mut request, score) = pending_request(&fixture, "resumed").await?;
    let (index, reservation) = backend.reserve(|| observe_unscored_call(&progress, store));
    request.pending_score.index = index;
    let reservation = reservation.unwrap();
    let prepared = reservation.prepare(request).await.unwrap();
    prepared.run().await;
    assert_eq!(
        tokio::time::timeout(ASYNC_TEST_TIMEOUT, score).await???,
        "low"
    );

    let requests = server.requests().await;
    assert_eq!(requests.len(), 3);
    let checkpoint: serde_json::Value = serde_json::from_slice(&requests[0])?;
    let resumed: serde_json::Value = serde_json::from_slice(&requests[2])?;
    let prefix = checkpoint["input"].as_array().unwrap();
    let input = resumed["input"].as_array().unwrap();
    assert_eq!(&input[..prefix.len()], prefix);
    assert_eq!(
        input
            .iter()
            .filter(|item| item["role"] == "assistant")
            .filter_map(|item| item["id"].as_str())
            .collect::<Vec<_>>(),
        ["checkpoint"],
    );
    let delta = serde_json::to_string(&input[prefix.len()..])?;
    assert!(delta.contains("TRANSCRIPT DELTA START"));
    assert_eq!(delta.matches("evidence-after-checkpoint").count(), 1);

    drop(release_expired);
    drop(backend);
    fixture.test.codex.shutdown_and_wait().await?;
    server.shutdown().await;
    Ok(())
}

#[tokio::test]
async fn expiry_discards_a_queued_score_without_recording_success() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let fixture = GuardianFailureFixture::new().await?;
    let store = fixture.test.codex.thread_extension_data();
    let server = responses::start_mock_server().await;
    responses::mount_sse_once(
        &server,
        responses::sse(vec![
            responses::ev_output_text_delta("low"),
            ev_assistant_message("score", "low"),
            ev_completed("score"),
        ]),
    )
    .await;
    let metrics = Arc::new(crate::async_scorer::sampler::tests::RecordingMetrics::default());
    let mut config = sampler_config(server.uri());
    config.metrics = Some(metrics.clone());
    let backend = ConversationBackend::new(Arc::new(LunaSampler::new(config)));
    let (mut request, score) = pending_request(&fixture, "queued").await?;
    request.pending_score.max_lag = 0;
    let (index, reservation) =
        backend.reserve(|| observe_unscored_call(&request.pending_score.progress, store));
    request.pending_score.index = index;
    let pending_score = request.pending_score.clone();
    // Finish the worker while the score stays queued and the consumer remains unpolled.
    let reservation = reservation.unwrap();
    tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
        let prepared = reservation.prepare(request).await.unwrap();
        prepared.run().await;
    })
    .await?;
    assert!(
        !metrics
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|metric| metric.0 == "codex.guardian_v2.sampling.retry_outcome")
    );
    observe_unscored_call(&pending_score.progress, store);
    tokio::select! {
        biased;
        () = pending_score.expired() => {},
        _ = score => panic!("expired score was accepted"),
    }
    metrics.assert_outcome("superseded", /*attempts*/ 1, "superseded");
    fixture.test.codex.shutdown_and_wait().await?;
    Ok(())
}
