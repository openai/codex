use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::PluginSearchParams;
use codex_app_server_protocol::PluginSearchResponse;
use codex_app_server_protocol::PluginSearchScope;
use codex_config::types::AuthCredentialsStoreMode;
use codex_login::AuthKeyringBackendKind;
use codex_login::login_with_api_key;
use codex_utils_absolute_path::AbsolutePathBuf;
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

#[tokio::test]
async fn plugin_search_stitches_local_results_into_the_first_remote_page() -> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let server = MockServer::start().await;
    std::fs::create_dir_all(codex_home.path().join(".tmp"))?;
    std::fs::write(
        codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json"),
        "{invalid json",
    )?;
    let overflow_plugin_names = (0..101)
        .map(|index| format!("integration-{index:03}"))
        .collect::<Vec<_>>();
    let mut local_plugins = vec![
        LocalPluginFixture {
            name: "calendar-notes",
            display_name: "Calendar Notes",
            keywords: &[],
            description: "Take notes",
        },
        LocalPluginFixture {
            name: "integrations",
            display_name: "Integrations",
            keywords: &["calendar"],
            description: "Connect services",
        },
        LocalPluginFixture {
            name: "task-sync",
            display_name: "Task Sync",
            keywords: &[],
            description: "Sync calendar events",
        },
    ];
    local_plugins.extend(overflow_plugin_names.iter().map(|name| LocalPluginFixture {
        name,
        display_name: name,
        keywords: &["calendar"],
        description: "Connect calendar services",
    }));
    local_plugins.push(LocalPluginFixture {
        name: "calendar-priority",
        display_name: "Calendar Priority",
        keywords: &[],
        description: "Prioritized calendar",
    });
    let marketplace_path = write_local_marketplace(
        repo_root.path(),
        "personal-tools",
        "marketplace.json",
        &local_plugins,
    )?;
    write_remote_plugin_search_config(codex_home.path(), &server, /*remote_plugin*/ true)?;
    write_chatgpt_search_auth(codex_home.path())?;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "calendar"))
        .and(query_param("limit", "1"))
        .and(query_param_is_missing("pageToken"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                remote_plugin_json("remote-exact", "calendar", "GLOBAL", /*discoverability*/ None),
                remote_plugin_json(
                    "remote-substring",
                    "connected-calendar",
                    "GLOBAL",
                    /*discoverability*/ None,
                ),
            ],
            "pagination": {"next_page_token": "next-page"},
        })))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "calendar"))
        .and(query_param("limit", "1"))
        .and(query_param("pageToken", "next-page"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [remote_plugin_json(
                "remote-later",
                "calendar-later",
                "GLOBAL",
                /*discoverability*/ None,
            )],
            "pagination": {"next_page_token": null},
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let roots = vec![AbsolutePathBuf::try_from(repo_root.path())?];
    let request_id = app_server
        .send_plugin_search_request(PluginSearchParams {
            search_term: "calendar".to_string(),
            scope: None,
            cwds: Some(roots.clone()),
            cursor: None,
            limit: Some(1),
        })
        .await?;
    let response: PluginSearchResponse =
        timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;

    assert_eq!(response.next_cursor.as_deref(), Some("next-page"));
    let mut expected_results = vec![
        ("calendar", None, false),
        ("calendar-notes", Some(&marketplace_path), false),
        ("calendar-priority", Some(&marketplace_path), false),
        ("connected-calendar", None, false),
        ("integrations", Some(&marketplace_path), false),
    ];
    expected_results.extend(
        overflow_plugin_names
            .iter()
            .take(/*n*/ 97)
            .map(|name| (name.as_str(), Some(&marketplace_path), false)),
    );
    assert_eq!(
        response
            .data
            .iter()
            .map(|result| (
                result.plugin.name.as_str(),
                result.marketplace_path.as_ref(),
                result.plugin.enabled,
            ))
            .collect::<Vec<_>>(),
        expected_results
    );

    let request_id = app_server
        .send_plugin_search_request(PluginSearchParams {
            search_term: "calendar".to_string(),
            scope: None,
            cwds: Some(roots),
            cursor: response.next_cursor,
            limit: Some(1),
        })
        .await?;
    let response: PluginSearchResponse =
        timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;

    assert_eq!(response.next_cursor, None);
    assert_eq!(
        response
            .data
            .iter()
            .map(|result| (result.plugin.name.as_str(), result.plugin.enabled))
            .collect::<Vec<_>>(),
        vec![("calendar-later", false)]
    );
    Ok(())
}

#[tokio::test]
async fn plugin_search_returns_local_matches_for_api_key_auth() -> Result<()> {
    for remote_plugin_enabled in [false, true] {
        let codex_home = TempDir::new()?;
        let repo_root = TempDir::new()?;
        let curated_root = codex_home.path().join(".tmp/plugins");
        let bundled_alpha_root = codex_home
            .path()
            .join(".tmp/bundled-marketplaces/openai-bundled-alpha");
        let server = MockServer::start().await;
        write_local_marketplace(
            &curated_root,
            "openai-api-curated",
            "api_marketplace.json",
            &[LocalPluginFixture {
                name: "calendar-built-in",
                display_name: "Calendar Built In",
                keywords: &[],
                description: "Built-in calendar",
            }],
        )?;
        write_local_marketplace(
            repo_root.path(),
            "personal-tools",
            "marketplace.json",
            &[
                LocalPluginFixture {
                    name: "developer-tools",
                    display_name: "Developer Tools",
                    keywords: &["api-key"],
                    description: "Manage credentials",
                },
                LocalPluginFixture {
                    name: "japanese-notes",
                    display_name: "日本語メモ",
                    keywords: &[],
                    description: "Japanese notes",
                },
            ],
        )?;
        write_local_marketplace(
            &bundled_alpha_root,
            "openai-bundled-alpha",
            "marketplace.json",
            &[LocalPluginFixture {
                name: "alpha-calendar",
                display_name: "Alpha Calendar",
                keywords: &[],
                description: "Built-in alpha calendar",
            }],
        )?;
        write_remote_plugin_search_config(codex_home.path(), &server, remote_plugin_enabled)?;
        login_with_api_key(
            codex_home.path(),
            "sk-test-key",
            AuthCredentialsStoreMode::File,
            AuthKeyringBackendKind::default(),
        )?;

        let mut app_server = TestAppServer::builder()
            .with_codex_home(codex_home.path())
            .without_auto_env()
            .build_initialized_with_timeout(DEFAULT_TIMEOUT)
            .await?;

        for (scope, search_term, expected_names) in [
            (
                Some(PluginSearchScope::Global),
                "calendar",
                vec!["calendar-built-in", "alpha-calendar"],
            ),
            (
                Some(PluginSearchScope::Personal),
                "API key",
                vec!["developer-tools"],
            ),
            (
                Some(PluginSearchScope::Personal),
                "日本語",
                vec!["japanese-notes"],
            ),
            (Some(PluginSearchScope::Personal), "calendar", Vec::new()),
            (Some(PluginSearchScope::Workspace), "calendar", Vec::new()),
            (
                None,
                "calendar",
                vec!["calendar-built-in", "alpha-calendar"],
            ),
            (None, "!!!", Vec::new()),
        ] {
            let request_id = app_server
                .send_plugin_search_request(PluginSearchParams {
                    search_term: search_term.to_string(),
                    scope,
                    cwds: Some(vec![
                        AbsolutePathBuf::try_from(repo_root.path())?,
                        AbsolutePathBuf::try_from(bundled_alpha_root.as_path())?,
                    ]),
                    cursor: None,
                    limit: None,
                })
                .await?;
            let response: PluginSearchResponse =
                timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;

            assert_eq!(
                response
                    .data
                    .iter()
                    .map(|result| result.plugin.name.as_str())
                    .collect::<Vec<_>>(),
                expected_names
            );
            assert_eq!(response.next_cursor, None);
        }

        assert!(
            server
                .received_requests()
                .await
                .expect("wiremock should record requests")
                .is_empty()
        );
    }
    Ok(())
}

#[tokio::test]
async fn plugin_search_deduplicates_shared_remote_identities_while_ignoring_local_curated_plugins()
-> Result<()> {
    let codex_home = TempDir::new()?;
    let repo_root = TempDir::new()?;
    let curated_root = codex_home.path().join(".tmp/plugins");
    let server = MockServer::start().await;
    write_local_marketplace(
        &curated_root,
        "openai-curated",
        "marketplace.json",
        &[
            LocalPluginFixture {
                name: "calendar",
                display_name: "Calendar",
                keywords: &[],
                description: "Built-in calendar",
            },
            LocalPluginFixture {
                name: "calendar-stale",
                display_name: "Stale Calendar",
                keywords: &[],
                description: "Removed from the remote curated catalog",
            },
        ],
    )?;
    let personal_marketplace_path = write_local_marketplace(
        repo_root.path(),
        "personal-tools",
        "marketplace.json",
        &[
            LocalPluginFixture {
                name: "local-planner",
                display_name: "Local Planner",
                keywords: &[],
                description: "Shared calendar",
            },
            LocalPluginFixture {
                name: "calendar-local-only",
                display_name: "Calendar Local Only",
                keywords: &[],
                description: "Enabled local calendar",
            },
        ],
    )?;
    let shared_plugin_path = repo_root.path().join("plugins/local-planner");
    std::fs::create_dir_all(codex_home.path().join(".tmp"))?;
    std::fs::write(
        codex_home
            .path()
            .join(".tmp/plugin-share-local-paths-v1.json"),
        serde_json::to_string(&json!({
            "localPluginPathsByRemotePluginId": {
                "remote-shared": shared_plugin_path,
            },
        }))?,
    )?;
    write_installed_plugin(codex_home.path(), "openai-curated", "calendar")?;
    write_installed_plugin(codex_home.path(), "personal-tools", "local-planner")?;
    write_installed_plugin(codex_home.path(), "personal-tools", "calendar-local-only")?;
    std::fs::write(
        codex_home.path().join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
remote_plugin = true

[plugins."calendar@openai-curated"]
enabled = true

[plugins."local-planner@personal-tools"]
enabled = true

[plugins."calendar-local-only@personal-tools"]
enabled = true
"#,
            server.uri()
        ),
    )?;
    write_chatgpt_search_auth(codex_home.path())?;

    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/search"))
        .and(query_param("q", "calendar"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": [
                remote_plugin_json(
                    "remote-curated",
                    "calendar",
                    "GLOBAL",
                    /*discoverability*/ None,
                ),
                remote_plugin_json(
                    "remote-shared",
                    "remote-calendar",
                    "WORKSPACE",
                    Some("LISTED"),
                ),
            ],
            "pagination": {"next_page_token": null},
        })))
        .expect(1)
        .mount(&server)
        .await;

    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_auto_env()
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    let request_id = app_server
        .send_plugin_search_request(PluginSearchParams {
            search_term: "calendar".to_string(),
            scope: None,
            cwds: Some(vec![AbsolutePathBuf::try_from(repo_root.path())?]),
            cursor: None,
            limit: None,
        })
        .await?;
    let raw_response: serde_json::Value =
        timeout(DEFAULT_TIMEOUT, app_server.read_response(request_id)).await??;
    assert_eq!(
        raw_response["data"]
            .as_array()
            .expect("plugin/search should return a data array")
            .iter()
            .map(|result| result["plugin"]
                .get("enabled")
                .and_then(serde_json::Value::as_bool))
            .collect::<Vec<_>>(),
        vec![Some(false), Some(false), Some(false)]
    );
    let response: PluginSearchResponse = serde_json::from_value(raw_response)?;

    assert_eq!(
        response
            .data
            .iter()
            .map(|result| (
                result.plugin.id.as_str(),
                result.plugin.installed,
                result.plugin.enabled,
                result.marketplace_path.as_ref(),
            ))
            .collect::<Vec<_>>(),
        vec![
            ("calendar@openai-curated-remote", false, false, None),
            (
                "calendar-local-only@personal-tools",
                true,
                false,
                Some(&personal_marketplace_path),
            ),
            ("remote-calendar@workspace-directory", true, false, None),
        ]
    );
    Ok(())
}

