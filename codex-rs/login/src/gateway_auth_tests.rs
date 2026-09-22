//! Exercises gateway authorization, credential recovery, and persistence across manager lifetimes.

use std::collections::HashMap;
use std::net::TcpListener;
use std::sync::Arc;
use std::time::Duration;

use chrono::Utc;
use codex_keyring_store::tests::MockKeyringStore;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::any;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::method;
use wiremock::matchers::path;

use super::GatewayAuthConfig;
use super::GatewayAuthManager;
use super::StoredToken;
use super::callback::CallbackListener;
use crate::test_support::transport_default_auth_route_config;

fn client(
    config: GatewayAuthConfig,
    keyring: Arc<MockKeyringStore>,
) -> (GatewayAuthManager, tempfile::TempDir) {
    let home = tempfile::tempdir().expect("Codex home");
    let manager = GatewayAuthManager::new(
        config,
        home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        keyring,
    )
    .expect("gateway auth manager");
    (manager, home)
}

fn config(server: &MockServer) -> GatewayAuthConfig {
    GatewayAuthConfig {
        authorization_url: format!("{}/authorize", server.uri()),
        token_url: format!("{}/token", server.uri()),
        client_id: "codex-test".to_string(),
        resource: Some("https://gateway.example.test/codex".to_string()),
        scopes: vec!["openid".to_string(), "gateway.inference".to_string()],
        redirect_port: None,
    }
}

fn loopback_config() -> GatewayAuthConfig {
    GatewayAuthConfig {
        authorization_url: "http://127.0.0.1:18080/authorize".to_string(),
        token_url: "http://127.0.0.1:18080/token".to_string(),
        client_id: "codex-test".to_string(),
        resource: None,
        scopes: Vec::new(),
        redirect_port: None,
    }
}

fn save_token(
    client: &GatewayAuthManager,
    access_token: &str,
    refresh_token: Option<&str>,
    expires_at: i64,
) {
    let token = StoredToken {
        access_token: access_token.to_string(),
        refresh_token: refresh_token.map(str::to_string),
        expires_at: Some(expires_at),
    };
    client
        .save_token(&token)
        .expect("save provider OAuth token");
}

fn complete_browser_authorization(authorization_url: &url::Url) {
    let query = authorization_url
        .query_pairs()
        .into_owned()
        .collect::<HashMap<_, _>>();
    let redirect_uri = query.get("redirect_uri").expect("redirect URI").clone();
    let state = query.get("state").expect("OAuth state").clone();
    tokio::spawn(async move {
        crate::auth::default_client::create_client_without_request_logging()
            .get(format!(
                "{redirect_uri}?code=browser-authorization-code&state={state}"
            ))
            .send()
            .await
            .expect("OAuth callback");
    });
}

#[tokio::test]
async fn reloads_replaced_credentials_without_a_cached_refresh_token() {
    let keyring = Arc::new(MockKeyringStore::default());
    let (client, _home) = client(loopback_config(), keyring.clone());
    save_token(
        &client,
        "cached-access-token",
        /*refresh_token*/ None,
        Utc::now().timestamp() + 3_600,
    );

    assert_eq!(
        client
            .resolve_access_token()
            .await
            .expect("cached access token"),
        "cached-access-token"
    );
    save_token(
        &client,
        "external-login",
        /*refresh_token*/ None,
        Utc::now().timestamp() + 3_600,
    );
    assert_eq!(
        client
            .refresh_access_token("cached-access-token")
            .await
            .expect("reload without a refresh token"),
        "external-login"
    );
}

