#![cfg(test)]

use super::*;
use crate::ConfigServerLoginMethod;
use crate::bundle_loader::ensure_config_server_login_with_manager;
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
use std::time::Duration;
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
        credentials_store: Default::default(),
    }
}

#[tokio::test]
async fn a_supplied_chatgpt_token_does_not_replace_config_server_sign_in() {
    let server = MockServer::start().await;
    let home = tempdir().unwrap();
    let endpoint = format!("https://{}", server.address());
    let mut config = codex_core::config::ConfigBuilder::default()
        .codex_home(home.path().into())
        .build()
        .await
        .unwrap()
        .auth_config();
    config.chatgpt_base_url = Some(server.uri());
    let requirements = codex_config::ConfigRequirementsToml {
        config_server: Some(ConfigServer {
            base_url: format!("{endpoint}/settings"),
            oauth: Some(Box::new(oauth_config(&endpoint))),
        }),
        ..Default::default()
    };
    let chatgpt = codex_login::CodexAuth::from_external_chatgpt_tokens(
        "e30.e30.c2ln",
        "endor",
        /*chatgpt_plan_type*/ None,
    )
    .unwrap();
    let loader = crate::managed_config_bundle_loader_for_auth(config, &chatgpt, &requirements)
        .await
        .unwrap();
    assert_eq!(loader.config_server(), requirements.config_server.as_ref());
    let error = loader.get().await.unwrap_err();
    assert_eq!(error.code(), CloudConfigBundleLoadErrorCode::Auth);
    assert_eq!(
        error.to_string(),
        "Sign in to the configuration server to load your organization's settings"
    );
    assert!(server.received_requests().await.unwrap().is_empty());
}

#[tokio::test]
async fn device_endpoint_failure_points_to_config_server_oauth() {
    let server = MockServer::start().await;
    let home = tempdir().unwrap();
    let oauth = OAuthSessionConfig {
        device_authorization_url: Some(format!("{}/missing", server.uri())),
        ..oauth_config(&server.uri())
    };
    let manager = OAuthSessionManager::new(
        oauth,
        home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        Arc::new(MockKeyringStore::default()),
    )
    .unwrap();
    let error = ensure_config_server_login_with_manager(
        &manager,
        ConfigServerLoginMethod::DeviceCode {
            present: |_| panic!("unexpected device code"),
        },
    )
    .await
    .unwrap_err();
    assert_eq!(
        error.to_string(),
        "Config server sign-in failed. Check config_server.oauth and the credential store."
    );
}

#[tokio::test]
async fn policy_fetch_uses_replacement_config_server_credentials() {
    let server = MockServer::start().await;
    let resource = "https://config.example/audience";
    let resource_field = b"resource=https%3A%2F%2Fconfig.example%2Faudience";
    Mock::given(method("POST"))
        .and(path("/oauth/device"))
        .and(form_has_field(resource_field))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "device-code",
            "user_code": "ABCD-EFGH",
            "verification_uri": "https://login.example/device",
            "expires_in": 60,
            "interval": 1,
        })))
        .expect(1)
        .mount(&server)
        .await;
    let sign_in = AtomicUsize::new(0);
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(form_has_field(resource_field))
        .respond_with(move |_: &wiremock::Request| {
            ResponseTemplate::new(200).set_body_json(json!({
                "access_token": format!("account-{}", sign_in.fetch_add(1, Ordering::Relaxed)),
                "token_type": "Bearer", "expires_in": 3600,
            }))
        })
        .expect(2)
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
    let oauth = OAuthSessionConfig {
        device_authorization_url: Some(format!("{}/oauth/device", server.uri())),
        resource: Some(resource.into()),
        ..oauth_config(&server.uri())
    };
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
    let auth = manager().without_interactive_login();
    tokio::time::timeout(
        Duration::from_secs(20),
        ensure_config_server_login_with_manager(
            &auth,
            ConfigServerLoginMethod::DeviceCode { present: |_| {} },
        ),
    )
    .await
    .expect("device sign-in should complete")
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

    auth.login_with_browser(std::future::pending(), complete_browser_authorization)
        .await
        .unwrap();
    // The old access token is still valid, so this must detect the changed grant.
    assert_eq!(
        service.load_startup_bundle_with_timeout().await.unwrap(),
        Some(policies[1].clone())
    );
    server.verify().await;
}

fn form_has_field(field: &'static [u8]) -> impl wiremock::Match {
    move |request: &wiremock::Request| {
        request
            .body
            .split(|byte| *byte == b'&')
            .any(|pair| pair == field)
    }
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
