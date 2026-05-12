use super::*;
use codex_login::CodexAuth;
use pretty_assertions::assert_eq;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;

fn test_config(server: &MockServer) -> RemotePluginServiceConfig {
    RemotePluginServiceConfig {
        chatgpt_base_url: format!("{}/backend-api", server.uri()),
    }
}

fn test_auth() -> CodexAuth {
    CodexAuth::create_dummy_chatgpt_auth_for_testing()
}

fn app(id: &str) -> AppConnectorId {
    AppConnectorId(id.to_string())
}

#[tokio::test]
async fn resolve_remote_plugin_app_ids_expands_templates_and_dedupes_stably() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/connectors/by_template_id/templated_apps_GitHubEnterprise",
        ))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .respond_with(ResponseTemplate::new(200).set_body_string(
            r#"{"connector_ids":["connector_ghe","asdk_app_ghe","connector_ghe"]}"#,
        ))
        .mount(&server)
        .await;

    let resolved = resolve_remote_plugin_app_ids(
        &test_config(&server),
        Some(&test_auth()),
        &[
            app("asdk_app_linear"),
            app("templated_apps_GitHubEnterprise"),
            app("asdk_app_linear"),
            app("asdk_app_ghe"),
        ],
    )
    .await;

    assert_eq!(
        resolved,
        vec![
            app("asdk_app_linear"),
            app("connector_ghe"),
            app("asdk_app_ghe"),
        ]
    );
}

#[tokio::test]
async fn resolve_remote_plugin_app_ids_drops_missing_template_mappings() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/connectors/by_template_id/templated_apps_GitHubEnterprise",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_string(r#"{"connector_ids":[]}"#))
        .mount(&server)
        .await;

    let resolved = resolve_remote_plugin_app_ids(
        &test_config(&server),
        Some(&test_auth()),
        &[app("templated_apps_GitHubEnterprise")],
    )
    .await;

    assert_eq!(resolved, Vec::<AppConnectorId>::new());
}

#[tokio::test]
async fn resolve_remote_plugin_app_ids_drops_templates_when_lookup_fails() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(
            "/backend-api/ps/connectors/by_template_id/templated_apps_GitHubEnterprise",
        ))
        .respond_with(ResponseTemplate::new(500).set_body_string("lookup failed"))
        .mount(&server)
        .await;

    let resolved = resolve_remote_plugin_app_ids(
        &test_config(&server),
        Some(&test_auth()),
        &[
            app("asdk_app_linear"),
            app("templated_apps_GitHubEnterprise"),
        ],
    )
    .await;

    assert_eq!(resolved, vec![app("asdk_app_linear")]);
}