#[tokio::test]
async fn expired_tokens_refresh_once_when_the_response_omits_rotation_and_expiry() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=refresh_token"))
        .and(body_string_contains("refresh_token=original-refresh"))
        .and(body_string_contains("resource="))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "refreshed-access-token",
            "token_type": "Bearer",
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let keyring = Arc::new(MockKeyringStore::default());
    let (client, _home) = client(config(&server), keyring.clone());
    save_token(
        &client,
        "expired-access-token",
        Some("original-refresh"),
        Utc::now().timestamp() - 1,
    );

    let other = GatewayAuthManager::new(
        client.state.config.clone(),
        client.state.codex_home.clone(),
        transport_default_auth_route_config().http_client_factory(),
        keyring.clone(),
    )
    .expect("independent gateway auth manager");
    let (access_token, other_access_token) =
        tokio::try_join!(client.resolve_access_token(), other.resolve_access_token(),)
            .expect("one refresh shared by both handles");
    assert_eq!(
        [access_token.as_str(), other_access_token.as_str()],
        ["refreshed-access-token"; 2]
    );
    assert_eq!(
        client
            .resolve_access_token()
            .await
            .expect("cached token without expiry"),
        "refreshed-access-token"
    );
    assert_eq!(
        serde_json::to_value(client.load_token().expect("persisted token"))
            .expect("stored credential"),
        json!({"access_token": "refreshed-access-token", "refresh_token": "original-refresh"})
    );
}

#[tokio::test]
async fn different_configurations_refresh_under_the_same_store_lock() {
    let server = MockServer::start().await;
    let keyring = Arc::new(MockKeyringStore::default());
    let (first, home) = client(config(&server), keyring.clone());
    let mut other_config = first.state.config.clone();
    other_config.client_id = "other-client".to_string();
    let second = GatewayAuthManager::new(
        other_config,
        home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        keyring,
    )
    .expect("independent configuration");
    for (manager, refresh_token) in [(&first, "refresh-a"), (&second, "refresh-b")] {
        save_token(
            manager,
            "expired-access",
            Some(refresh_token),
            Utc::now().timestamp() - 1,
        );
    }

    let contender = Arc::new(
        super::storage::lock_credentials(home.path())
            .await
            .expect("store lock"),
    );
    contender.unlock().expect("release store lock");
    for (client_id, refresh_token, access_token) in [
        ("codex-test", "refresh-a", "access-a"),
        ("other-client", "refresh-b", "access-b"),
    ] {
        let contender = Arc::clone(&contender);
        Mock::given(method("POST"))
            .and(body_string_contains(format!("client_id={client_id}")))
            .and(body_string_contains(format!(
                "refresh_token={refresh_token}"
            )))
            .respond_with(move |_: &wiremock::Request| {
                assert!(matches!(
                    contender.try_lock(),
                    Err(std::fs::TryLockError::WouldBlock)
                ));
                ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "access_token": access_token,
                    "refresh_token": format!("rotated-{refresh_token}"),
                }))
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
    }
    let tokens = tokio::try_join!(first.resolve_access_token(), second.resolve_access_token())
        .expect("both configurations refresh");
    assert_eq!(tokens, ("access-a".to_string(), "access-b".to_string()));
    assert_eq!(
        serde_json::to_value([
            first.load_token().expect("first persisted credential"),
            second.load_token().expect("second persisted credential"),
        ])
        .expect("stored credentials"),
        json!([
            {"access_token": "access-a", "refresh_token": "rotated-refresh-a"},
            {"access_token": "access-b", "refresh_token": "rotated-refresh-b"},
        ])
    );
}

#[tokio::test]
async fn rejected_token_refresh_is_shared_and_late_rejections_reuse_the_latest_token() {
    let server = MockServer::start().await;
    for (refresh_token, access_token, next_refresh_token) in [
        ("refresh-a", "access-b", "refresh-b"),
        ("refresh-b", "access-c", "refresh-c"),
    ] {
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains(format!(
                "refresh_token={refresh_token}"
            )))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "access_token": access_token,
                "refresh_token": next_refresh_token,
                "expires_in": 3600,
            })))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
    }
    let keyring = Arc::new(MockKeyringStore::default());
    let (client, _home) = client(config(&server), keyring.clone());
    save_token(
        &client,
        "access-a",
        Some("refresh-a"),
        Utc::now().timestamp() + 3_600,
    );
    assert_eq!(
        client.resolve_access_token().await.expect("initial token"),
        "access-a"
    );
    let cloned = client.clone();
    let (first, second) = tokio::try_join!(
        client.refresh_access_token("access-a"),
        cloned.refresh_access_token("access-a"),
    )
    .expect("one refresh for concurrent rejections");
    assert_eq!([first.as_str(), second.as_str()], ["access-b"; 2]);
    assert_eq!(
        cloned
            .refresh_access_token("access-b")
            .await
            .expect("B itself was rejected"),
        "access-c"
    );
    for rejected_access_token in ["access-a", "access-b"] {
        assert_eq!(
            client
                .refresh_access_token(rejected_access_token)
                .await
                .expect("late rejection"),
            "access-c"
        );
    }
}

