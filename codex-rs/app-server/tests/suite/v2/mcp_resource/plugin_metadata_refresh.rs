//! Exercises installed-metadata refreshes against live MCP and skill caches.

use super::*;
use axum::Json;
use axum::routing::get;
use codex_app_server_protocol::PluginInstalledResponse;
use codex_app_server_protocol::PluginReconcileResponse;
use flate2::Compression;
use flate2::write::GzEncoder;
use pretty_assertions::assert_eq;
use tokio::sync::RwLock;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn signed_image_renewal_preserves_live_mcp_and_skills() -> Result<()> {
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
    let manifest = br#"{"name":"demo-plugin"}"#;
    let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(/*mode*/ 0o644);
    header.set_cksum();
    archive.append_data(&mut header, ".codex-plugin/plugin.json", &manifest[..])?;
    let bundle = archive.into_inner()?.finish()?;

    let calls = Arc::new(ResourceAppsMcpCalls::default());
    let sessions = Arc::new(AtomicUsize::new(0));
    let server_calls = Arc::clone(&calls);
    let server_sessions = Arc::clone(&sessions);
    let mcp_service = StreamableHttpService::new(
        move || {
            server_sessions.fetch_add(1, Ordering::SeqCst);
            Ok(MetadataMcpServer(ResourceAppsMcpServer {
                calls: Arc::clone(&server_calls),
            }))
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
    // Hosted skills are exposed without a local executor, as in the other resource tests.
    let request_id = app_server
        .send_thread_start_request(ThreadStartParams {
            model: Some("gpt-5.5".to_string()),
            environments: Some(Vec::new()),
            ..Default::default()
        })
        .await?;
    let ThreadStartResponse { thread, .. } =
        timeout(DEFAULT_READ_TIMEOUT, app_server.read_response(request_id)).await??;

    read_skill_in_turn(&mut app_server, &responses_server, &thread.id).await?;
    let warm_sessions = sessions.load(Ordering::SeqCst);
    assert!(warm_sessions > 0);
    let warm_calls = calls.snapshot();
    assert_eq!(warm_calls.list_resources, 1);
    assert_eq!(warm_calls.main_prompt_reads, 1);

    installed.write().await["release"]["interface"]["logo_url"] = json!(renewed_logo);
    refresh_and_expect_logo(&mut app_server, renewed_logo).await?;
    read_skill_in_turn(&mut app_server, &responses_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(calls.snapshot(), warm_calls);

    installed.write().await["release"]["interface"]["capabilities"] =
        json!(["Updated store badge"]);
    refresh_and_expect_logo(&mut app_server, renewed_logo).await?;
    read_skill_in_turn(&mut app_server, &responses_server, &thread.id).await?;
    assert_eq!(sessions.load(Ordering::SeqCst), warm_sessions);
    assert_eq!(calls.snapshot(), warm_calls);

    // A policy change must invalidate both caches without rewriting any bundle files.
    installed.write().await["release"]["interface"]["logo_url"] = json!(changed_logo);
    installed.write().await["authentication_policy"] = json!("ON_INSTALL");
    refresh_and_expect_logo(&mut app_server, changed_logo).await?;
    read_skill_in_turn(&mut app_server, &responses_server, &thread.id).await?;
    // Core may retain the HTTP connection, but the callback must invalidate resources.
    assert_eq!(
        calls.snapshot(),
        ResourceAppsMcpCallCounts {
            list_resources: 2,
            main_prompt_reads: 2,
            reference_reads: 0,
        }
    );
    server_handle.abort();
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

async fn read_skill_in_turn(
    app_server: &mut TestAppServer,
    responses_server: &wiremock::MockServer,
    thread_id: &str,
) -> Result<()> {
    let response_mock = responses::mount_sse_sequence(
        responses_server,
        vec![
            responses::sse(vec![
                responses::ev_function_call_with_namespace(
                    "read-main",
                    "skills",
                    "read",
                    &json!({"package": SKILL_RESOURCE_URI}).to_string(),
                ),
                responses::ev_completed("read"),
            ]),
            responses::sse(vec![responses::ev_completed("done")]),
        ],
    )
    .await;
    let completed = timeout(
        DEFAULT_READ_TIMEOUT,
        app_server.start_turn_and_wait_for_completion(TurnStartParams {
            thread_id: thread_id.to_string(),
            input: vec![UserInput::Text {
                text: "Read the deployment skill.".to_string(),
                text_elements: Vec::new(),
            }],
            ..Default::default()
        }),
    )
    .await??;
    assert_eq!(completed.turn.status, TurnStatus::Completed);
    let requests = response_mock.requests();
    assert_eq!(requests.len(), 2);
    assert!(
        requests[0]
            .message_input_texts("developer")
            .iter()
            .any(|text| text.contains(SKILL_NAME))
    );
    let output = requests[1]
        .function_call_output_text("read-main")
        .context("skill read should reach the model")?;
    assert_eq!(
        serde_json::from_str::<serde_json::Value>(&output)?,
        json!({"resource": SKILL_MAIN_PROMPT_URI, "contents": SKILL_CONTENTS, "next_cursor": null})
    );
    Ok(())
}

// Reuse the resource-read fixture with a healthy, single-page skills catalog.
struct MetadataMcpServer(ResourceAppsMcpServer);

impl ServerHandler for MetadataMcpServer {
    fn get_info(&self) -> ServerInfo {
        self.0.get_info()
    }

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, rmcp::ErrorData> {
        self.0.calls.list_resources.fetch_add(1, Ordering::Relaxed);
        Ok(ListResourcesResult::with_all_items(vec![skill_resource(
            SKILL_RESOURCE_URI,
            "plugin_demo/deploy",
            RAW_SKILL_DESCRIPTION,
            "mcp/skill",
            "demo-plugin",
            "deploy",
        )]))
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ReadResourceResponse, rmcp::ErrorData> {
        self.0.read_resource(request, context).await
    }
}
