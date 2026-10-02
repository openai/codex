use super::*;
use crate::legacy_core::config::ConfigOverrides;
use codex_config::LoaderOverrides;
use pretty_assertions::assert_eq;
use std::fs;
use std::path::Path;
use std::process::Command;

fn git(root: &Path, args: &[&str]) {
    let result = Command::new("git")
        .current_dir(root)
        .args([
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "-c",
            "commit.gpgSign=false",
        ])
        .args(args)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
}

fn value(response: DynamicToolCallResponse) -> Value {
    assert!(response.success, "{response:?}");
    let [DynamicToolCallOutputContentItem::InputText { text }] = response.content_items.as_slice()
    else {
        panic!("expected text response")
    };
    serde_json::from_str(text).unwrap()
}

async fn start_server(
    config: &Config,
) -> color_eyre::Result<crate::app_server_session::AppServerSession> {
    let mut server = crate::start_embedded_app_server_for_picker(config).await?;
    let (events, _events_rx) = tokio::sync::mpsc::unbounded_channel();
    let (status, _status_rx) = tokio::sync::broadcast::channel(16);
    server
        .start_dynamic_tool_mcp(
            config.clone(),
            crate::app_event_sender::AppEventSender::new(events),
            status,
        )
        .await?;
    let mut overrides = None;
    server.thread_tool_transport().configure_mcp(&mut overrides);
    let overrides = overrides.unwrap();
    assert!(!overrides.contains_key("mcp_servers.codex_tui"));
    let mcp = &overrides["mcp_servers.codex_worktrees"];
    let response = codex_http_client::HttpClientBuilder::new().build_direct()?
        .post(mcp["url"].as_str().unwrap())
        .header("Authorization", mcp["http_headers"]["Authorization"].as_str().unwrap())
        .header("Accept", "application/json, text/event-stream")
        .header("MCP-Protocol-Version", "2026-07-28")
        .header("MCP-Method", "tools/list")
        .json(&json!({"jsonrpc":"2.0","id":1,"method":"tools/list","params":{"_meta":{
            "io.modelcontextprotocol/protocolVersion":"2026-07-28","io.modelcontextprotocol/clientCapabilities":{}}}}))
        .send().await?.text().await?;
    let result: Value = serde_json::from_str(
        response
            .lines()
            .find_map(|line| line.strip_prefix("data: "))
            .unwrap_or(&response),
    )?;
    assert_eq!(
        result["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec![
            "create_worktree",
            "get_worktree_creation_status",
            "list_worktrees"
        ]
    );
    Ok(server)
}