#[tokio::test]
async fn refresh_does_not_reuse_expired_or_rejected_persisted_replacements() {
    for (rejected_access_token, expires_at) in [
        ("access-a", Utc::now().timestamp() - 1),
        ("access-b", Utc::now().timestamp() + 3_600),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(body_string_contains("refresh_token=refresh-b"))
            .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "access_token": "access-c", "refresh_token": "refresh-c",
            })))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let keyring = Arc::new(MockKeyringStore::default());
        let (client, _home) = client(config(&server), keyring.clone());
        save_token(
            &client,
            "access-a",
            Some("refresh-a"),
            Utc::now().timestamp() + 3_600,
        );
        assert_eq!(
            client.resolve_access_token().await.expect("cached A"),
            "access-a"
        );
        save_token(&client, "access-b", Some("refresh-b"), expires_at);

        assert_eq!(
            client
                .refresh_access_token(rejected_access_token)
                .await
                .expect("refresh B"),
            "access-c"
        );
        assert_eq!(
            serde_json::to_value(client.load_token().expect("saved replacement"))
                .expect("stored credential"),
            json!({"access_token": "access-c", "refresh_token": "refresh-c"})
        );
    }
}

#[tokio::test]
async fn token_endpoint_failures_do_not_expose_refresh_tokens() {
    let server = MockServer::start().await;
    let refresh_token = r#"secret-"refresh\token"#;
    let form_encoded: String =
        url::form_urlencoded::byte_serialize(refresh_token.as_bytes()).collect();
    let description = format!("{}: {form_encoded}", "x".repeat(/*n*/ 500));
    Mock::given(method("POST"))
        .and(path("/token"))
        .respond_with(
            ResponseTemplate::new(/*s*/ 503)
                .insert_header("x-request-id", "request-123")
                .set_body_json(json!({
                    "error": "temporarily_unavailable",
                    "error_description": description,
                })),
        )
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let keyring = Arc::new(MockKeyringStore::default());
    let (client, _home) = client(config(&server), keyring.clone());
    save_token(
        &client,
        "expired-access-token",
        Some(refresh_token),
        Utc::now().timestamp() - 1,
    );

    let error = client
        .resolve_access_token()
        .await
        .expect_err("surface token endpoint failure");

    let message = error.to_string();
    assert!(message.contains(&format!("{}/token returned HTTP 503", server.uri())));
    assert!(message.contains("[REDACTED]"));
    assert!(message.contains("request id: request-123"));
    assert!(!message.contains("secret-"));
}

