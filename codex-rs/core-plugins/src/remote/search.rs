use super::RemotePluginCatalogError;
use super::RemotePluginListResponse;
use super::RemotePluginScope;
use super::RemotePluginServiceConfig;
use super::authenticated_request;
use super::ensure_chatgpt_auth;
use super::send_and_decode;
use codex_login::CodexAuth;
use codex_login::default_client::build_reqwest_client;
use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemotePluginSearchPlugin {
    pub remote_plugin_id: String,
    pub name: String,
    pub display_name: String,
    pub short_description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RemotePluginSearchResult {
    pub plugins: Vec<RemotePluginSearchPlugin>,
}

pub async fn search_global_remote_plugins(
    config: &RemotePluginServiceConfig,
    auth: Option<&CodexAuth>,
    q: &str,
) -> Result<RemotePluginSearchResult, RemotePluginCatalogError> {
    let auth = ensure_chatgpt_auth(auth)?;
    let base_url = config.chatgpt_base_url.trim_end_matches('/');
    let url = format!("{base_url}/ps/plugins/discover");
    let client = build_reqwest_client();
    let request = authenticated_request(client.get(&url), auth)?
        .query(&[("q", q), ("scope", RemotePluginScope::Global.api_value())]);
    let response: RemotePluginListResponse = send_and_decode(request, &url).await?;
    Ok(RemotePluginSearchResult {
        plugins: response
            .plugins
            .into_iter()
            .map(|plugin| RemotePluginSearchPlugin {
                remote_plugin_id: plugin.id,
                name: plugin.name,
                display_name: plugin.release.display_name,
                short_description: plugin.release.interface.short_description,
            })
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;
    use reqwest::StatusCode;
    use serde_json::json;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;
    use wiremock::matchers::query_param_is_missing;

    fn test_config(server: &MockServer) -> RemotePluginServiceConfig {
        RemotePluginServiceConfig {
            chatgpt_base_url: format!("{}/backend-api", server.uri()),
        }
    }

    fn test_auth() -> CodexAuth {
        CodexAuth::create_dummy_chatgpt_auth_for_testing()
    }

    #[tokio::test]
    async fn search_global_remote_plugins_returns_compact_first_page() {
        let server = MockServer::start().await;
        let q = "plugin_name:slack AND tool_name:(search OR messages)";
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/discover"))
            .and(header("authorization", "Bearer Access Token"))
            .and(header("chatgpt-account-id", "account_id"))
            .and(query_param("q", q))
            .and(query_param("scope", "GLOBAL"))
            .and(query_param_is_missing("includeInstalled"))
            .and(query_param_is_missing("includeDownloadUrls"))
            .and(query_param_is_missing("limit"))
            .and(query_param_is_missing("pageToken"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!({
                "plugins": [{
                    "id": "plugin_123",
                    "name": "slack",
                    "scope": "GLOBAL",
                    "discoverability": "LISTED",
                    "status": "ENABLED",
                    "installation_policy": "AVAILABLE",
                    "authentication_policy": "ON_USE",
                    "release": {
                        "version": "1.0.0",
                        "display_name": "Slack",
                        "description": "Search and read Slack messages",
                        "interface": {
                            "short_description": "Search and read Slack messages"
                        },
                        "skills": []
                    }
                }],
                "pagination": {
                    "limit": 50,
                    "next_page_token": "unused-next-page"
                }
            })))
            .mount(&server)
            .await;

        assert_eq!(
            search_global_remote_plugins(&test_config(&server), Some(&test_auth()), q)
                .await
                .unwrap(),
            RemotePluginSearchResult {
                plugins: vec![RemotePluginSearchPlugin {
                    remote_plugin_id: "plugin_123".to_string(),
                    name: "slack".to_string(),
                    display_name: "Slack".to_string(),
                    short_description: Some("Search and read Slack messages".to_string()),
                }],
            }
        );
    }

    #[tokio::test]
    async fn search_global_remote_plugins_preserves_backend_error_details() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/backend-api/ps/plugins/discover"))
            .and(query_param("q", "tool_name:(search OR)"))
            .and(query_param("scope", "GLOBAL"))
            .respond_with(
                ResponseTemplate::new(422).set_body_string(r#"{"detail":"Expected query term"}"#),
            )
            .mount(&server)
            .await;

        let err = search_global_remote_plugins(
            &test_config(&server),
            Some(&test_auth()),
            "tool_name:(search OR)",
        )
        .await
        .unwrap_err();
        let RemotePluginCatalogError::UnexpectedStatus { url, status, body } = err else {
            panic!("expected unexpected status error");
        };
        assert_eq!(
            (url, status, body),
            (
                format!("{}/backend-api/ps/plugins/discover", server.uri()),
                StatusCode::UNPROCESSABLE_ENTITY,
                r#"{"detail":"Expected query term"}"#.to_string(),
            )
        );
    }
}
