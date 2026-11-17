//! Exercises installed-metadata refreshes against live MCP and bundled plugin skills.

use super::*;
use axum::Json;
use axum::routing::get;
use codex_app_server_protocol::PluginInstalledResponse;
use codex_app_server_protocol::PluginReconcileResponse;
use codex_app_server_protocol::SkillMetadata;
use codex_app_server_protocol::SkillsListParams;
use codex_app_server_protocol::SkillsListResponse;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use tokio::sync::RwLock;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn catalog_metadata_updates_preserve_live_mcp_and_plugin_skills() -> Result<()> {
    let responses_server = responses::start_mock_server().await;
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let base_url = format!("http://{}", listener.local_addr()?);
    let original_logo = "https://files.openai.com/plugins/logo.png?sv=1&sr=b&sig=first&se=old";
    let renewed_logo = "https://files.openai.com/plugins/logo.png?sv=1&sr=b&sig=second&se=new";
    let changed_logo = "https://files.openai.com/plugins/new-logo.png?sv=1&sr=b&sig=third&se=new";
    let installed = Arc::new(RwLock::new(json!({
        "id": "plugins~Plugin_00000000000000000000000000000000",
        "name": "demo-plugin",
        "scope": "GLOBAL",
        "installation_policy": "AVAILABLE",
        "authentication_policy": "ON_USE",
        "release": {
            "version": "1.0.0",
            "display_name": "Demo plugin",
            "description": "Test plugin",
            "bundle_download_url": format!("{base_url}/bundle"),
            "interface": {"logo_url": original_logo},
        },
        "enabled": true,
    })));
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for (path, contents) in [
        (".codex-plugin/plugin.json", r#"{"name":"demo-plugin"}"#),
        ("skills/deploy/SKILL.md", SKILL_CONTENTS),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(/*mode*/ 0o644);
        header.set_cksum();
        archive.append_data(&mut header, path, contents.as_bytes())?;
    }
    let bundle = archive.into_inner()?.finish()?;

    let calls = Arc::new(ResourceAppsMcpCalls::default());
    let sessions = Arc::new(AtomicUsize::new(0));
    let server_calls = Arc::clone(&calls);
    let server_sessions = Arc::clone(&sessions);
    let mcp_service = StreamableHttpService::new(
        move || {
            server_sessions.fetch_add(1, Ordering::SeqCst);
            Ok(ResourceAppsMcpServer {
                calls: Arc::clone(&server_calls),
            })
        },
        Arc::new(LocalSessionManager::default()),
        StreamableHttpServerConfig::default(),
    );
    let server_installed = Arc::clone(&installed);
    let router = Router::new()
        .nest_service("/api/codex/ps/mcp", mcp_service)
        .route(
            "/ps/plugins/installed",
            get(move || {
                let installed = Arc::clone(&server_installed);
                async move {
                    Json(json!({
                        "plugins": [installed.read().await.clone()],
                        "pagination": {"limit": 200, "next_page_token": null},
                    }))
                }
            }),
        )
        .route("/bundle", get(move || async move { bundle }));
    let server_handle = tokio::spawn(async move {
        let _ = axum::serve(listener, router).await;
    });

    let codex_home = TempDir::new()?;
    MockResponsesConfig::new(&responses_server.uri())
        .with_root_config(&format!("chatgpt_base_url = \"{base_url}\""))
        .enable_feature(Feature::Apps)
        .enable_feature(Feature::Plugins)
        .enable_feature(Feature::RemotePlugin)
        .disable_feature(Feature::PluginSharing)
        .with_extra_config("[skills]\ninclude_instructions = true")
        .write(codex_home.path())?;
    write_chatgpt_auth(
        codex_home.path(),
        ChatGptAuthFixture::new("chatgpt-token")
            .account_id("account-123")
            .chatgpt_user_id("user-123")
            .chatgpt_account_id("account-123"),
        AuthCredentialsStoreMode::File,
    )?;
    let mut app_server = TestAppServer::builder()
        .with_codex_home(codex_home.path())
        .without_managed_config()
        .with_env_overrides(&[(
            "CODEX_TEST_ALLOW_HTTP_REMOTE_PLUGIN_BUNDLE_DOWNLOADS",
            Some("1"),
        )])
        .build_initialized()
        .await?;
    refresh_and_expect_logo(&mut app_server, original_logo).await?;
    // MCP resource reads do not require an executor or a registered cloud-skill provider.
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.5".to_string()),
            environments: Some(Vec::new()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;

    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    let warm_sessions = sessions.load(Ordering::SeqCst);
    assert!(warm_sessions > 0);
    let warm_skills = list_plugin_skills(&mut app_server, codex_home.path()).await?;
    assert_eq!(warm_skills.len(), 1);
    assert_eq!(warm_skills[0].name, SKILL_NAME);
    assert_eq!(
        warm_skills[0].description,
        "Deploy through the orchestrator."
    );
    assert!(warm_skills[0].enabled);
    assert_eq!(
        std::fs::read_to_string(warm_skills[0].path.as_str())?,
        SKILL_CONTENTS
    );

    installed.write().await["release"]["interface"]["logo_url"] = json!(renewed_logo);
    refresh_and_expect_logo(&mut app_server, renewed_logo).await?;
    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(
        list_plugin_skills(&mut app_server, codex_home.path()).await?,
        warm_skills
    );

    installed.write().await["release"]["interface"]["capabilities"] =
        json!(["Updated store badge"]);
    refresh_and_expect_logo(&mut app_server, renewed_logo).await?;
    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(
        list_plugin_skills(&mut app_server, codex_home.path()).await?,
        warm_skills
    );

    installed.write().await["installed_at"] = json!("2026-11-16T12:00:00Z");
    refresh_and_expect_logo(&mut app_server, renewed_logo).await?;
    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);

    {
        let mut plugin = installed.write().await;
        plugin["release"]["version"] = json!("1.1.0");
        plugin["release"]["bundle_download_url"] = json!(format!("{base_url}/missing-bundle"));
    }
    let request = app_server
        .send_raw_request("plugin/installed", Some(json!({})))
        .await?;
    let _: PluginInstalledResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    let request = app_server
        .send_raw_request("plugin/reconcile", Some(json!({})))
        .await?;
    let reconciled: PluginReconcileResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    assert!(reconciled.changed_plugins.is_empty());
    assert_eq!(
        reconciled.failed_remote_plugin_ids,
        vec!["plugins~Plugin_00000000000000000000000000000000"]
    );
    let request = app_server
        .send_raw_request("plugin/installed", Some(json!({})))
        .await?;
    let response: PluginInstalledResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    let plugin = response
        .marketplaces
        .iter()
        .flat_map(|marketplace| &marketplace.plugins)
        .find(|plugin| plugin.name == "demo-plugin")
        .context("installed plugin should remain visible after an advertised upgrade")?;
    assert_eq!(plugin.version.as_deref(), Some("1.1.0"));
    assert_eq!(plugin.local_version.as_deref(), Some("1.0.0"));
    let request = app_server
        .send_raw_request("plugin/reconcile", Some(json!({})))
        .await?;
    let _: PluginReconcileResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(
        list_plugin_skills(&mut app_server, codex_home.path()).await?,
        warm_skills
    );

    // Runtime metadata must update the cached skill catalog without changing the bundle.
    {
        let mut plugin = installed.write().await;
        plugin["release"]["version"] = json!("1.0.0");
        plugin["release"]["bundle_download_url"] = json!(format!("{base_url}/bundle"));
        plugin["release"]["interface"]["logo_url"] = json!(changed_logo);
        plugin["enabled"] = json!(false);
    }
    refresh_and_expect_logo(&mut app_server, changed_logo).await?;
    assert!(
        list_plugin_skills(&mut app_server, codex_home.path())
            .await?
            .is_empty()
    );
    read_mcp_skill_resource(&mut app_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(calls.snapshot().main_prompt_reads, 6);
    assert_eq!(
        std::fs::read_to_string(warm_skills[0].path.as_str())?,
        SKILL_CONTENTS
    );
    // Restore the plugin, then simulate a different process activating a new bundle.
    let plugin_root = codex_home
        .path()
        .join("plugins/cache/openai-curated-remote/demo-plugin");
    let original_root = plugin_root.join("1.0.0");
    write_fixture_mcp(&original_root, "fixture-original")?;
    installed.write().await["enabled"] = json!(true);
    reconcile_plugins(&mut app_server).await?;
    expect_fixture_mcp(&mut app_server, &thread.id, "fixture-original").await?;

    {
        let mut plugin = installed.write().await;
        plugin["release"]["version"] = json!("1.1.0");
        plugin["release"]["bundle_download_url"] = json!(format!("{base_url}/missing-bundle"));
    }
    let failed = reconcile_plugins(&mut app_server).await?;
    assert!(failed.changed_plugins.is_empty());
    assert_eq!(failed.failed_remote_plugin_ids.len(), 1);
    expect_fixture_mcp(&mut app_server, &thread.id, "fixture-original").await?;

    let upgraded_root = plugin_root.join("1.1.0");
    std::fs::rename(&original_root, &upgraded_root)?;
    write_fixture_mcp(&upgraded_root, "fixture-upgrade")?;
    assert!(
        reconcile_plugins(&mut app_server)
            .await?
            .changed_plugins
            .is_empty()
    );
    expect_fixture_mcp(&mut app_server, &thread.id, "fixture-upgrade").await?;

    // A different remote identity can replace the bundle without changing its directory version.
    let replacement_id = "plugins~Plugin_00000000000000000000000000000001";
    installed.write().await["id"] = json!(replacement_id);
    let failed = reconcile_plugins(&mut app_server).await?;
    assert_eq!(failed.failed_remote_plugin_ids, vec![replacement_id]);
    expect_fixture_mcp(&mut app_server, &thread.id, "fixture-upgrade").await?;
    write_fixture_mcp(&upgraded_root, "fixture-replacement")?;
    std::fs::write(
        plugin_root.join(".codex-remote-plugin-install.json"),
        serde_json::to_vec(&json!({"schema_version": 1, "remote_plugin_id": replacement_id}))?,
    )?;
    assert!(
        reconcile_plugins(&mut app_server)
            .await?
            .changed_plugins
            .is_empty()
    );
    expect_fixture_mcp(&mut app_server, &thread.id, "fixture-replacement").await?;

    server_handle.abort();
    Ok(())
}

fn write_fixture_mcp(root: &std::path::Path, name: &str) -> Result<()> {
    std::fs::write(
        root.join(".mcp.json"),
        serde_json::to_vec(&json!({
            "mcpServers": {(name): {"type": "http", "url": "http://127.0.0.1:9/mcp"}}
        }))?,
    )?;
    Ok(())
}

async fn reconcile_plugins(app_server: &mut TestAppServer) -> Result<PluginReconcileResponse> {
    let request = app_server
        .send_raw_request("plugin/reconcile", Some(json!({})))
        .await?;
    timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await?
}

async fn expect_fixture_mcp(
    app_server: &mut TestAppServer,
    thread_id: &str,
    expected: &str,
) -> Result<()> {
    timeout(DEFAULT_READ_TIMEOUT, async {
        loop {
            let request = app_server
                .send_raw_request("mcpServerStatus/list", Some(json!({"threadId": thread_id})))
                .await?;
            let response: serde_json::Value = app_server.read_response(request).await?;
            let names = response["data"]
                .as_array()
                .context("missing MCP status data")?
                .iter()
                .filter_map(|row| row["name"].as_str())
                .filter(|name| name.starts_with("fixture-"))
                .collect::<Vec<_>>();
            if names == [expected] {
                return Ok::<(), anyhow::Error>(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        }
    })
    .await??;
    Ok(())
}

async fn refresh_and_expect_logo(app_server: &mut TestAppServer, logo: &str) -> Result<()> {
    // plugin/installed starts the real background sync with the production callback.
    // Reconcile acquires the same gate, so its completion fences the background pass.
    let request = app_server
        .send_raw_request("plugin/installed", Some(json!({})))
        .await?;
    let _: PluginInstalledResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    let request = app_server
        .send_raw_request("plugin/reconcile", Some(json!({})))
        .await?;
    let reconciled: PluginReconcileResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    assert!(reconciled.failed_remote_plugin_ids.is_empty());
    assert!(
        reconciled
            .failed_materialization_remote_plugin_ids
            .is_empty()
    );
    let request = app_server
        .send_raw_request("plugin/installed", Some(json!({})))
        .await?;
    let installed: PluginInstalledResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    let plugin = installed
        .marketplaces
        .iter()
        .flat_map(|marketplace| &marketplace.plugins)
        .find(|plugin| plugin.name == "demo-plugin")
        .context("installed plugin should be exposed through the public API")?;
    assert_eq!(
        plugin
            .interface
            .as_ref()
            .and_then(|info| info.logo_url.as_deref()),
        Some(logo)
    );
    // Finish the follow-up read's background pass before changing the next snapshot.
    let request = app_server
        .send_raw_request("plugin/reconcile", Some(json!({})))
        .await?;
    let _: PluginReconcileResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    Ok(())
}

async fn list_plugin_skills(
    app_server: &mut TestAppServer,
    cwd: &Path,
) -> Result<Vec<SkillMetadata>> {
    let request = app_server
        .send_skills_list_request(SkillsListParams {
            cwds: vec![cwd.to_path_buf()],
            force_reload: false,
        })
        .await?;
    let response: SkillsListResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    assert_eq!(response.data.len(), 1);
    assert!(response.data[0].errors.is_empty());
    Ok(response
        .data
        .into_iter()
        .flat_map(|entry| entry.skills)
        .filter(|skill| skill.plugin_id.as_deref() == Some("demo-plugin@openai-curated-remote"))
        .collect())
}

async fn read_mcp_skill_resource(app_server: &mut TestAppServer, thread_id: &str) -> Result<()> {
    let request = app_server
        .send_mcp_resource_read_request(McpResourceReadParams {
            environment_id: None,
            meta: None,
            thread_id: Some(thread_id.to_string()),
            origin_call_id: None,
            server: "codex_apps".to_string(),
            uri: SKILL_MAIN_PROMPT_URI.to_string(),
            connector_id: None,
            target: None,
        })
        .await?;
    let response: McpResourceReadResponse =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request)).await??;
    assert_eq!(
        response,
        McpResourceReadResponse {
            contents: vec![McpResourceContent::Text {
                uri: SKILL_MAIN_PROMPT_URI.to_string(),
                mime_type: Some("text/markdown".to_string()),
                text: SKILL_CONTENTS.to_string(),
                meta: None,
            }],
            origin_call_id: None,
        }
    );
    Ok(())
}
