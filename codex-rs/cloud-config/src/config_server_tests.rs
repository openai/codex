#![cfg(test)]

use super::*;
use crate::bundle_loader::managed_config_bundle_loader_for_service;
use crate::service::CLOUD_CONFIG_BUNDLE_TIMEOUT;
use crate::service::CloudConfigBundleService;
use codex_config::CloudConfigBundleLoader;
use codex_config::ConfigServer;
use codex_config::test_support::CloudConfigBundleFixture;
use codex_keyring_store::tests::MockKeyringStore;
use codex_login::test_support::transport_default_auth_route_config;
use pretty_assertions::assert_eq;
use serde_json::json;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use tempfile::tempdir;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

// HTTP fixtures exercise the same service while production enforces HTTPS at construction.
fn remote_loader(url: String, codex_home: PathBuf) -> CloudConfigBundleLoader {
    let factory = transport_default_auth_route_config()
        .http_client_factory()
        .clone();
    let source = ConfigServerSource::new(url, /*oauth*/ None, codex_home.clone(), factory).unwrap();
    managed_config_bundle_loader_for_service(CloudConfigBundleService::from_source(
        source,
        codex_home,
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    ))
    .0
}

fn oauth_config(base_url: &str) -> OAuthSessionConfig {
    OAuthSessionConfig {
        authorization_url: format!("{base_url}/authorize"),
        token_url: format!("{base_url}/token"),
        device_authorization_url: None,
        client_id: "company-client".into(),
        resource: None,
        scopes: vec![],
        redirect_port: None,
    }
}

#[tokio::test]
async fn policy_fetch_requires_config_server_sign_in() {
    let server = MockServer::start().await;
    let home = tempdir().unwrap();
    let endpoint = format!("https://{}", server.address());
    let factory = transport_default_auth_route_config()
        .http_client_factory()
        .clone();
    let loader = config_server_bundle_loader(
        ConfigServer {
            base_url: format!("{endpoint}/settings"),
            oauth: Some(Box::new(oauth_config(&endpoint))),
        },
        home.path().to_path_buf(),
        factory,
    );
    let error = loader.get().await.unwrap_err();
    assert_eq!(error.code(), CloudConfigBundleLoadErrorCode::Auth);
    assert_eq!(
        error.to_string(),
        "Sign in to the configuration server to load your organization's settings"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn policy_fetch_uses_replacement_config_server_credentials() {
    let server = MockServer::start().await;
    let sign_in = AtomicUsize::new(0);
    Mock::given(path("/token"))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_json(json!({
                "access_token": format!("account-{}", sign_in.fetch_add(1, Ordering::Relaxed)),
                "token_type": "Bearer", "expires_in": 3600,
            }))
        })
        .mount(&server)
        .await;
    let policies = ["never", "on-request"].map(|approval| {
        CloudConfigBundleFixture::enterprise_requirement(format!(
            "allowed_approval_policies = ['{approval}']"
        ))
        .into_bundle()
    });
    for (account, policy) in policies.iter().enumerate() {
        Mock::given(path("/settings/codex/config/bundle"))
            .and(header("authorization", format!("Bearer account-{account}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(policy))
            .mount(&server)
            .await;
    }
    let home = tempdir().unwrap();
    let factory = transport_default_auth_route_config()
        .http_client_factory()
        .clone();
    let oauth = oauth_config(&server.uri());
    let keyring = Arc::new(MockKeyringStore::default());
    let manager = || {
        OAuthSessionManager::new(
            oauth.clone(),
            home.path().to_path_buf(),
            &factory,
            keyring.clone(),
        )
        .unwrap()
    };
    let gateway = manager();
    gateway
        .login_with_browser(std::future::pending(), complete_browser_authorization)
        .await
        .unwrap();
    let source = ConfigServerSource {
        cache_namespace: "test".into(),
        client: BackendClient::for_config_endpoint(
            format!("{}/settings", server.uri()),
            factory.clone(),
        ),
        auth: Some(manager().without_interactive_login()),
    };
    let service = CloudConfigBundleService::from_source(
        source,
        home.path().to_path_buf(),
        CLOUD_CONFIG_BUNDLE_TIMEOUT,
    );
    assert_eq!(
        service.load_startup_bundle_with_timeout().await.unwrap(),
        Some(policies[0].clone())
    );

    gateway
        .login_with_browser(std::future::pending(), complete_browser_authorization)
        .await
        .unwrap();
    // The old access token is still valid, so this must detect the changed grant.
    assert_eq!(
        service.load_startup_bundle_with_timeout().await.unwrap(),
        Some(policies[1].clone())
    );
}

fn complete_browser_authorization(url: &wiremock::http::Url) {
    let query = url
        .query_pairs()
        .collect::<std::collections::HashMap<_, _>>();
    let callback = format!(
        "{}?code=browser-authorization-code&state={}",
        query["redirect_uri"], query["state"]
    );
    tokio::spawn(async move {
        codex_login::auth::default_client::create_client_without_request_logging()
            .get(callback)
            .send()
            .await
            .expect("OAuth callback");
    });
}

#[tokio::test]
async fn cached_policy_is_reused_only_for_the_same_endpoint() {
    let server = MockServer::start().await;
    let home = tempdir().unwrap();
    let endpoint = format!("{}/settings", server.uri());
    let loader = || remote_loader(endpoint.clone(), home.path().to_path_buf());
    let bundle =
        CloudConfigBundleFixture::enterprise_requirement("allowed_approval_policies = ['never']")
            .into_bundle();
    Mock::given(method("GET"))
        .and(path("/settings/codex/config/bundle"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(&bundle))
        .mount(&server)
        .await;
    let first = loader();
    assert_eq!(first.get().await.unwrap(), Some(bundle.clone()));
    drop(first);
    server.reset().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(/*s*/ 403))
        .mount(&server)
        .await;
    assert_eq!(loader().get().await.unwrap(), Some(bundle));
    let different = remote_loader(
        format!("{}/different", server.uri()),
        home.path().to_path_buf(),
    );
    assert_eq!(different.get().await.unwrap_err().status_code(), Some(403));
}
