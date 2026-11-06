//! Exercises the device protocol at its HTTP boundary and the shared credential lifecycle.

use super::*;
use core_test_support::oauth::mount_device_code_grant;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn device_login_presents_prompt_and_persists_grant() {
    let server = MockServer::start().await;
    let _token_request = mount_device_code_grant(&server, approved_device_tokens()).await;
    let (client, _home) = client(device_config(&server), Default::default());
    client
        .save_token(&StoredToken {
            authorization_id: Some("previous-grant".into()),
            access_token: "previous-access".into(),
            refresh_token: None,
            expires_at: None,
        })
        .unwrap();
    let mut shown = None;
    client
        .login_with_device_code(std::future::pending(), |prompt| {
            shown = Some((
                prompt.verification_uri.to_string(),
                prompt.user_code.clone(),
            ));
        })
        .await
        .unwrap();
    assert_eq!(
        shown,
        Some(("https://login.example/device".into(), "ABCD-EFGH".into()))
    );
    let requests = server.received_requests().await.unwrap();
    let challenge = url::form_urlencoded::parse(&requests[0].body).collect::<HashMap<_, _>>();
    assert_eq!(challenge["client_id"], "codex-test");
    assert_eq!(challenge["scope"], "openid gateway.inference");
    let stored = client.load_token().unwrap().unwrap();
    assert_eq!(
        (
            stored.access_token.as_str(),
            stored.refresh_token.as_deref()
        ),
        ("device-access", Some("device-refresh"))
    );
    let sign_in_id = stored.authorization_id.expect("saved sign-in ID");
    assert_ne!(sign_in_id, "previous-grant");
}

async fn mount_device_challenge(server: &MockServer, verification_uri: &str) {
    Mock::given(method("POST"))
        .and(path("/devicecode"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "device_code": "secret-device-code", "user_code": "ABCD-EFGH",
            "verification_uri": verification_uri, "expires_in": 60,
        })))
        .expect(1)
        .mount(server)
        .await;
}

#[tokio::test]
async fn device_login_rejects_an_insecure_verification_link_before_showing_the_code() {
    let server = MockServer::start().await;
    mount_device_challenge(&server, "http://sso.example/activate").await;
    let (client, _home) = client(device_config(&server), Default::default());
    let error = client
        .login_with_device_code(std::future::pending(), |_| {
            panic!("unsafe verification link was shown")
        })
        .await
        .unwrap_err();
    assert_eq!(
        error.to_string(),
        "The gateway returned an invalid device verification URL"
    );
}

#[tokio::test]
async fn device_login_reports_network_policy_denial_for_either_endpoint() {
    let server = MockServer::start().await;
    mount_device_challenge(&server, "https://sso.example/activate").await;
    let config = device_config(&server);
    for allowed in [
        config.token_url.as_str(),
        config.device_authorization_url.as_deref().unwrap(),
    ] {
        let policy = codex_http_client::NetworkPolicy::unmanaged()
            .restrict_to_endpoints([url::Url::parse(allowed).unwrap()].into());
        let factory = transport_default_auth_route_config()
            .http_client_factory()
            .clone()
            .with_network_policy(policy);
        let home = tempfile::tempdir().unwrap();
        let client = OAuthSessionManager::new(
            config.clone(),
            home.path().to_path_buf(),
            &factory,
            Arc::new(MockKeyringStore::default()),
        )
        .unwrap();
        let error = client
            .login_with_device_code(std::future::pending(), |_| {
                panic!("code shown for a blocked login")
            })
            .await
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            codex_http_client::NetworkPolicyDenied::Destination.to_string()
        );
    }
}

#[tokio::test]
async fn denied_or_expired_codes_explain_why_sign_in_stopped() {
    for (code, expected) in [("access_denied", "denied"), ("expired_token", "expired")] {
        let server = MockServer::start().await;
        let response = ResponseTemplate::new(400)
            .set_body_json(json!({"error": code, "error_description": "secret-device-code"}));
        let _token_request = mount_device_code_grant(&server, response).await;
        let (client, _home) = client(device_config(&server), Default::default());

        let error = client
            .login_with_device_code(std::future::pending(), |_| {})
            .await
            .unwrap_err();
        assert!(error.to_string().contains(expected));
        assert!(!format!("{error:?}").contains("secret-device-code"));
    }
}

#[tokio::test]
async fn device_login_can_be_canceled_while_waiting_for_approval() {
    let server = MockServer::start().await;
    let pending = ResponseTemplate::new(400)
        .set_body_json(json!({"error": "authorization_pending"}))
        .set_delay(Duration::from_secs(30));
    let token_request = mount_device_code_grant(&server, pending).await;
    let (client, home) = client(device_config(&server), Default::default());

    let cancel = async {
        token_request.wait_until_satisfied().await;
        // Another Codex process can use the credential store while the user decides.
        let _store = super::super::storage::lock_credentials(home.path())
            .await
            .unwrap();
    };
    let result = tokio::time::timeout(
        Duration::from_secs(15),
        client.login_with_device_code(cancel, |_| {}),
    )
    .await
    .expect("cancel should finish while the issuer is still waiting");
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::Interrupted);
}
