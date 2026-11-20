#![cfg(test)]

//! Tests config-server OAuth defaults, discovery activation, and routing.

use super::DISCOVERY_TIMEOUT;
use super::discover_oauth;
use super::resolve_config_server;
use codex_config::ConfigServer;
use codex_login::AuthRouteConfig;
use codex_login::OAuthSessionConfig;
use codex_login::test_support::transport_default_auth_route_config;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::io;
use std::net::TcpListener;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::path;

fn metadata(issuer: &str) -> Value {
    json!({
        "issuer": issuer,
        "authorization_endpoint": "https://login.example/authorize",
        "token_endpoint": "https://login.example/token",
        "scopes_supported": ["openid", "profile"],
    })
}

#[tokio::test]
async fn applies_company_defaults_and_optional_device_endpoint() {
    let server = MockServer::start().await;
    for device_url in [None, Some("https://login.example/device")] {
        server.reset().await;
        let mut metadata = metadata(&server.uri());
        if let Some(device_url) = device_url {
            metadata["device_authorization_endpoint"] = json!(device_url);
        }
        Mock::given(path("/.well-known/oauth-authorization-server"))
            .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
            .expect(1)
            .mount(&server)
            .await;
        assert_eq!(
            discover_oauth(&server.uri(), &transport_default_auth_route_config())
                .await
                .unwrap(),
            Some(OAuthSessionConfig {
                authorization_url: "https://login.example/authorize".into(),
                token_url: "https://login.example/token".into(),
                device_authorization_url: device_url.map(str::to_owned),
                client_id: "codex".into(),
                resource: None,
                scopes: Vec::new(),
                redirect_port: None,
                credentials_store: Default::default(),
            })
        );
    }
}

#[tokio::test]
async fn rejects_invalid_device_endpoint() {
    let server = MockServer::start().await;
    let mut metadata = metadata(&server.uri());
    metadata["device_authorization_endpoint"] = json!(["secret"]);
    Mock::given(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata))
        .expect(1)
        .mount(&server)
        .await;
    let error = discover_oauth(&server.uri(), &transport_default_auth_route_config())
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "OAuth discovery device endpoint is invalid"
    );
}

#[tokio::test]
async fn bounds_company_discovery_duration() {
    let server = MockServer::start().await;
    Mock::given(path("/.well-known/oauth-authorization-server"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(metadata(&server.uri()))
                .set_delay(DISCOVERY_TIMEOUT + Duration::from_secs(1)),
        )
        .expect(1)
        .mount(&server)
        .await;
    let error = discover_oauth(&server.uri(), &transport_default_auth_route_config())
        .await
        .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
}

#[tokio::test]
async fn preserves_explicit_oauth_without_discovery() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let routes = transport_default_auth_route_config();
    let explicit_oauth = OAuthSessionConfig {
        authorization_url: String::from("https://login.example/authorize"),
        token_url: String::from("https://login.example/token"),
        device_authorization_url: None,
        client_id: String::from("existing-client"),
        resource: Some(String::from("existing-resource")),
        scopes: vec![String::from("existing-scope")],
        redirect_port: Some(1455),
        credentials_store: Default::default(),
    };

    let config = ConfigServer {
        base_url: format!("https://{address}"),
        oauth: Some(Box::new(explicit_oauth)),
    };
    assert_eq!(
        resolve_config_server(&config, &routes).await.unwrap(),
        config
    );
    let home = tempfile::tempdir().unwrap();
    crate::anonymous_config_server_bundle_loader(&config, home.path().into(), &routes)
        .await
        .expect_err("Git must refuse explicit OAuth without fetching anonymous policy");
    assert_eq!(
        listener
            .accept()
            .expect_err("OAuth discovery should be skipped")
            .kind(),
        io::ErrorKind::WouldBlock
    );
}

#[tokio::test]
async fn discovery_obeys_local_network_policy_without_requiring_fetched_policy() {
    let server = MockServer::start().await;
    let allowed = transport_default_auth_route_config()
        .http_client_factory()
        .clone();
    let blocked = allowed
        .clone()
        .with_network_policy(codex_http_client::NetworkPolicyController::default().policy());
    let routes = AuthRouteConfig::from_http_client_factory(blocked.clone())
        .with_local_bootstrap_factory(allowed.clone());
    Mock::given(path("/.well-known/oauth-authorization-server"))
        .respond_with(ResponseTemplate::new(200).set_body_json(metadata(&server.uri())))
        .expect(1)
        .mount(&server)
        .await;
    assert!(
        discover_oauth(&server.uri(), &routes)
            .await
            .unwrap()
            .is_some()
    );

    server.reset().await;
    let routes =
        AuthRouteConfig::from_http_client_factory(allowed).with_local_bootstrap_factory(blocked);
    assert!(discover_oauth(&server.uri(), &routes).await.is_err());
    assert!(server.received_requests().await.unwrap().is_empty());
}
