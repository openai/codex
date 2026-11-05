//! Runtime-authenticated catalogs keep presigned bundle transfers unauthenticated.

use super::*;
use codex_features::RuntimeAuthMode;
use codex_http_client::HttpClientFactory;
use codex_http_client::OutboundProxyPolicy;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

#[tokio::test]
async fn runtime_bundle_download_follows_redirect_without_actor_auth() {
    let origin = MockServer::start().await;
    let destination = MockServer::start().await;
    let final_url = format!("{}/bundle", destination.uri());
    Mock::given(path("/bundle"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", &final_url))
        .expect(1)
        .mount(&origin)
        .await;
    Mock::given(path("/bundle"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bundle"))
        .expect(1)
        .mount(&destination)
        .await;
    let config = RemotePluginServiceConfig::new_with_selected_runtime_auth(
        origin.uri(),
        HttpClientFactory::new(OutboundProxyPolicy::ReqwestDefault),
        /*product_sku*/ None,
        (Default::default(), RuntimeAuthMode::AssertionOnly),
    );
    let error = download_remote_plugin_bundle_with_limit(
        &config,
        &format!("{}/bundle", origin.uri()),
        /*max_bytes*/ 64,
    )
    .await
    .expect_err("HTTP final URLs remain unsupported after following redirects");
    assert!(
        matches!(
            &error,
            RemotePluginBundleInstallError::UnsupportedBundleDownloadFinalUrl { .. }
        ),
        "expected rejection of the HTTP redirect destination: {error}"
    );
    let mut requests = origin.received_requests().await.unwrap();
    requests.extend(destination.received_requests().await.unwrap());
    assert_eq!(requests.len(), 2);
    for request in requests {
        assert!(!request.headers.contains_key("authorization"));
        assert!(!request.headers.contains_key("x-openai-agent-assertion"));
    }
}
