//! Exercises lag expiry through tool admission and real classifier transports.

use super::*;
use crate::async_scorer::conversation::ConversationBackend;
use crate::async_scorer::sampler::tests::RecordingMetrics;
use crate::async_scorer::sampler::tests::sampler_config;
use pretty_assertions::assert_eq;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn stale_classifications_close_the_transport_without_invalidating_scores() -> Result<()> {
    skip_if_no_network!(Ok(()));
    for mode in ["snapshot", "conversation"] {
        for (max_lag, early_score) in [(0, false), (2, false), (4, false), (2, true)] {
            let fixture = GuardianFailureFixture::with_config(&format!(
                "[features.guardianv2]\nasync_classifier_mode = '{mode}'\nmax_tool_call_lag = {max_lag}"
            ))
            .await?;
            let mut events = vec![responses::ev_response_created("stale")];
            if early_score {
                events.push(responses::ev_output_text_delta("low"));
            }
            let server =
                responses::start_websocket_server_with_headers(vec![WebSocketConnectionConfig {
                    // Rejected interruption exercises the close fallback.
                    requests: vec![
                        events,
                        vec![json!({
                            "type": "response.interrupt.failed",
                            "response_id": "stale",
                            "error": {"code": "interrupt_not_supported"},
                        })],
                        // Keep reading until the client closes; no third request is expected.
                        Vec::new(),
                    ],
                    response_headers: Vec::new(),
                    accept_delay: None,
                    close_after_requests: true,
                }])
                .await;
            let store = fixture.test.codex.thread_extension_data();
            let metrics = Arc::new(RecordingMetrics::default());
            let mut config = sampler_config(
                proxy_websocket_servers_with_http(
                    &[&server],
                    ProxyPrewarmLimit::AllConnections,
                    /*http_url*/ None,
                )
                .await?,
            );
            config.metrics = Some(metrics.clone());
            store.insert(LunaSampler::new(config));
            let sampler = store.get::<LunaSampler>().unwrap();
            sampler.prewarm().await;
            if mode == "conversation" {
                store.insert(ConversationBackend::new(Arc::clone(&sampler)));
            }
            let progress = store.get::<GuardianV2ScoreProgress>().unwrap();
            let mut updates = progress.updates.subscribe();
            fixture.score_tool(ToolName::plain("read_file")).await;
            if early_score {
                tokio::time::timeout(ASYNC_TEST_TIMEOUT, async {
                    while progress.inspect(/*call_id*/ None).action_risk != Some(0.0) {
                        updates
                            .changed()
                            .await
                            .expect("score progress remains live");
                    }
                })
                .await?;
            }
            tokio::time::timeout(
                ASYNC_TEST_TIMEOUT,
                server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 0),
            )
            .await?;
            let score = cached_score(store);
            let mut expected = progress.inspect(/*call_id*/ None);
            for _ in 0..max_lag {
                observe_unscored_call(&progress, store);
            }
            // The running classifier must not send an interrupt at the reusable boundary.
            assert!(
                tokio::time::timeout(
                    Duration::from_millis(250),
                    server.wait_for_request(/*connection_index*/ 0, /*request_index*/ 1),
                )
                .await
                .is_err(),
                "classifier interrupted inside its configured lag window"
            );
            observe_unscored_call(&progress, store);
            assert!(
                server
                    .wait_for_closed_connections(/*expected*/ 1, ASYNC_TEST_TIMEOUT)
                    .await
            );
            assert_eq!(server.single_connection().len(), 2);
            expected.lag += max_lag + 1;
            assert_eq!(progress.inspect(/*call_id*/ None), expected);
            assert_eq!(cached_score(store), score);

            let (outcome, failure_reason) = if early_score {
                ("success", "none")
            } else {
                ("superseded", "superseded")
            };
            // Socket closure can precede the sampling task recording its outcome.
            tokio::time::timeout(ASYNC_TEST_TIMEOUT, metrics.1.notified()).await?;
            metrics.assert_outcome(outcome, /*attempts*/ 1, failure_reason);

            fixture.test.codex.shutdown_and_wait().await?;
            server.shutdown().await;
        }
    }
    Ok(())
}
