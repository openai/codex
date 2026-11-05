#![cfg(test)]
//! Exercise authorization failures in the real session-recovery loop.

use super::*;
use codex_exec_server::EnvironmentObservedStatus;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use prost::Message as _;
use std::collections::BTreeSet;
use std::sync::atomic::AtomicBool;
use std::time::Instant;
use tokio_tungstenite::tungstenite::Message;

#[path = "../../src/proto/codex.exec_server.relay.v1.rs"]
mod wire;

// Keep the executor connection alive while closing/replacing harness connections,
// as rendezvous does. This is the same topology as the original retry reproduction.
async fn relay_reconnecting_harness(
    mut executor: tokio_tungstenite::WebSocketStream<TcpStream>,
    mut harness: tokio_tungstenite::WebSocketStream<TcpStream>,
    listener: TcpListener,
    mut reset: mpsc::Receiver<()>,
) -> Result<()> {
    loop {
        let mut stream_id = String::new();
        loop {
            tokio::select! {
                Some(()) = reset.recv() => {
                    harness.close(None).await?;
                    break;
                }
                message = harness.next() => {
                    let Some(Ok(message)) = message else { break; };
                    if matches!(message, Message::Close(_)) { break; }
                    if let Message::Binary(bytes) = &message {
                        stream_id = wire::RelayMessageFrame::decode(bytes.as_ref())?.stream_id;
                    }
                    executor.send(message).await?;
                }
                message = executor.next() => {
                    let message = message.context("executor must stay connected")??;
                    if let Message::Ping(bytes) = message {
                        executor.send(Message::Pong(bytes)).await?;
                    } else {
                        harness.send(message).await?;
                    }
                }
            }
        }
        let reset = wire::RelayMessageFrame {
            version: 1,
            stream_id,
            body: Some(wire::relay_message_frame::Body::Reset(wire::RelayReset {
                reason: "harness disconnected".to_owned(),
            })),
            ..Default::default()
        };
        executor
            .send(Message::Binary(reset.encode_to_vec().into()))
            .await?;
        harness = accept_websocket(&listener, "next harness").await?;
    }
}

#[test_case::test_case(401; "rejected_executor_credentials")]
#[test_case::test_case(403; "stale_harness_grant")]
#[test_case::test_case(502; "identity_service_unavailable")]
#[test_case::test_case(503; "service_unavailable")]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn recovery_preserves_auth_failures(status: u16) -> Result<()> {
    let fixture = RegistryFixture::new(vec![200], Duration::ZERO).await?;
    let reject = Arc::new(AtomicBool::new(false));
    let validations = Arc::new(AtomicUsize::new(0));
    Mock::given(method("POST"))
        .and(path(format!(
            "/cloud/environment/{ENVIRONMENT_ID}/validate"
        )))
        .respond_with({
            let reject = reject.clone();
            let validations = validations.clone();
            move |_: &wiremock::Request| {
                let attempt = validations.fetch_add(1, Ordering::SeqCst);
                if reject.load(Ordering::SeqCst) && (status == 401 || attempt < 4) {
                    ResponseTemplate::new(status).set_body_string("secret-must-not-leak")
                } else {
                    ResponseTemplate::new(200).set_body_json(serde_json::json!({"valid": true}))
                }
            }
        })
        .with_priority(1)
        .mount(&fixture.registry)
        .await;
    let (shutdown, remote) = fixture.start()?;
    let executor = accept_websocket(&fixture.listener, "executor").await?;
    let provider = Arc::new(FreshBundleNoiseConnectProvider {
        websocket_url: format!("ws://{}/relay?role=harness", fixture.listener.local_addr()?),
        executor_public_key: registered_executor_public_key(&fixture.registry).await?,
        calls: AtomicUsize::new(0),
    });
    let manager = EnvironmentManager::without_environments(HttpClientFactory::new(
        OutboundProxyPolicy::ReqwestDefault,
    ));
    let environment = manager
        .materialize_pending_noise_environment(ENVIRONMENT_ID.to_owned(), provider.clone())?;
    let mut state = environment.subscribe_connection_state().context("state")?;
    manager.report_environment_provisioning_status(
        ENVIRONMENT_ID.to_owned(),
        Ok(EnvironmentReadyInfo {
            selected_capability_roots: vec![],
        }),
        provider.clone(),
    )?;
    let first = AbortOnDropHandle::new(tokio::spawn({
        let environment = environment.clone();
        async move { environment.force_info().await }
    }));
    let harness = accept_websocket(&fixture.listener, "harness").await?;
    let (reset_tx, reset_rx) = mpsc::channel(1);
    let router = AbortOnDropHandle::new(tokio::spawn(relay_reconnecting_harness(
        executor,
        harness,
        fixture.listener,
        reset_rx,
    )));
    let original_info = timeout(TEST_TIMEOUT, first).await???;
    state.borrow_and_update();
    reject.store(true, Ordering::SeqCst);
    let started = Instant::now();
    reset_tx.send(()).await?;
    let terminal = timeout(TEST_TIMEOUT, async {
        loop {
            state.changed().await?;
            let current = *state.borrow_and_update();
            if matches!(
                current,
                EnvironmentConnectionState::Connected | EnvironmentConnectionState::Disconnected
            ) {
                break anyhow::Ok(current);
            }
        }
    })
    .await??;
    if status == 401 {
        assert_eq!(terminal, EnvironmentConnectionState::Disconnected);
        assert!(matches!(environment.status().await,
            EnvironmentObservedStatus::Disconnected { error }
                if error == "environment registry authentication error: executor credentials rejected"));
        assert_eq!(provider.calls(), 2); // Initial connection plus one rejected recovery.
    } else {
        assert_eq!(terminal, EnvironmentConnectionState::Connected);
        assert_eq!(environment.force_info().await?, original_info);
        assert_eq!(provider.calls(), 5); // Initial, three rejected recoveries, then success.
    }
    assert_eq!(validations.load(Ordering::SeqCst), provider.calls());
    let requests = fixture
        .registry
        .received_requests()
        .await
        .context("requests")?;
    let validations: Vec<_> = requests
        .iter()
        .filter(|r| r.url.path().ends_with("/validate"))
        .collect();
    let bearers: BTreeSet<_> = validations
        .iter()
        .map(|r| r.headers.get("authorization").unwrap())
        .collect();
    let grants: BTreeSet<_> = validations.iter().map(|r| {
        serde_json::from_slice::<serde_json::Value>(&r.body).unwrap()["harness_key_authorization"].to_string()
    }).collect();
    assert_eq!(bearers.len(), 1);
    assert_eq!(grants.len(), validations.len());
    println!(
        "status={status}, recovery_attempts={}, elapsed_ms={}, terminal={terminal:?}",
        provider.calls() - 1,
        started.elapsed().as_millis()
    );
    assert!(!router.is_finished() && !remote.is_finished());
    router.abort();
    let _ = router.await;
    let _ = shutdown.send(());
    timeout(TEST_TIMEOUT, remote).await???;
    Ok(())
}
