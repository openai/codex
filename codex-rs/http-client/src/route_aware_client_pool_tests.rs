use std::sync::Arc;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use pretty_assertions::assert_eq;

use super::*;
use crate::OutboundProxyPolicy;

#[tokio::test]
async fn reuses_client_for_urls_with_the_same_route() {
    let builder_count = Arc::new(AtomicUsize::new(0));
    let observed_builder_count = Arc::clone(&builder_count);
    let pool = RouteAwareClientPool::with_builder_factory(
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        ClientRouteClass::Api,
        move || {
            observed_builder_count.fetch_add(1, Ordering::SeqCst);
            reqwest::Client::builder()
        },
    );

    pool.client_for_url("https://example.com/first")
        .await
        .expect("first client should build");
    pool.client_for_url("https://example.com/second")
        .await
        .expect("second client should reuse the route");

    assert_eq!(builder_count.load(Ordering::SeqCst), 1);
}
