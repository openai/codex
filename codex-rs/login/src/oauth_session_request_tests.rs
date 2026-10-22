//! Request credentials retain their grant identity through unauthorized recovery.

use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn request_refresh_retry_keeps_the_same_sign_in() {
    let server = MockServer::start().await;
    let (manager, _home) = client(config(&server), Arc::new(MockKeyringStore::default()));
    manager
        .save_token(&StoredToken {
            access_token: "rejected".into(),
            authorization_id: Some("grant".into()),
            refresh_token: Some("refresh".into()),
            expires_at: Some(Utc::now().timestamp() + 3_600),
        })
        .unwrap();
    let mut request = manager.begin_request();
    assert_eq!(
        request.credentials().await.unwrap().access_token,
        "rejected"
    );
    assert!(request.reject_credentials());
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("refresh_token=refresh"))
        .respond_with(ResponseTemplate::new(503))
        .expect(1)
        .mount(&server)
        .await;
    assert!(request.credentials().await.is_err());
    server.verify().await;
    server.reset().await;
    Mock::given(method("POST"))
        .and(path("/token"))
        .and(body_string_contains("refresh_token=refresh"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "access_token": "replacement",
            "token_type": "Bearer",
            "expires_in": 3600
        })))
        .expect(1)
        .mount(&server)
        .await;
    let credentials = request.credentials().await.unwrap();
    assert_eq!(
        (credentials.access_token, credentials.authorization_id),
        ("replacement".into(), Some("grant".into()))
    );
    assert_eq!(manager.authorization_id().await.unwrap(), "grant");
    // Ordinary transport retries reuse the refreshed token without another exchange.
    assert_eq!(
        request.credentials().await.unwrap().access_token,
        "replacement"
    );
}
