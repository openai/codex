use super::*;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use std::time::Duration;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;
use wiremock::matchers::query_param_is_missing;

fn service_config(server: &MockServer) -> RemotePluginServiceConfig {
    RemotePluginServiceConfig {
        chatgpt_base_url: server.uri(),
    }
}

fn installed_plugin(index: usize, scope: RemotePluginScope) -> Value {
    let scope = match scope {
        RemotePluginScope::Global => "GLOBAL",
        RemotePluginScope::User => "USER",
        RemotePluginScope::Workspace => "WORKSPACE",
    };
    let mut plugin = json!({
        "id": format!("plugin-{index:03}"),
        "name": format!("plugin-{index:03}"),
        "scope": scope,
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "status": "ENABLED",
        "release": {
            "version": "1.2.3",
            "display_name": format!("Plugin {index:03}"),
            "description": "Remote plugin",
            "bundle_download_url": format!("https://example.com/plugin-{index:03}.tar.gz"),
            "app_manifest": {"apps": {"app": {"id": format!("app-{index:03}")}}},
            "interface": {},
            "skills": []
        },
        "enabled": index.is_multiple_of(2),
        "disabled_skill_names": []
    });
    if scope == "WORKSPACE" {
        plugin["discoverability"] = json!("LISTED");
    }
    plugin
}

fn installed_page(plugins: Vec<Value>, next_page_token: Option<&str>) -> Value {
    json!({
        "plugins": plugins,
        "pagination": {
            "limit": REMOTE_INSTALLED_PLUGIN_PAGE_LIMIT,
            "next_page_token": next_page_token,
        }
    })
}

fn unscoped_installed_request() -> wiremock::MockBuilder {
    Mock::given(method("GET"))
        .and(path("/ps/plugins/installed"))
        .and(query_param_is_missing("scope"))
        .and(query_param("includeDownloadUrls", "true"))
        .and(query_param("limit", "1000"))
        .and(header("authorization", "Bearer Access Token"))
        .and(header("chatgpt-account-id", "account_id"))
        .and(header("OAI-Product-Sku", "codex"))
}

#[tokio::test]
async fn unscoped_snapshot_follows_pagination_and_preserves_raw_metadata() {
    let server = MockServer::start().await;
    let first_page_plugins = (0..50)
        .map(|index| installed_plugin(index, RemotePluginScope::Global))
        .collect();
    unscoped_installed_request()
        .and(query_param_is_missing("pageToken"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(installed_page(first_page_plugins, Some("page-2"))),
        )
        .expect(1)
        .mount(&server)
        .await;
    unscoped_installed_request()
        .and(query_param("pageToken", "page-2"))
        .respond_with(ResponseTemplate::new(200).set_body_json(installed_page(
            vec![
                installed_plugin(50, RemotePluginScope::User),
                installed_plugin(51, RemotePluginScope::Workspace),
            ],
            None,
        )))
        .expect(1)
        .mount(&server)
        .await;

    let cache = RemoteInstalledPluginSnapshotCache::default();
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let snapshot = cache
        .get_or_fetch(&service_config(&server), &auth)
        .await
        .expect("unscoped snapshot should load");

    assert_eq!(snapshot.len(), 52);
    assert_eq!(snapshot[50].plugin.scope, RemotePluginScope::User);
    assert_eq!(snapshot[51].plugin.scope, RemotePluginScope::Workspace);
    assert_eq!(
        snapshot[51].plugin.release.bundle_download_url.as_deref(),
        Some("https://example.com/plugin-051.tar.gz")
    );
    assert_eq!(
        snapshot[51].plugin.release.app_manifest,
        Some(json!({"apps": {"app": {"id": "app-051"}}}))
    );
    assert_eq!(
        snapshot[51].plugin.installation_policy,
        codex_app_server_protocol::PluginInstallPolicy::Available
    );
    assert_eq!(
        snapshot[51].plugin.authentication_policy,
        codex_app_server_protocol::PluginAuthPolicy::OnUse
    );
}

#[tokio::test]
async fn concurrent_scope_partitions_share_one_upstream_request() {
    let server = MockServer::start().await;
    unscoped_installed_request()
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(installed_page(
                    vec![
                        installed_plugin(0, RemotePluginScope::Global),
                        installed_plugin(1, RemotePluginScope::User),
                        installed_plugin(2, RemotePluginScope::Workspace),
                    ],
                    None,
                )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let cache = RemoteInstalledPluginSnapshotCache::default();
    let config = service_config(&server);
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let (global, user, workspace) = tokio::join!(
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::Global),
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::User),
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::Workspace),
    );

    for (result, expected_name) in [
        (global, "plugin-000"),
        (user, "plugin-001"),
        (workspace, "plugin-002"),
    ] {
        let plugins = result.expect("scope partition should load");
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].plugin.name, expected_name);
    }
}

#[tokio::test]
async fn concurrent_failures_share_one_upstream_request() {
    let server = MockServer::start().await;
    unscoped_installed_request()
        .respond_with(ResponseTemplate::new(503).set_delay(Duration::from_millis(100)))
        .expect(1)
        .mount(&server)
        .await;

    let cache = RemoteInstalledPluginSnapshotCache::default();
    let config = service_config(&server);
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let (global, user, workspace) = tokio::join!(
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::Global),
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::User),
        installed_plugins_for_scope(&cache, &config, &auth, RemotePluginScope::Workspace),
    );

    for result in [global, user, workspace] {
        assert!(result.is_err());
    }
}

#[tokio::test]
async fn invalidation_aborts_an_in_flight_snapshot() {
    let server = MockServer::start().await;
    unscoped_installed_request()
        .respond_with(
            ResponseTemplate::new(200)
                .set_delay(Duration::from_millis(100))
                .set_body_json(installed_page(
                    vec![installed_plugin(0, RemotePluginScope::Global)],
                    None,
                )),
        )
        .expect(1)
        .mount(&server)
        .await;

    let cache = RemoteInstalledPluginSnapshotCache::default();
    let config = service_config(&server);
    let auth = CodexAuth::create_dummy_chatgpt_auth_for_testing();
    let (result, ()) = tokio::join!(cache.get_or_fetch(&config, &auth), async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        cache.invalidate();
    });

    assert!(matches!(
        result,
        Err(RemotePluginCatalogError::UnexpectedResponse(message))
            if message == REMOTE_INSTALLED_PLUGIN_SNAPSHOT_INVALIDATED
    ));
}