#[tokio::test]
async fn query_credentials_are_not_exposed_by_echoed_errors_or_truncated_request_ids() {
    enum QueryLocation {
        AuthorizationEndpoint,
        TokenEndpoint,
        Resource,
    }
    for location in [
        QueryLocation::AuthorizationEndpoint,
        QueryLocation::TokenEndpoint,
        QueryLocation::Resource,
    ] {
        let server = MockServer::start().await;
        let secret = "query-secret-".repeat(/*n*/ 16);
        Mock::given(method("POST"))
            .and(path("/token"))
            .respond_with(
                ResponseTemplate::new(/*s*/ 503)
                    .insert_header("x-request-id", secret.as_str())
                    .set_body_json(
                        json!({"error_description": format!("Unknown input: {secret}")}),
                    ),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let mut oauth = config(&server);
        let url = match location {
            QueryLocation::AuthorizationEndpoint => &mut oauth.authorization_url,
            QueryLocation::TokenEndpoint => &mut oauth.token_url,
            QueryLocation::Resource => oauth.resource.as_mut().expect("configured resource"),
        };
        url.push_str(&format!("?custom_key={secret}"));
        let keyring = Arc::new(MockKeyringStore::default());
        let (manager, _home) = client(oauth, keyring.clone());
        save_token(
            &manager,
            "old-access",
            Some("old-refresh"),
            Utc::now().timestamp() - 1,
        );
        let message = manager
            .resolve_access_token()
            .await
            .expect_err("token endpoint failure")
            .to_string();
        assert!(message.contains("returned HTTP 503"));
        assert!(message.contains("provider response details omitted"));
        assert!(!message.contains("query-secret"));
    }
}

#[tokio::test]
async fn gateway_credentials_use_a_dedicated_encrypted_file_and_reload_large_tokens() {
    let codex_home = tempfile::tempdir().expect("Codex home");
    let keyring = Arc::new(MockKeyringStore::default());
    let access_token = "gateway-token-".repeat(/*n*/ 512);
    let config = loopback_config();
    let client = GatewayAuthManager::new(
        config.clone(),
        codex_home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        keyring.clone(),
    )
    .expect("gateway auth manager");
    client
        .save_token(&StoredToken {
            access_token: access_token.clone(),
            refresh_token: None,
            expires_at: None,
        })
        .expect("save encrypted provider OAuth token");
    assert_eq!(keyring.saved_value(&client.credential_id()), None);
    assert!(
        codex_home
            .path()
            .join("secrets/gateway_oauth.age")
            .is_file()
    );
    assert!(!codex_home.path().join("secrets/codex_auth.age").exists());

    let reloaded = GatewayAuthManager::new(
        config,
        codex_home.path().to_path_buf(),
        transport_default_auth_route_config().http_client_factory(),
        keyring,
    )
    .expect("reloaded gateway auth manager");
    assert_eq!(
        reloaded
            .resolve_access_token()
            .await
            .expect("reload encrypted token"),
        access_token
    );
}

#[tokio::test]
async fn browser_authorization_exchanges_and_persists_under_the_store_lock() {
    let server = MockServer::start().await;
    let mut oauth = config(&server);
    oauth.authorization_url.push_str("?prompt=login");
    let (client, home) = client(oauth, Arc::new(MockKeyringStore::default()));
    let contender = super::storage::lock_credentials(home.path())
        .await
        .expect("store lock");
    contender.unlock().expect("release store lock");
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains("client_id=codex-test"))
        .and(body_string_contains("code=browser-authorization-code"))
        .and(body_string_contains("code_verifier="))
        .and(body_string_contains("redirect_uri="))
        .respond_with(move |_: &wiremock::Request| {
            assert!(matches!(
                contender.try_lock(),
                Err(std::fs::TryLockError::WouldBlock)
            ));
            ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                "access_token": "browser-access-token",
                "refresh_token": "browser-refresh-token",
                "token_type": "Bearer",
            }))
        })
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let mut cached = Arc::clone(&client.state.cached_token).lock_owned().await;
    let token = client
        .authorize_with_browser(&mut cached, |authorization_url| {
            let query = authorization_url
                .query_pairs()
                .into_owned()
                .collect::<HashMap<_, _>>();
            assert_eq!(query.get("prompt").map(String::as_str), Some("login"));
            assert_eq!(
                query.get("scope").map(String::as_str),
                Some("openid gateway.inference")
            );
            complete_browser_authorization(authorization_url);
        })
        .await
        .expect("browser authorization");

    assert_eq!(token, "browser-access-token");
    let expected =
        json!({"access_token": "browser-access-token", "refresh_token": "browser-refresh-token"});
    assert_eq!(
        serde_json::to_value(cached.token.as_ref()).expect("cached token"),
        expected
    );
    assert_eq!(
        serde_json::to_value(client.load_token().expect("persisted token")).expect("stored token"),
        expected
    );
}

