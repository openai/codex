use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::body_string_contains;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

#[test]
fn organizations_by_preference_orders_default_then_personal_then_input_order() {
    let organizations = vec![
        Organization {
            id: "org-first".to_string(),
            title: Some("First".to_string()),
            is_default: false,
            personal: false,
        },
        Organization {
            id: "org-personal".to_string(),
            title: Some("Personal".to_string()),
            is_default: false,
            personal: true,
        },
        Organization {
            id: "org-default".to_string(),
            title: Some("Default".to_string()),
            is_default: true,
            personal: false,
        },
    ];

    let selected = organizations_by_preference(&organizations);

    assert_eq!(
        selected,
        vec![&organizations[2], &organizations[1], &organizations[0]]
    );
}

#[test]
fn find_default_project_returns_initial_project() {
    let projects = vec![
        Project {
            id: "proj-secondary".to_string(),
            title: Some("Secondary".to_string()),
            is_initial: false,
        },
        Project {
            id: "proj-default".to_string(),
            title: Some("Default".to_string()),
            is_initial: true,
        },
    ];

    let selected = find_default_project(&projects);

    assert_eq!(selected, projects.get(1));
}

#[tokio::test]
async fn create_api_key_from_authorization_code_creates_api_key() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/oauth/token"))
        .and(header("content-type", "application/x-www-form-urlencoded"))
        .and(body_string_contains("client_id=client-123"))
        .and(body_string_contains("code_verifier=verifier-123"))
        .and(body_string_contains("code=auth-code-123"))
        .and(body_string_contains("grant_type=authorization_code"))
        .and(body_string_contains(
            "redirect_uri=http%3A%2F%2Flocalhost%3A5000%2Fauth%2Fcallback",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "id_token": "id-token-123",
            "access_token": "oauth-access-123",
            "refresh_token": "oauth-refresh-123",
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path("/dashboard/onboarding/login"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "user": {
                "session": {
                    "sensitive_id": "session-123",
                }
            }
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/v1/organizations"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "org-default",
                    "title": "Default Org",
                    "is_default": true,
                },
                {
                    "id": "org-secondary",
                    "title": "Secondary Org",
                }
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dashboard/organizations/org-default/projects"))
        .and(query_param("detail", "basic"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": []
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/dashboard/organizations/org-secondary/projects"))
        .and(query_param("detail", "basic"))
        .and(query_param("limit", "100"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "data": [
                {
                    "id": "proj-default",
                    "title": "Default Project",
                    "is_initial": true,
                }
            ]
        })))
        .mount(&server)
        .await;
    Mock::given(method("POST"))
        .and(path(
            "/dashboard/organizations/org-secondary/projects/proj-default/api_keys",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "key": {
                "sensitive_id": "sk-proj-123",
            }
        })))
        .mount(&server)
        .await;

    let options = CreateApiKeyOptions {
        issuer: server.uri(),
        client_id: "client-123".to_string(),
        audience: PLATFORM_AUDIENCE.to_string(),
        api_base: server.uri(),
        app: APP.to_string(),
        callback_port: CALLBACK_PORT,
        scope: SCOPE.to_string(),
        api_key_name: PROJECT_API_KEY_NAME.to_string(),
        project_poll_interval_seconds: 1,
        project_poll_timeout_seconds: 5,
    };
    let client = build_http_client().expect("client");

    let output = create_api_key_from_authorization_code(
        &client,
        &options,
        "http://localhost:5000/auth/callback",
        "verifier-123",
        "auth-code-123",
    )
    .await
    .expect("provision");

    assert_eq!(
        output,
        CreatedApiKey {
            organization_id: "org-secondary".to_string(),
            organization_title: Some("Secondary Org".to_string()),
            default_project_id: "proj-default".to_string(),
            default_project_title: Some("Default Project".to_string()),
            project_api_key: "sk-proj-123".to_string(),
        }
    );
}