struct LocalPluginFixture<'a> {
    name: &'a str,
    display_name: &'a str,
    keywords: &'a [&'a str],
    description: &'a str,
}

fn write_local_marketplace(
    root: &std::path::Path,
    marketplace_name: &str,
    manifest_name: &str,
    plugins: &[LocalPluginFixture<'_>],
) -> Result<AbsolutePathBuf> {
    std::fs::create_dir_all(root.join(".git"))?;
    std::fs::create_dir_all(root.join(".agents/plugins"))?;
    let marketplace_path = root.join(".agents/plugins").join(manifest_name);
    let entries = plugins
        .iter()
        .map(|plugin| {
            json!({
                "name": plugin.name,
                "source": {
                    "source": "local",
                    "path": format!("./plugins/{}", plugin.name),
                },
            })
        })
        .collect::<Vec<_>>();
    std::fs::write(
        &marketplace_path,
        serde_json::to_string(&json!({
            "name": marketplace_name,
            "plugins": entries,
        }))?,
    )?;

    for plugin in plugins {
        let plugin_manifest = root.join("plugins").join(plugin.name).join(".codex-plugin");
        std::fs::create_dir_all(&plugin_manifest)?;
        std::fs::write(
            plugin_manifest.join("plugin.json"),
            serde_json::to_string(&json!({
                "name": plugin.name,
                "keywords": plugin.keywords,
                "interface": {
                    "displayName": plugin.display_name,
                    "shortDescription": plugin.description,
                },
            }))?,
        )?;
    }

    Ok(AbsolutePathBuf::try_from(marketplace_path)?)
}

fn write_remote_plugin_search_config(
    codex_home: &std::path::Path,
    server: &MockServer,
    remote_plugin: bool,
) -> Result<()> {
    std::fs::write(
        codex_home.join("config.toml"),
        format!(
            r#"chatgpt_base_url = "{}/backend-api/"

[features]
plugins = true
remote_plugin = {remote_plugin}
"#,
            server.uri()
        ),
    )?;
    Ok(())
}

fn write_chatgpt_search_auth(codex_home: &std::path::Path) -> Result<()> {
    write_chatgpt_auth(
        codex_home,
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    Ok(())
}

fn write_installed_plugin(
    codex_home: &std::path::Path,
    marketplace_name: &str,
    plugin_name: &str,
) -> Result<()> {
    let plugin_manifest = codex_home
        .join("plugins/cache")
        .join(marketplace_name)
        .join(plugin_name)
        .join("1.2.3")
        .join(".codex-plugin");
    std::fs::create_dir_all(&plugin_manifest)?;
    std::fs::write(
        plugin_manifest.join("plugin.json"),
        serde_json::to_string(&json!({
            "name": plugin_name,
            "version": "1.2.3",
            "interface": {"displayName": plugin_name},
        }))?,
    )?;
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