#[tokio::test]
async fn token_grants_reject_redirects_without_sending_credentials_to_the_target() {
    enum Grant {
        RefreshToken,
        AuthorizationCode,
    }

    for (status, grant) in [(307, Grant::RefreshToken), (308, Grant::AuthorizationCode)] {
        let server = MockServer::start().await;
        let target = MockServer::start().await;
        Mock::given(any())
            .respond_with(ResponseTemplate::new(/*s*/ 200))
            .expect(/*r*/ 0)
            .mount(&target)
            .await;
        let grant_body = match grant {
            Grant::RefreshToken => "grant_type=refresh_token",
            Grant::AuthorizationCode => "grant_type=authorization_code",
        };
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains(grant_body))
            .respond_with(
                ResponseTemplate::new(status)
                    .insert_header("Location", format!("{}/redirected-token", target.uri())),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let keyring = Arc::new(MockKeyringStore::default());
        let (client, _home) = client(config(&server), keyring.clone());
        let error = match grant {
            Grant::RefreshToken => {
                save_token(
                    &client,
                    "original-access",
                    Some("original-refresh"),
                    Utc::now().timestamp() - 1,
                );
                client
                    .refresh_access_token("original-access")
                    .await
                    .expect_err("refresh redirect")
            }
            Grant::AuthorizationCode => {
                let mut cached = Arc::clone(&client.state.cached_token).lock_owned().await;
                client
                    .authorize_with_browser(&mut cached, complete_browser_authorization)
                    .await
                    .expect_err("code exchange redirect")
            }
        };
        assert!(
            error
                .to_string()
                .contains(&format!("returned HTTP {status}"))
        );
    }
}

#[tokio::test]
async fn ignores_mismatched_callback_state_even_for_provider_errors() {
    let mut listener =
        CallbackListener::new(/*redirect_port*/ None, "expected-state".to_string())
            .expect("callback listener");
    let redirect_uri = listener.redirect_uri().to_string();
    let client = crate::auth::default_client::create_client_without_request_logging();

    for callback in [
        "?error=access_denied&state=wrong-state",
        "?code=untrusted-code&state=wrong-state",
        "?code=untrusted-code&state=expected-state.continue-with-chatgpt",
    ] {
        let response = client
            .get(format!("{redirect_uri}{callback}"))
            .send()
            .await
            .expect("rejected callback response");
        assert_eq!(response.status().as_u16(), 400);
    }

    let response = client
        .get(format!(
            "{redirect_uri}?code=trusted-code&state=expected-state"
        ))
        .send()
        .await
        .expect("valid callback response");
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        listener.wait().await.expect("callback code"),
        "trusted-code"
    );
}

