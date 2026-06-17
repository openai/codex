use super::*;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::query_param;

#[tokio::test]
async fn requests_runner_token_with_exact_audience() -> anyhow::Result<()> {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("Authorization", "Bearer runner-request-secret"))
        .and(query_param("api-version", "2.0"))
        .and(query_param("audience", "openai-audience"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!({"value": "github.actions.jwt"})),
        )
        .expect(1)
        .mount(&server)
        .await;
    let source = GithubActionsSubjectTokenProvider {
        request_url: Some(format!("{}?api-version=2.0", server.uri())),
        request_token: Some("runner-request-secret".to_string()),
        audience: "openai-audience".to_string(),
        http: reqwest::Client::new(),
    };

    assert_eq!(
        source.subject_token().await?,
        SubjectToken::jwt("github.actions.jwt", "github_actions")?
    );
    assert!(!format!("{source:?}").contains("runner-request-secret"));
    Ok(())
}

#[test]
fn replaces_runner_supplied_audience() -> anyhow::Result<()> {
    let source = GithubActionsSubjectTokenProvider {
        request_url: Some(
            "https://vstoken.actions.githubusercontent.com/token?audience=wrong&api-version=2.0"
                .to_string(),
        ),
        request_token: Some("runner-request-secret".to_string()),
        audience: "openai-audience".to_string(),
        http: reqwest::Client::new(),
    };

    let audiences = source
        .request_url()?
        .query_pairs()
        .filter(|(name, _)| name == "audience")
        .map(|(_, value)| value.into_owned())
        .collect::<Vec<_>>();
    assert_eq!(audiences, vec!["openai-audience"]);
    Ok(())
}

#[test]
fn rejects_request_url_user_info_and_fragments() {
    for request_url in [
        "https://user@actions.githubusercontent.com/token",
        "https://actions.githubusercontent.com/token#fragment",
    ] {
        let source = GithubActionsSubjectTokenProvider {
            request_url: Some(request_url.to_string()),
            request_token: Some("runner-request-secret".to_string()),
            audience: "openai-audience".to_string(),
            http: reqwest::Client::new(),
        };

        assert!(matches!(
            source.request_url(),
            Err(SubjectTokenError::InvalidConfiguration {
                provider: "github_actions"
            })
        ));
    }
}
