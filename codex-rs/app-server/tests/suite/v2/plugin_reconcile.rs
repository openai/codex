//! Exercises blocking plugin reconciliation through the public JSON-RPC API.

use std::time::Duration;

use anyhow::Result;
use app_test_support::ChatGptAuthFixture;
use app_test_support::MockResponsesConfig;
use app_test_support::TestAppServer;
use app_test_support::write_chatgpt_auth;
use codex_app_server_protocol::PluginReconcileChangedPlugin;
use codex_app_server_protocol::PluginReconcileResponse;
use codex_app_server_protocol::TurnStartParams;
use codex_app_server_protocol::TurnStatus;
use codex_app_server_protocol::UserInput;
use codex_config::types::AuthCredentialsStoreMode;
use codex_features::Feature;
use core_test_support::responses;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use serde_json::Value;
use serde_json::json;
use tempfile::TempDir;
use test_case::test_case;
use tokio::time::timeout;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::header;
use wiremock::matchers::method;
use wiremock::matchers::path;
use wiremock::matchers::query_param;

use super::analytics::wait_for_matching_analytics_event;

const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
const REMOTE_PLUGIN_ID: &str = "plugins~Plugin_00000000000000000000000000000000";

#[test_case("WORKSPACE", "workspace-directory"; "workspace")]
#[test_case("GLOBAL", "openai-curated-remote"; "global")]
#[tokio::test]
async fn plugin_reconcile_syncs_bundles_and_reports_changes(
    scope: &str,
    marketplace: &str,
) -> Result<()> {
    let server = MockServer::start().await;
    // Both scopes must retain background-sync behavior with remote_plugin disabled.
    let (mut app_server, codex_home) = start_app_server(&server).await?;
    let thread = app_server.start_thread(Default::default()).await?.thread;
    let bundle_url = format!("{}/bundle", server.uri());
    let plugin_id = format!("linear@{marketplace}");
    let first = PluginReconcileChangedPlugin {
        id: plugin_id.clone(),
        has_mcps: true,
        has_apps: false,
        has_hooks: true,
        has_skills: true,
    };
    let second = PluginReconcileChangedPlugin {
        id: plugin_id.clone(),
        has_mcps: false,
        has_apps: true,
        has_hooks: false,
        has_skills: false,
    };
    let updated = PluginReconcileChangedPlugin {
        id: plugin_id,
        has_mcps: true,
        has_apps: true,
        has_hooks: true,
        has_skills: true,
    };
    let failed_id = "plugins~Plugin_11111111111111111111111111111111";
    let mut missing_bundle = installed_plugin("1.0.0", &bundle_url, "WORKSPACE");
    missing_bundle["id"] = json!(failed_id);
    missing_bundle["name"] = json!("missing-bundle");
    missing_bundle["release"]["bundle_download_url"] = Value::Null;
    let mut first_pass_failure = Some(missing_bundle);

    // v1.1 drops hooks; v2 restores them and drops Apps. Both updates must report
    // both sides, while uninstall below reports only v2's remaining capabilities.
    // Enablement changes report cached capabilities without downloading the bundle again.
    for (version, enabled, capabilities, expected_plugins, expected_downloads) in [
        ("1.0.0", true, &first, vec![first.clone()], 1),
        ("1.0.0", true, &first, Vec::new(), 0),
        ("1.0.0", false, &first, vec![first.clone()], 0),
        ("1.0.0", true, &first, vec![first.clone()], 0),
        ("1.1.0", true, &second, vec![updated.clone()], 1),
        ("2.0.0", true, &first, vec![updated], 1),
    ] {
        let mut files = vec![(
            ".codex-plugin/plugin.json",
            json!({"name": "linear"}).to_string(),
        )];
        if capabilities.has_mcps {
            files.push((
                ".mcp.json",
                json!({"mcpServers": {"example": {"command": "unused", "enabled": false}}})
                    .to_string(),
            ));
        }
        if capabilities.has_apps {
            files.push((
                ".app.json",
                json!({"apps": {"example": {"id": "connector_example"}}}).to_string(),
            ));
        }
        if capabilities.has_hooks {
            files.push((
                "hooks/hooks.json",
                json!({"hooks": {"UserPromptSubmit": [{"hooks": [{
                    "type": "command", "command": format!("echo hook {version}")
                }]}]}})
                .to_string(),
            ));
        }
        if capabilities.has_skills {
            files.push((
                "skills/example/SKILL.md",
                "---\nname: example\ndescription: Example skill\n---\nExample skill".to_string(),
            ));
        }
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
        for (path, contents) in files {
            let mut header = tar::Header::new_gnu();
            header.set_size(contents.len() as u64);
            header.set_mode(/*mode*/ 0o644);
            header.set_cksum();
            archive.append_data(&mut header, path, contents.as_bytes())?;
        }
        let bundle = archive.into_inner()?.finish()?;
        let mut plugin = installed_plugin(version, &bundle_url, scope);
        plugin["enabled"] = json!(enabled);
        let mut plugins = vec![plugin];
        let mut failures = Vec::new();
        if let Some(failure) = first_pass_failure.take() {
            plugins.push(failure);
            failures.push(failed_id.to_string());
        }
        let expected = PluginReconcileResponse {
            changed_plugins: expected_plugins,
            failed_remote_plugin_ids: failures.clone(),
            failed_materialization_remote_plugin_ids: failures,
        };
        let active_plugin_ids = if enabled {
            vec![REMOTE_PLUGIN_ID.to_string()]
        } else {
            Vec::new()
        };
        mount_installed_snapshot(&server, plugins).await;
        Mock::given(method("GET"))
            .and(path("/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_bytes(bundle))
            .expect(expected_downloads)
            .mount(&server)
            .await;
        assert_eq!(reconcile(&mut app_server).await?, expected);
        // A turn exercises the loaded registry, not fresh hooks/list discovery. Hooks run
        // on the app-server host even when the thread uses a remote executor.
        assert_eq!(
            turn_hook_runs(
                &mut app_server,
                &server,
                thread.id.clone(),
                active_plugin_ids
            )
            .await?,
            usize::from(scope == "WORKSPACE" && enabled && capabilities.has_hooks),
        );
        server.verify().await;
        server.reset().await;
    }

    // Uninstall must report removed capabilities and stop already-loaded hooks.
    mount_installed_snapshot(&server, Vec::new()).await;
    assert_eq!(
        reconcile(&mut app_server).await?,
        PluginReconcileResponse {
            changed_plugins: vec![first],
            ..Default::default()
        }
    );
    assert_eq!(
        turn_hook_runs(&mut app_server, &server, thread.id.clone(), Vec::new()).await?,
        0
    );
    server.verify().await;
    server.reset().await;

    // The next RPC must honor the latest config without contacting the plugin service.
    let config_path = codex_home.path().join("config.toml");
    let config = std::fs::read_to_string(&config_path)?;
    std::fs::write(
        config_path,
        config.replace("plugins = true", "plugins = false"),
    )?;
    assert_eq!(
        reconcile(&mut app_server).await?,
        PluginReconcileResponse::default()
    );
    let requests = server.received_requests().await.expect("recorded requests");
    assert!(requests.iter().all(|request| {
        !request.url.path().starts_with("/backend-api/ps/plugins/")
            && request.url.path() != "/bundle"
    }));
    Ok(())
}

async fn turn_hook_runs(
    app_server: &mut TestAppServer,
    server: &MockServer,
    thread_id: String,
    active_plugin_ids: Vec<String>,
) -> Result<usize> {
    Mock::given(method("POST"))
        .and(path("/backend-api/codex/analytics-events/events"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
    let _response = responses::mount_sse_once(
        server,
        responses::sse(vec![
            responses::ev_response_created("response"),
            responses::ev_completed("response"),
        ]),
    )
    .await;
    app_server.clear_message_buffer();
    let completed = timeout(
        DEFAULT_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.clone(),
            input: vec![UserInput::Text {
                text: "run hooks".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let event = wait_for_matching_analytics_event(server, DEFAULT_TIMEOUT, |event| {
        event["event_type"] == "codex_turn_event"
            && event["event_params"]["turn_id"] == completed.turn.id
    })
    .await?;
    assert_eq!(event["event_params"]["thread_id"], thread_id);
    assert_eq!(
        event["event_params"]["active_plugin_ids_at_turn_start"],
        json!(active_plugin_ids)
    );
    Ok(app_server
        .pending_notification_methods()
        .iter()
        .filter(|method| method.as_str() == "hook/started")
        .count())
}

async fn reconcile(app_server: &mut TestAppServer) -> Result<PluginReconcileResponse> {
    let request = app_server
        .send_raw_request(
            "plugin/reconcile",
            Some(json!({"reason": "tooling_changed"})),
        )
        .await?;
    timeout(DEFAULT_TIMEOUT, app_server.read_response(request)).await?
}

async fn start_app_server(server: &MockServer) -> Result<(TestAppServer, TempDir)> {
    let codex_home = TempDir::new()?;
    let base_url = format!("{}/backend-api/", server.uri());
    MockResponsesConfig::new(&server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{base_url}\""))
        .disable_feature(Feature::Apps)
        .enable_feature(Feature::Plugins)
        .enable_feature(Feature::CodexHooks)
        .disable_feature(Feature::RemotePlugin)
        .disable_feature(Feature::PluginSharing)
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .with_env_overrides(&[(
            "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS",
            Some("1"),
        )])
        .build_initialized_with_timeout(DEFAULT_TIMEOUT)
        .await?;
    Ok((app_server, codex_home))
}

async fn mount_installed_snapshot(server: &MockServer, plugins: Vec<Value>) {
    Mock::given(method("GET"))
        .and(path("/backend-api/ps/plugins/installed"))
        .and(query_param("includeDownloadUrls", "true"))
        .and(header("authorization", "Bearer chatgpt-token"))
        .and(header("chatgpt-account-id", "account-123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "plugins": plugins,
            "pagination": { "limit": 200, "next_page_token": null },
        })))
        .expect(1)
        .mount(server)
        .await;
}

fn installed_plugin(version: &str, bundle_url: &str, scope: &str) -> Value {
    json!({
        "id": REMOTE_PLUGIN_ID,
        "name": "linear",
        "scope": scope,
        "discoverability": (scope == "WORKSPACE").then_some("LISTED"),
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
            "version": version,
            "display_name": "Linear",
            "description": "Track work in Linear",
            "bundle_download_url": bundle_url,
            "interface": {},
        },
        "enabled": true,
    })
}
