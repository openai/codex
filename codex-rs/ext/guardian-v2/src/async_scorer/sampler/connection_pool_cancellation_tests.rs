//! Classifier cancellation preserves usage and reuses only unused or fully drained sockets.

use super::*;
use crate::async_scorer::sampler::CLASSIFICATION_TOKEN_USAGE_METRIC;
use crate::async_scorer::sampler::LunaSamplerError;
use crate::async_scorer::sampler::response::PendingResponse;
use crate::async_scorer::sampler::tests::RecordingMetrics;
use crate::async_scorer::sampler::tests::connect_sampler;
use crate::async_scorer::sampler::tests::proxy_websocket_servers;
use crate::async_scorer::sampler::tests::sample_request;
use crate::async_scorer::sampler::tests::sampler_config;
use anyhow::Result;
use codex_api::ResponseEvent;
use codex_api::ResponseInterrupt;
use core_test_support::responses;
use core_test_support::responses::ev_assistant_message;
use core_test_support::responses::ev_completed;
use core_test_support::responses::ev_completed_with_tokens;
use core_test_support::skip_if_no_network;
use pretty_assertions::assert_eq;
use serde_json::json;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cancelled_classifiers_interrupt_and_reuse_only_completed_streams() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let accepted = json!({"type": "response.interrupt.accepted", "response_id": "stale"});
    let rejected = json!({
        "type": "response.interrupt.failed",
        "response_id": "stale",
        "error": {"code": "interrupt_not_supported", "message": "Interruption is disabled"},
    });
    let error = json!({"type": "error", "status": 400});
    let completed = ev_completed_with_tokens("stale", /*total_tokens*/ 42);
    // None means reuse; explicit failures must close before the one-second drain deadline.
    let close_now = Some(Duration::from_millis(500));
    let close_after_timeout = Some(Duration::from_secs(2));
    for (interrupted, close_timeout, expected_tokens, outcome) in [
        (vec![accepted.clone(), completed.clone()], None, Some(42)),
        (vec![completed.clone(), accepted], None, Some(42)),
        (vec![rejected.clone()], close_now, None),
        (vec![completed.clone(), rejected], close_now, Some(42)),
        (vec![completed.clone(), error.clone()], close_now, Some(42)),
        (
            vec![
                json!({"type": "response.interrupt.accepted", "response_id": "another-response"}),
                completed.clone(),
            ],
            close_now,
            None,
        ),
        (vec![completed], close_after_timeout, Some(42)),
        (vec![error], close_after_timeout, None),
        (Vec::new(), close_after_timeout, None),
    ]
    .into_iter()
    .flat_map(|(events, timeout, tokens)| {
        ["cancelled", "superseded"].map(|outcome| (events.clone(), timeout, tokens, outcome))
    }) {
        let server = responses::start_websocket_server(vec![vec![
            vec![responses::ev_response_created("stale")],
            interrupted,
            vec![ev_assistant_message("fresh", "low"), ev_completed("fresh")],
        ]])
        .await;
        // Offer one socket: another classification can use it only if cancellation returned it.
        let metrics = Arc::new(RecordingMetrics::default());
        let mut config = sampler_config(proxy_websocket_servers(&[&server]).await?);
        config.metrics = Some(metrics.clone());
        let sampler = Arc::new(connect_sampler(config).await?);
        let pending_sampler = Arc::clone(&sampler);
        let mut request = sample_request("stale");
        request.pending_score = Some(crate::async_scorer::score::PendingScore {
            progress: Arc::default(),
            index: 0,
            max_lag: 0,
        });
        let pending = tokio::spawn(async move { pending_sampler.sample(request).await });
        tokio::time::timeout(
            Duration::from_secs(2),
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
        )
        .await?;
        if outcome == "cancelled" {
            pending.abort();
            assert!(pending.await.unwrap_err().is_cancelled());
        } else {
            sampler
                .active_requests
                .lock()
                .unwrap()
                .pop_front()
                .unwrap()
                .supersede
                .send(())
                .unwrap();
            assert!(matches!(pending.await?, Err(LunaSamplerError::Superseded)));
        }
        metrics.assert_outcome(outcome, /*attempts*/ 1, outcome);
        let interrupt = tokio::time::timeout(
            Duration::from_secs(2),
            server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 1),
        )
        .await?;
        assert_eq!(
            interrupt.body_json(),
            json!({
                "type": "response.interrupt", "response_id": "stale", "mode": "discard_partial_items"
            })
        );
        if let Some(close_timeout) = close_timeout {
            assert!(
                server
                    .wait_for_closed_connections(/*expected*/ 1, close_timeout)
                    .await
            );
        }
        // Reacquire all slots to await cleanup and accounting, even after socket closure.
        let permits = tokio::time::timeout(
            Duration::from_secs(2),
            Arc::clone(&sampler.connections.classifications)
                .acquire_many_owned(u32::try_from(MAX_CONCURRENT_REQUESTS)?),
        )
        .await??;
        assert_eq!(
            sampler.connections.idle_connections.lock().unwrap().len(),
            usize::from(close_timeout.is_none())
        );
        drop(permits);
        if close_timeout.is_none() {
            assert_eq!(sampler.sample(sample_request("fresh")).await?, "low");
            let requests = server.single_connection();
            assert_eq!(requests.len(), 3);
            assert_eq!(
                requests[2].body_json()["client_metadata"]["parent_turn_id"],
                json!("fresh")
            );
        }
        // A missing acknowledgement must not discard usage already received by the transport.
        if let Some(total) = expected_tokens {
            let expected = (
                CLASSIFICATION_TOKEN_USAGE_METRIC.to_owned(),
                total,
                vec![("token_type".to_owned(), "total".to_owned())],
            );
            // Both branches above synchronize with lease release, after accounting.
            assert!(metrics.0.lock().unwrap().contains(&expected));
        }
        drop(sampler);
        server.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn cancellation_preserves_unused_sockets_and_backpressured_completion() -> Result<()> {
    skip_if_no_network!(Ok(()));
    let mut completed = ev_completed_with_tokens("stale", /*total_tokens*/ 42);
    completed["safety_buffering"] = json!({"use_cases": [], "reasons": []});
    let output_events = 2_000;
    let events = std::iter::once(responses::ev_response_created("stale"))
        .chain(std::iter::repeat_n(
            responses::ev_output_text_delta("x"),
            output_events,
        ))
        .chain([completed])
        .collect();
    // Keep the server open after an interrupt so only the cleanup deadline closes it.
    let server =
        responses::start_websocket_server(vec![vec![events, Vec::new(), Vec::new()]]).await;
    let sampler =
        connect_sampler(sampler_config(proxy_websocket_servers(&[&server]).await?)).await?;
    let pool = &sampler.connections;
    let lease = pool.lease().await?;
    let thread_id = lease.thread_id.clone();
    let request = sampler
        .execution(
            &sample_request("stale"),
            Vec::new(),
            /*auth_owner_generation*/ None,
        )
        .request;
    // Cancel before this current-thread runtime can dispatch, then reuse its only socket.
    let stream = lease.stream_request(&request).await?;
    drop(PendingResponse::new(stream, lease, /*metrics*/ None));
    let cleanup = pool
        .classifications
        .acquire_many(u32::try_from(MAX_CONCURRENT_REQUESTS)?);
    drop(tokio::time::timeout(Duration::from_secs(/*secs*/ 2), cleanup).await??);
    let lease = pool.lease().await?;
    assert_eq!(lease.thread_id, thread_id);
    let mut stream = lease.stream_request(&request).await?;
    // Consume Created and leave exactly a full channel of output before completion.
    let consumed = output_events
        .checked_sub(stream.rx_event.max_capacity())
        .expect("fixture must fill the response channel")
        + 1;
    for _ in 0..consumed {
        stream.rx_event.recv().await.expect("output")?;
    }
    let interrupt = stream.interrupt.take().expect("websocket interruption");
    assert!(
        interrupt
            .send(ResponseInterrupt::Cancel {
                deadline: Instant::now() + Duration::from_secs(/*secs*/ 1),
            })
            .is_ok()
    );
    assert!(
        server
            .wait_for_closed_connections(/*expected*/ 1, Duration::from_secs(/*secs*/ 2))
            .await
    );
    let mut total_tokens = None;
    while let Some(event) = stream.rx_event.recv().await {
        if let ResponseEvent::Completed { token_usage, .. } = event? {
            total_tokens = token_usage.map(|usage| usage.total_tokens);
        }
    }
    assert_eq!(total_tokens, Some(42));
    drop(lease);
    server.shutdown().await;
    Ok(())
}
