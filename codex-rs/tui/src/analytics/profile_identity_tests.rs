//! In-flight profile responses cannot expose data after the active account changes.
use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn profile_rejects_account_change_during_success_or_error_response() {
    let server = MockServer::start().await;
    for status in [200, 404, 503] {
        server.reset().await;
        let (home, live) = live(&server, "plus").await;
        let home_path = home.path().to_path_buf();
        Mock::given(method("GET"))
            .and(path("/backend-api/wham/profiles/me"))
            .and(header("chatgpt-account-id", "account-a"))
            .respond_with(move |_: &wiremock::Request| {
                sign_in(&home_path, "account-a", "user-b", "plus");
                ResponseTemplate::new(status)
                    .set_body_json(json!({"stats":{"lifetime_tokens":123}}))
            })
            .expect(/*r*/ 1)
            .mount(&server)
            .await;
        assert_eq!(
            live.profile().await.err().unwrap(),
            "Account changed. Press R to refresh Analytics."
        );
    }
}