#[tokio::test]
async fn cancelled_callback_wait_releases_its_configured_port() {
    let available_port = TcpListener::bind("127.0.0.1:0").expect("available callback port");
    let port = available_port
        .local_addr()
        .expect("callback address")
        .port();
    drop(available_port);
    let listener =
        CallbackListener::new(Some(port), "expected-state".to_string()).expect("callback listener");

    let callback_wait = tokio::spawn(async move {
        let mut listener = listener;
        listener.wait().await
    });
    callback_wait.abort();
    let _ = callback_wait.await;

    tokio::time::timeout(Duration::from_secs(/*secs*/ 2), async {
        loop {
            if TcpListener::bind(("127.0.0.1", port)).is_ok() {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("cancelled callback released its listener");
}

#[tokio::test]
async fn rejects_insecure_or_credentialed_endpoints() {
    let keyring = Arc::new(MockKeyringStore::default());
    let mut oauth = loopback_config();
    oauth.authorization_url = "http://issuer.example.test/authorize".to_string();
    let (invalid_authorization, _home) = client(oauth, keyring.clone());
    assert!(invalid_authorization.resolve_access_token().await.is_err());

    let mut oauth = loopback_config();
    oauth.token_url = "https://user:secret@issuer.example.test/token".to_string();
    let (invalid_token, _home) = client(oauth, keyring);
    assert!(invalid_token.resolve_access_token().await.is_err());
}

#[test]
fn accepts_ipv4_and_ipv6_loopback_endpoints() {
    for endpoint in [
        "http://127.0.0.1/token",
        "http://127.42.0.1/token",
        "http://[::1]/token",
        "http://localhost/token",
    ] {
        assert!(
            super::validate_oauth_url(endpoint, "provider OAuth token endpoint").is_ok(),
            "loopback endpoint was rejected: {endpoint}"
        );
    }
    assert!(
        super::validate_oauth_url("http://[::2]/token", "provider OAuth token endpoint").is_err()
    );
}

#[tokio::test]
async fn cancelled_refresh_still_persists_and_caches_rotated_credentials() {
    use tokio::io::AsyncBufReadExt;
    use tokio::io::AsyncReadExt;
    use tokio::io::AsyncWriteExt;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("token listener");
    let endpoint = format!(
        "http://{}/token",
        listener.local_addr().expect("token address")
    );
    let (accepted, received) = tokio::sync::oneshot::channel();
    let (release, response_ready) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("token request");
        let mut reader = tokio::io::BufReader::new(&mut stream);
        let mut content_length = 0;
        loop {
            let mut line = String::new();
            assert!(
                reader.read_line(&mut line).await.expect("request header") > 0,
                "incomplete request headers"
            );
            if line == "\r\n" {
                break;
            }
            if let Some(length) = line.to_ascii_lowercase().strip_prefix("content-length:") {
                content_length = length.trim().parse().expect("body length");
            }
        }
        reader
            .read_exact(&mut vec![0; content_length])
            .await
            .expect("complete grant");
        accepted.send(()).expect("provider accepted rotation");
        response_ready
            .await
            .expect("caller cancelled before response");
        let body =
            json!({"access_token": "new-access", "refresh_token": "new-refresh"}).to_string();
        stream
            .write_all(
                format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            )
            .await
            .expect("rotated credentials");
    });
    let keyring = Arc::new(MockKeyringStore::default());
    let mut oauth = loopback_config();
    oauth.token_url = endpoint;
    let (manager, _home) = client(oauth, keyring.clone());
    save_token(
        &manager,
        "old-access",
        Some("old-refresh"),
        Utc::now().timestamp() - 1,
    );
    let caller = manager.clone();
    let task = tokio::spawn(async move { caller.resolve_access_token().await });
    received.await.expect("accepted refresh");
    task.abort();
    assert!(task.await.expect_err("cancelled caller").is_cancelled());
    release.send(()).expect("release response");
    server.await.expect("token response task");
    // Taking the same cache lock waits for the detached transaction to finish persisting.
    let cached = manager.state.cached_token.lock().await;
    let expected = json!({"access_token": "new-access", "refresh_token": "new-refresh"});
    assert_eq!(
        serde_json::to_value(cached.token.as_ref()).expect("cached token"),
        expected
    );
    assert_eq!(
        serde_json::to_value(manager.load_token().expect("persisted token"))
            .expect("stored credential"),
        expected
    );
}

#[tokio::test]
async fn only_explicit_refresh_grant_rejections_request_reauthorization() {
    for (status, error_code) in [
        (400, Some("invalid_grant")),
        (400, Some("unauthorized_client")),
        (400, Some("unsupported_grant_type")),
        (400, Some("invalid_request")),
        (400, None),
        (401, Some("unauthorized_client")),
        (503, Some("unsupported_grant_type")),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/token"))
            .and(body_string_contains("grant_type=refresh_token"))
            .and(body_string_contains("refresh_token=old-refresh"))
            .respond_with(ResponseTemplate::new(status).set_body_json(json!({"error": error_code})))
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        let keyring = Arc::new(MockKeyringStore::default());
        let (manager, _home) = client(config(&server), keyring.clone());
        save_token(
            &manager,
            "old-access",
            Some("old-refresh"),
            Utc::now().timestamp() - 1,
        );
        let mut cached = Arc::clone(&manager.state.cached_token).lock_owned().await;
        let result = manager
            .refresh(&mut cached, &super::RefreshPolicy::WhenExpired)
            .await;

        match (status, error_code) {
            (400, Some("invalid_grant" | "unauthorized_client" | "unsupported_grant_type")) => {
                assert!(matches!(result, Ok(super::RefreshOutcome::Authorize)));
            }
            _ => {
                let message = result
                    .err()
                    .expect("surface token endpoint failure")
                    .to_string();
                assert!(message.contains(&format!("returned HTTP {status}")));
                if let Some(error_code) = error_code {
                    assert!(message.contains(error_code));
                }
            }
        }
    }
}

#[tokio::test]
async fn invalid_grant_recovers_external_credentials_with_a_bounded_retry() {
    for retry_status in [None, Some(200), Some(400)] {
        let server = MockServer::start().await;
        let keyring = Arc::new(MockKeyringStore::default());
        let (manager, _home) = client(config(&server), keyring.clone());
        save_token(
            &manager,
            "old-access",
            Some("old-refresh"),
            Utc::now().timestamp() - 1,
        );
        let writer = manager.clone();
        Mock::given(method("POST"))
            .and(body_string_contains("refresh_token=old-refresh"))
            .respond_with(move |_: &wiremock::Request| {
                writer
                    .save_token(&StoredToken {
                        access_token: "external-access".to_string(),
                        refresh_token: Some("external-refresh".to_string()),
                        expires_at: retry_status.map(|_| Utc::now().timestamp() - 1),
                    })
                    .expect("external client persists rotation");
                ResponseTemplate::new(/*s*/ 400).set_body_json(json!({"error": "invalid_grant"}))
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        if let Some(status) = retry_status {
            let body = if status == 200 {
                json!({"access_token": "retried-access", "refresh_token": "retried-refresh"})
            } else {
                json!({"error": "invalid_grant"})
            };
            Mock::given(method("POST"))
                .and(body_string_contains("refresh_token=external-refresh"))
                .respond_with(ResponseTemplate::new(status).set_body_json(body))
                .expect(/*r*/ 1)
                .mount(&server)
                .await;
        }
        if retry_status == Some(400) {
            let mut cached = Arc::clone(&manager.state.cached_token).lock_owned().await;
            cached.token = manager.load_token().expect("initial credential");
            assert!(matches!(
                manager
                    .refresh(&mut cached, &super::RefreshPolicy::WhenExpired)
                    .await
                    .expect("bounded recovery"),
                super::RefreshOutcome::Authorize
            ));
        } else {
            let expected = if retry_status.is_some() {
                "retried-access"
            } else {
                "external-access"
            };
            assert_eq!(
                manager
                    .resolve_access_token()
                    .await
                    .expect("reload instead of browser"),
                expected
            );
        }
    }
}

#[tokio::test]
async fn rejects_reserved_authorization_parameters_before_starting_login() {
    for name in [
        "response_type",
        "client_id",
        "redirect_uri",
        "state",
        "scope",
        "resource",
        "code_challenge",
        "code_challenge_method",
    ] {
        let mut oauth = loopback_config();
        oauth
            .authorization_url
            .push_str(&format!("?{name}=configured-value"));
        let (manager, _home) = client(oauth, Arc::new(MockKeyringStore::default()));
        assert_eq!(
            manager
                .resolve_access_token()
                .await
                .expect_err("reserved parameter")
                .to_string(),
            "provider OAuth authorization endpoint cannot include OAuth request parameters"
        );
    }
}

#[test]
fn debug_output_does_not_expose_configured_url_credentials() {
    let mut oauth = loopback_config();
    oauth
        .authorization_url
        .push_str("?issuer_credential=authorization-secret");
    oauth.token_url.push_str("?token=endpoint-secret");
    oauth.resource = Some("https://gateway.test/?custom_key=resource-secret".to_string());
    let (manager, _home) = client(oauth.clone(), Arc::new(MockKeyringStore::default()));
    for diagnostic in [format!("{oauth:?}"), format!("{manager:?}")] {
        for secret in ["authorization-secret", "endpoint-secret", "resource-secret"] {
            assert!(
                !diagnostic.contains(secret),
                "credential leaked in Debug output"
            );
        }
    }
}

#[tokio::test]
async fn malformed_token_response_keeps_credentials_and_omits_decoder_secrets() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
            "access_token": "response-secret", "expires_in": "response-secret",
        })))
        .expect(/*r*/ 1)
        .mount(&server)
        .await;
    let keyring = Arc::new(MockKeyringStore::default());
    let (manager, _home) = client(config(&server), keyring.clone());
    save_token(
        &manager,
        "old-access",
        Some("old-refresh"),
        Utc::now().timestamp() - 1,
    );
    let before = serde_json::to_value(manager.load_token().expect("initial token"))
        .expect("stored credential");
    let error = manager
        .resolve_access_token()
        .await
        .expect_err("invalid token response");
    assert_eq!(
        error.to_string(),
        "provider OAuth token response is invalid"
    );
    assert!(!format!("{error:?}").contains("response-secret"));
    assert_eq!(
        serde_json::to_value(manager.load_token().expect("unchanged token"))
            .expect("stored credential"),
        before
    );
}

