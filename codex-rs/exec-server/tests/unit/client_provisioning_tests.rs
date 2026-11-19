//! Exercises provisioning recovery while startup is publishing its previous failure.

use super::*;
use crate::client_api::Deferred;
use crate::noise_channel::NoiseChannelIdentity;
use futures::poll;
use tokio::sync::oneshot;

#[test_case::test_case(false; "initial startup")]
#[test_case::test_case(true; "subsequent attempt")]
#[tokio::test]
async fn caller_after_ready_retries_an_unpublished_provisioning_failure(reconnecting: bool) {
    let (readiness, readiness_rx) = watch::channel(Some(Err("first failure".to_string())));
    let client = LazyRemoteExecServerClient::new(
        ExecServerTransportParams::Deferred(Box::new(Deferred {
            readiness: readiness_rx,
            transport: ExecServerTransportParams::NoiseRendezvous {
                provider: Arc::new(FailingProvider),
                identity: NoiseChannelIdentity::generate().expect("Noise identity"),
            },
        })),
        HttpClientFactory::new(codex_http_client::OutboundProxyPolicy::ReqwestDefault),
    );
    let (failed_tx, failed_rx) = oneshot::channel();
    let (publish_tx, publish_rx) = oneshot::channel();
    let attempt = if reconnecting {
        assert!(client.wait_until_ready().await.is_err());
        let attempt = Arc::new(ConnectionAttempt::default());
        *client.reconnect.lock().expect("reconnect lock") = Some(Arc::clone(&attempt));
        attempt
    } else {
        Arc::clone(&client.startup)
    };
    let startup_client = client.clone();
    let startup = tokio::spawn(async move {
        attempt
            .result
            .get_or_init(|| async {
                let result = startup_client.connect_with_retry(&attempt).await;
                assert!(result.is_err());
                failed_tx.send(()).expect("failure observed");
                // Suspend at the publication boundary, as another executor thread could.
                publish_rx.await.expect("publish startup result");
                result
            })
            .await
            .clone()
    });
    failed_rx.await.expect("startup consumed the Failed report");
    readiness.send_replace(Some(Ok(())));

    let mut after_ready = Box::pin(client.wait_until_ready());
    assert!(poll!(&mut after_ready).is_pending());
    publish_tx.send(()).expect("release startup");
    let error = after_ready.await.unwrap_err();
    assert!(
        error.to_string().contains("provider reached after Ready"),
        "post-Ready caller should retry the stale failure: {error}"
    );
    assert!(startup.await.expect("startup task").is_err());
}

struct FailingProvider;

impl crate::NoiseRendezvousConnectProvider for FailingProvider {
    fn connect_bundle(
        &self,
        _: crate::NoiseChannelPublicKey,
    ) -> BoxFuture<'_, Result<crate::NoiseRendezvousConnectBundle, ExecServerError>> {
        Box::pin(async {
            Err(ExecServerError::Protocol(
                "provider reached after Ready".to_string(),
            ))
        })
    }
}
