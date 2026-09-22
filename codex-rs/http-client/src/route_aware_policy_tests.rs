//! Unavailable policy rejects requests before a socket is opened.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn unavailable_policy_does_not_connect() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url = format!("https://{}/", listener.local_addr().unwrap());
    let controller = crate::NetworkPolicyController::default();
    let factory = HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
        .with_network_policy(controller.policy());
    let client = factory.build_client(&url, ClientRouteClass::Api).unwrap();
    assert!(matches!(
        client.get(&url).send().await,
        Err(RouteAwareRequestError::Policy(
            NetworkPolicyDenied::Unavailable
        ))
    ));
    assert_eq!(
        listener.accept().unwrap_err().kind(),
        std::io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn account_change_cancels_redirect_route_selection() {
    let (address, server) = super::tests::spawn_response_server(vec![
        "HTTP/1.1 302 Found\r\nLocation: /final\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
            .into(),
    ]);
    let controller = crate::NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(policy.revision(), crate::DestinationPolicy::Unrestricted);
    let pool = RouteAwareClientPool::new(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy.clone()),
        ClientRouteClass::Api,
    );
    let request = reqwest::Request::new(
        Method::GET,
        reqwest::Url::parse(&format!("http://{address}/start")).unwrap(),
    );
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 5),
        pool.send_with_resolver(request, |url| {
            let policy = policy.clone();
            async move {
                if url.ends_with("/final") {
                    policy.invalidate();
                    return std::future::pending().await;
                }
                Ok(OutboundProxyRoute::Direct)
            }
        }),
    )
    .await
    .unwrap();
    assert!(matches!(
        result,
        Err(RouteAwareRequestError::Policy(NetworkPolicyDenied::Revoked))
    ));
    assert_eq!(server.join().unwrap().len(), 1);
}

#[tokio::test]
async fn managed_redirect_failure_is_tracked_before_proxy_retry() {
    let unused = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let dead_address = unused.local_addr().unwrap();
    drop(unused);
    let (address, server) = super::tests::spawn_response_server(vec![format!(
        "HTTP/1.1 307 Temporary Redirect\r\nLocation: http://{dead_address}/token\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
    )]);
    let controller = crate::NetworkPolicyController::default();
    let policy = controller.policy();
    controller.publish(policy.revision(), crate::DestinationPolicy::Unrestricted);
    let redirected = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(/*v*/ false));
    let pool = RouteAwareClientPool::with_builder(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault)
            .with_network_policy(policy)
            .with_system_proxy_fallback(),
        ClientRouteClass::Auth,
        HttpClientBuilder::new().with_redirect_tracking(redirected.clone()),
    );
    let result = pool
        .post(format!("http://{address}/token"))
        .body("code=one-time-code")
        .timeout(std::time::Duration::from_secs(/*secs*/ 5))
        .send()
        .await;
    assert!(result.is_err());
    assert!(redirected.load(std::sync::atomic::Ordering::Relaxed));
    assert_eq!(server.join().unwrap().len(), 1);
}