#[tokio::test]
async fn failed_save_retains_rotation_without_overwriting_a_new_external_login() {
    #[derive(Clone, Copy, PartialEq, Eq)]
    enum Recovery {
        RetrySave,
        Expired,
        ExternalLogin,
    }
    for recovery in [
        Recovery::RetrySave,
        Recovery::Expired,
        Recovery::ExternalLogin,
    ] {
        let server = MockServer::start().await;
        let keyring = Arc::new(MockKeyringStore::default());
        let (manager, home) = client(config(&server), keyring.clone());
        save_token(
            &manager,
            "old-access",
            Some("old-refresh"),
            Utc::now().timestamp() - 1,
        );
        let before = serde_json::to_value(manager.load_token().expect("initial token"))
            .expect("stored credential");
        let path = home.path().join("secrets/gateway_oauth.age");
        let backup = path.with_extension("backup");
        let response_path = path.clone();
        let response_backup = backup.clone();
        Mock::given(method("POST"))
            .and(body_string_contains("refresh_token=old-refresh"))
            .respond_with(move |_: &wiremock::Request| {
                // Make persistence fail after the provider has rotated the token.
                std::fs::rename(&response_path, &response_backup).expect("retain old store");
                std::fs::create_dir(&response_path).expect("block credential persistence");
                ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "access_token": "rotated-access", "refresh_token": "rotated-refresh",
                }))
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        if recovery == Recovery::Expired {
            Mock::given(method("POST"))
                .and(body_string_contains("refresh_token=rotated-refresh"))
                .respond_with(ResponseTemplate::new(/*s*/ 200).set_body_json(json!({
                    "access_token": "renewed-access", "refresh_token": "renewed-refresh",
                })))
                .expect(/*r*/ 1)
                .mount(&server)
                .await;
        }
        assert!(manager.resolve_access_token().await.is_err());
        std::fs::remove_dir(&path).expect("unblock credential persistence");
        std::fs::rename(&backup, &path).expect("restore old store");
        assert_eq!(
            serde_json::to_value(manager.load_token().expect("unchanged token"))
                .expect("stored credential"),
            before
        );
        if recovery == Recovery::ExternalLogin {
            manager
                .save_token(&StoredToken {
                    access_token: "external-access".to_string(),
                    refresh_token: Some("external-refresh".to_string()),
                    expires_at: None,
                })
                .expect("external login");
        }
        // A failed browser login must discard pending rotation only if storage changed.
        Mock::given(method("POST"))
            .and(body_string_contains("grant_type=authorization_code"))
            .respond_with(
                ResponseTemplate::new(/*s*/ 400).set_body_json(json!({"error": "invalid_grant"})),
            )
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        {
            let mut cached = Arc::clone(&manager.state.cached_token).lock_owned().await;
            manager
                .authorize_with_browser(&mut cached, complete_browser_authorization)
                .await
                .expect_err("failed login must preserve the correct recovery baseline");
        }
        if recovery == Recovery::Expired {
            manager
                .state
                .cached_token
                .lock()
                .await
                .pending
                .as_mut()
                .expect("pending rotation")
                .expires_at = Some(Utc::now().timestamp() - 1);
        }
        let expected = match recovery {
            Recovery::ExternalLogin => {
                json!({"access_token": "external-access", "refresh_token": "external-refresh"})
            }
            Recovery::Expired => {
                json!({"access_token": "renewed-access", "refresh_token": "renewed-refresh"})
            }
            Recovery::RetrySave => {
                json!({"access_token": "rotated-access", "refresh_token": "rotated-refresh"})
            }
        };
        let recovered = if recovery == Recovery::RetrySave {
            manager.refresh_access_token("old-access").await
        } else {
            manager.resolve_access_token().await
        };
        assert_eq!(
            recovered.expect("recover pending rotation"),
            expected["access_token"].as_str().expect("expected token")
        );
        assert_eq!(
            serde_json::to_value(manager.load_token().expect("saved rotation"))
                .expect("stored credential"),
            expected
        );
    }
}
