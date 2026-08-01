use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::PluginSearchParams;
use codex_app_server_protocol::PluginSearchResponse;
use codex_app_server_protocol::PluginSearchScope;
use codex_config::types::AuthCredentialsStoreMode;
use pretty_assertions::assert_eq;
use serde_json::json;
use tempfile::TempDir;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;
use wiremock::matchers::query_param_is_missing;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(10);

#[tokio::test]
async fn plugin_search_omits_shared_workspace_results_when_plugin_sharing_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
remote_plugin = true
plugin_sharing = false
"#,
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "linear"))
        .and(query_param("limit", "16"))
        .and(query_param("pageToken", "incoming-token"))
        .and(query_param_is_missing("scope"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                remote_plugin_json(
                    "plugin-global",
                    "global-linear",
                    "GLOBAL",
                    /*discoverability*/ None,
                ),
                remote_plugin_json(
                    "plugin-user",
                    "personal-linear",
                    "USER",
                    /*discoverability*/ None,
                ),
                remote_plugin_json(
                    "plugin-listed",
                    "listed-linear",
                    "WORKSPACE",
                    /*discoverability*/ Some("LISTED"),
                ),
                remote_plugin_json(
                    "plugin-private",
                    "private-linear",
                    "WORKSPACE",
                    /*discoverability*/ Some("PRIVATE"),
                ),
                remote_plugin_json(
                    "plugin-unlisted",
                    "unlisted-linear",
                    "WORKSPACE",
                    /*discoverability*/ Some("UNLISTED"),
                ),
            ],
            "pagination": {"next_page_token": "outgoing-token"},
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    let request_id = mcp
        .send_plugin_search_request(PluginSearchParams {
            search_term: "linear".to_string(),
            scope: None,
            cwds: None,
            cursor: Some("incoming-token".to_string()),
            limit: None,
        })
        .await?;
    let response: PluginSearchResponse =
        timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

    assert_eq!(response.next_cursor.as_deref(), Some("outgoing-token"));
    assert_eq!(
        response
            .data
            .iter()
            .map(|result| { (result.marketplace_name.as_str(), result.plugin.id.as_str(),) })
            .collect::<Vec<_>>(),
        vec![
            (
                "openai-curated-remote",
                "global-linear@openai-curated-remote"
            ),
            (
                "created-by-me-remote",
                "personal-linear@created-by-me-remote"
            ),
            ("workspace-directory", "listed-linear@workspace-directory"),
        ]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_search_only_searches_workspace_when_remote_plugin_disabled() -> Result<()> {
    let codex_home = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
remote_plugin = false
"#,
            server.uri()
        ),
    )?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "linear"))
        .and(query_param("scope", "WORKSPACE"))
        .and(query_param("limit", "16"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json(
                "plugin-workspace",
                "workspace-linear",
                "WORKSPACE",
                /*discoverability*/ Some("LISTED"),
            )],
            "pagination": {"next_page_token": null},
        })))
        .expect(2)
        .mount(&server)
        .await;

    let mut mcp = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;

    for scope in [None, Some(PluginSearchScope::Workspace)] {
        let request_id = mcp
            .send_plugin_search_request(PluginSearchParams {
                search_term: "linear".to_string(),
                scope,
                cwds: None,
                cursor: None,
                limit: None,
            })
            .await?;
        let response: PluginSearchResponse =
            timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

        assert_eq!(
            response
                .data
                .iter()
                .map(|result| (result.marketplace_name.as_str(), result.plugin.id.as_str(),))
                .collect::<Vec<_>>(),
            vec![(
                "workspace-directory",
                "workspace-linear@workspace-directory",
            )]
        );
    }

    for scope in [PluginSearchScope::Global, PluginSearchScope::Personal] {
        let request_id = mcp
            .send_plugin_search_request(PluginSearchParams {
                search_term: "linear".to_string(),
                scope: Some(scope),
                cwds: None,
                cursor: None,
                limit: None,
            })
            .await?;
        let response: PluginSearchResponse =
            timeout(DEFAULT_TIMEOUT, mcp.read_response(request_id)).await??;

        assert_eq!(
            response,
            PluginSearchResponse {
                data: Vec::new(),
                next_cursor: None,
            }
        );
    }

    let search_requests = server
        .received_requests()
        .await
        .expect("wiremock should record requests")
        .into_iter()
        .filter(|request| request.url.path() == "/backend-api/ps/plugins/search")
        .collect::<Vec<_>>();
    assert_eq!(search_requests.len(), 2);
    assert_eq!(
        search_requests
            .iter()
            .filter_map(|request| {
                request
                    .url
                    .query_pairs()
                    .find(|(name, _value)| name == "scope")
                    .map(|(_name, value)| value.into_owned())
            })
            .collect::<Vec<_>>(),
        vec!["WORKSPACE", "WORKSPACE"]
    );
    Ok(())
}

fn remote_plugin_json(
    remote_plugin_id: &str,
    plugin_name: &str,
    scope: &str,
    discoverability: Option<&str>,
) -> serde_json::Value {
    json!({
        "id": remote_plugin_id,
        "name": plugin_name,
        "scope": scope,
        "discoverability": discoverability,
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
            "display_name": plugin_name,
            "description": format!("{plugin_name} description"),
            "interface": {},
        },
    })
}