#[tokio::test]
async fn creation_attaches_to_original_task_and_survives_service_restart() -> color_eyre::Result<()>
{
    let home = tempfile::tempdir()?;
    let responses =
        app_test_support::create_mock_responses_server_repeating_assistant("Ready").await;
    app_test_support::MockResponsesConfig::new(&responses.uri()).write(home.path())?;
    let repository = home.path().join("project");
    fs::create_dir(&repository)?;
    git(&repository, &["init", "--initial-branch=main"]);
    git(&repository, &["config", "core.autocrlf", "false"]);
    fs::write(repository.join("tracked.txt"), "base\n")?;
    git(&repository, &["add", "."]);
    git(&repository, &["commit", "-m", "base"]);
    fs::write(repository.join("tracked.txt"), "source edits\n")?;
    let mut config = ConfigBuilder::default()
        .codex_home(home.path().to_path_buf())
        .harness_overrides(ConfigOverrides {
            cwd: Some(home.path().to_path_buf()),
            ..Default::default()
        })
        .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
        .build()
        .await?;
    config.features.enable(codex_features::Feature::Worktrees)?;
    let mut server = Box::pin(start_server(&config)).await?;
    let mut conflicting = config.clone();
    let raw: codex_config::RawMcpServerConfig =
        serde_json::from_value(json!({"url":"http://127.0.0.1:1/mcp"}))?;
    let mut servers = conflicting.mcp_servers.get().clone();
    servers.insert(
        "codex_worktrees".to_owned(),
        codex_config::McpServerConfig::try_from(raw).map_err(color_eyre::eyre::Report::msg)?,
    );
    conflicting.mcp_servers.set(servers)?;
    let error = Box::pin(server.start_thread(&conflicting))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("MCP server owns"));
    let started = Box::pin(server.start_thread(&config)).await?;
    assert!(!started.task_tools_available);
    let thread_id = started.session.thread_id.to_string();
    let _: codex_app_server_protocol::TurnStartResponse = server
        .request_handle()
        .request_typed(ClientRequest::TurnStart {
            request_id: request_id(),
            params: codex_app_server_protocol::TurnStartParams {
                thread_id: thread_id.clone(),
                cwd: Some(home.path().to_path_buf()),
                environments: Some(serde_json::from_value(json!([
                    {"environmentId":"local", "cwd":repository}
                ]))?),
                input: vec![codex_app_server_protocol::UserInput::Text {
                    text: "Prepare to work".to_owned(),
                    text_elements: Vec::new(),
                }],
                ..Default::default()
            },
        })
        .await?;
    tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if matches!(server.next_event().await, Some(codex_app_server_client::AppServerEvent::ServerNotification(notification)) if matches!(notification.as_ref(), codex_app_server_protocol::ServerNotification::TurnCompleted(_))) { break; }
        }
    }).await?;
    let tools = ManagedWorktreeTools::new(&config)
        .await
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    assert!(
        !tools
            .execute(
                server.request_handle(),
                Uuid::new_v4().to_string(),
                "create_worktree",
                json!({"async":true})
            )
            .await
            .success
    );
    assert!(!tools.manager.settings().root.exists());
    let mut ephemeral_config = config.clone();
    ephemeral_config.ephemeral = true;
    let ephemeral = Box::pin(server.start_thread(&ephemeral_config)).await?;
    let refused = tools
        .execute(
            server.request_handle(),
            ephemeral.session.thread_id.to_string(),
            "create_worktree",
            json!({"allowAsync":true}),
        )
        .await;
    assert!(!refused.success, "{refused:?}");
    assert!(format!("{refused:?}").contains("Ephemeral side conversations"));
    assert!(!tools.manager.settings().root.exists());
    assert!(tools.operations.lock().await.is_empty());
    let call = |tool: &'static str, arguments| {
        tools.execute(server.request_handle(), thread_id.clone(), tool, arguments)
    };
    let mut enterprise_loader = LoaderOverrides::without_managed_config_for_tests();
    enterprise_loader.ignore_user_config = true;
    for builder in [
        ConfigBuilder::default().codex_home(home.path().to_path_buf())
            .loader_overrides(LoaderOverrides::without_managed_config_for_tests())
            .cli_overrides(vec![("projects".to_owned(), toml::Value::try_from(json!({codex_config::loader::project_trust_key(&repository): {"trust_level":"untrusted"}}))?)]),
        ConfigBuilder::default().codex_home(home.path().to_path_buf())
            .loader_overrides(enterprise_loader)
            .cloud_config_bundle(codex_config::test_support::CloudConfigBundleFixture::loader_with_enterprise_config(format!("[projects.{:?}]\ntrust_level = \"untrusted\"\n", codex_config::loader::project_trust_key(&repository)))),
    ] {
        let restricted = tools.clone().with_source_config_builder(builder);
        let refused = restricted.execute(server.request_handle(), thread_id.clone(), "create_worktree", json!({"async":true})).await;
        assert!(!refused.success, "effective source distrust must survive reload");
        assert!(format!("{refused:?}").contains("explicitly untrusted"), "{refused:?}");
        assert!(!tools.manager.settings().root.exists());
    }
    let pending = value(call("create_worktree", json!({"async":true})).await);
    let operation_id = pending["operationId"].as_str().unwrap();
    let created = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            let result = value(
                call(
                    "get_worktree_creation_status",
                    json!({"operationId":operation_id}),
                )
                .await,
            );
            if result["type"] != "pending" {
                break result;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await?;
    assert_eq!(created["type"], "created");
    let checkout = PathBuf::from(created["worktree"]["root"].as_str().unwrap());
    assert_eq!(
        (
            fs::read_to_string(repository.join("tracked.txt"))?,
            fs::read_to_string(checkout.join("tracked.txt"))?
        ),
        ("source edits\n".to_owned(), "base\n".to_owned())
    );
    let read: ThreadReadResponse = server
        .request_handle()
        .request_typed(ClientRequest::ThreadRead {
            request_id: request_id(),
            params: ThreadReadParams {
                thread_id: thread_id.clone(),
                include_turns: false,
            },
        })
        .await?;
    assert_eq!(read.thread.cwd.as_path(), dunce::canonicalize(home.path())?);
    let fresh = ManagedWorktreeTools::new(&config)
        .await
        .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?;
    assert!(
        !fresh
            .execute(
                server.request_handle(),
                thread_id.clone(),
                "get_worktree_creation_status",
                json!({"operationId":operation_id})
            )
            .await
            .success
    );
    let page = attachments(&server.request_handle(), &thread_id, /*cursor*/ None)
        .await
        .map_err(|error| color_eyre::eyre::eyre!(error))?;
    assert_eq!(
        (page.data[0].attachment_type.as_str(), &page.data[0].payload),
        ("worktree", &created["worktree"])
    );
    assert_eq!(
        fresh
            .manager
            .owner(&checkout)
            .map_err(|error| color_eyre::eyre::eyre!(error.to_string()))?,
        Some(thread_id.clone())
    );
    assert!(
        !tools
            .execute(
                server.request_handle(),
                "another-task".to_owned(),
                "get_worktree_creation_status",
                json!({"operationId":operation_id})
            )
            .await
            .success
    );
    // APFS rejects invalid UTF-8 names; exercise this serialization panic on Linux.
    #[cfg(target_os = "linux")]
    {
        use std::os::unix::ffi::OsStringExt;
        let invalid = home
            .path()
            .join(std::ffi::OsString::from_vec(b"invalid-\xff".to_vec()));
        fs::create_dir(&invalid)?;
        let mut supervised = ManagedWorktreeTools::new(&config).await.unwrap();
        let mut settings = supervised.manager.settings().clone();
        settings.root = invalid;
        supervised.manager = WorktreeManager::new(settings);
        let pending = value(
            supervised
                .execute(
                    server.request_handle(),
                    thread_id.clone(),
                    "create_worktree",
                    json!({"async":true}),
                )
                .await,
        );
        let result = tokio::time::timeout(Duration::from_secs(/*secs*/ 20), async {
            loop {
                let result = supervised
                    .execute(
                        server.request_handle(),
                        thread_id.clone(),
                        "get_worktree_creation_status",
                        json!({"operationId":pending["operationId"]}),
                    )
                    .await;
                if !result.success {
                    break result;
                }
                tokio::time::sleep(Duration::from_millis(/*millis*/ 20)).await;
            }
        })
        .await?;
        assert!(
            format!("{result:?}").contains("outcome is uncertain"),
            "{result:?}"
        );
        assert!(
            supervised
                .operations
                .lock()
                .await
                .values()
                .all(|operation| operation.result.is_some())
        );
        assert_eq!(supervised.manager.list(&repository).unwrap().len(), 1);
    }
    server.shutdown().await?;
    let mut resumed_server = Box::pin(start_server(&config)).await?;
    let resumed = Box::pin(resumed_server.resume_thread(
        &crate::local_settings::LocalSettings::from(&config),
        config.clone(),
        codex_protocol::ThreadId::from_string(&thread_id)?,
        crate::app_server_session::ResumeModelSettings::RestoreFromThread,
    ))
    .await?;
    assert!(!resumed.task_tools_available);
    assert_eq!(
        value(
            fresh
                .execute(
                    resumed_server.request_handle(),
                    thread_id,
                    "list_worktrees",
                    json!({})
                )
                .await
        )["data"][0]["root"],
        created["worktree"]["root"]
    );
    Box::pin(model_tests::exercise(
        &mut resumed_server,
        &responses,
        &config,
        &repository,
    ))
    .await?;
    resumed_server.shutdown().await?;
    Ok(())
}

#[path = "managed_worktree_model_tests.rs"]
mod model_tests;
