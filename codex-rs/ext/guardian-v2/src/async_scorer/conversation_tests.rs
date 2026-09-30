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
            backend.reserve(|| index).1.unwrap().submit(request);
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
async fn admission_orders_delayed_preparation_and_bounds_active_plus_pending_work() -> Result<()> {
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
    let events = responses::sse(vec![
        ev_assistant_message("score", "low"),
        ev_completed("score"),
    ]);
    let mock = responses::mount_sse_sequence(&server, vec![events; 3]).await;
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
    let mut scores = Vec::new();
    for (reservation, label) in [(c, "C"), (b, "B"), (a, "A")] {
        let (request, score) = pending_request(&fixture, label).await?;
        reservation.submit(request);
        scores.push(score);
    }
    for score in scores {
        assert_eq!(score.await??, "low");
    }
    let turns = mock
        .requests()
        .iter()
        .map(|request| {
            request.body_json()["client_metadata"]["parent_turn_id"]
                .as_str()
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    assert_eq!(turns, ["A", "B", "C"]);
    fixture.test.codex.shutdown_and_wait().await?;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn queued_authorization_and_stopped_generations_cannot_continue() -> Result<()> {
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
    stale.submit(request);
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
    stopped.submit(request);
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
