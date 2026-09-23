//! Destination admission and account revocation cover the gRPC request lifetime.

use codex_http_client::DestinationPolicy;
use codex_http_client::HttpClientFactory;
use codex_http_client::NetworkPolicyController;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;

use super::*;

#[tokio::test]
async fn restricted_destination_is_rejected_before_connecting_to_the_code_mode_host() -> Result<()>
{
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let controller = NetworkPolicyController::default();
    assert!(controller.publish(
        controller.policy().revision(),
        DestinationPolicy::Restricted {
            allowed_hosts: ["allowed.example".into()].into()
        }
    ));
    let provider = Arc::new(GrpcCodeModeSessionProvider::with_http_client_factory(
        format!("https://{}", listener.local_addr()?),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(controller.policy()),
    ));
    let Err(error) = provider.create_session().await else {
        anyhow::bail!("restricted endpoint created a session")
    };
    assert!(
        error.contains("destination denied by application network policy"),
        "{error}"
    );
    assert!(
        timeout(Duration::from_millis(/*millis*/ 50), listener.accept())
            .await
            .is_err()
    );

    Ok(())
}

#[tokio::test]
async fn account_change_revokes_active_rpc_and_renews_the_cached_client() -> Result<()> {
    let host = HostHarness::start("grpc://127.0.0.1:0").await?;
    let controller = NetworkPolicyController::default();
    assert!(controller.publish(
        controller.policy().revision(),
        DestinationPolicy::Unrestricted
    ));
    let provider = GrpcCodeModeSessionProvider::with_http_client_factory(
        host.endpoint.clone(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(controller.policy()),
    );
    let session = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let mut pending = request("await new Promise(() => {});");
    pending.yield_time_ms = Some(60_000);
    let started = session
        .execute(pending, Arc::new(NoopCodeModeSessionDelegate))
        .await
        .map_err(anyhow::Error::msg)?;

    controller.policy().invalidate();
    assert!(
        timeout(TEST_TIMEOUT, started.initial_response())
            .await
            .context("account revocation did not interrupt the active RPC")?
            .is_err()
    );
    assert!(controller.publish(
        controller.policy().revision(),
        DestinationPolicy::Unrestricted
    ));

    let replacement = provider
        .create_session()
        .await
        .map_err(anyhow::Error::msg)?;
    let response = execute(
        &replacement,
        request(r#"text("replacement")"#),
        Arc::new(NoopCodeModeSessionDelegate),
    )
    .await?;
    assert_eq!(
        response,
        text_response("1", "replacement", response.code_mode_host_duration())
    );
    session.shutdown().await.map_err(anyhow::Error::msg)?;
    replacement.shutdown().await.map_err(anyhow::Error::msg)?;
    Ok(())
}
