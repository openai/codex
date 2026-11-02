//! Two real agents share the detached board through ordinary HTTP MCP configuration.

use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;

#[path = "../../../message-board/tests/common/mod.rs"]
mod board_host;

#[test_case::test_case(false, false; "config")]
#[test_case::test_case(true, false; "plugin")]
#[test_case::test_case(true, true; "in_memory")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_board_is_shared_with_children(plugin: bool, in_memory: bool) -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let home = Arc::new(tempfile::tempdir()?);
    let sqlite = codex_state::SqliteConfig::new_for_testing(home.path().to_path_buf().try_into()?);
    let mut process = None;
    let mut local = None;
    let (url, authorization) = if in_memory {
        let launched = codex_message_board_mcp::launch_in_memory().await?;
        let connection = (
            launched.url().to_owned(),
            launched.authorization().to_str()?.to_owned(),
        );
        local = Some(launched);
        connection
    } else {
        let _state = codex_state::StateRuntime::init(sqlite.clone(), "openai".into()).await?;
        let command = codex_utils_cargo_bin::cargo_bin("codex-message-board-mcp")?;
        let mut child =
            tokio::process::Command::from(codex_utils_process::background_command(command))
                .arg("--sqlite-home")
                .arg(sqlite.home())
                .args(["--listen", "127.0.0.1:0", "--allowed-host", "board.example"])
                .env("CODEX_MESSAGE_BOARD_TOKEN", "http-board-test-token")
                .stdout(std::process::Stdio::piped())
                .kill_on_drop(true)
                .spawn()?;
        let ready = tokio::io::BufReader::new(child.stdout.take().context("server stdout")?)
            .lines()
            .next_line()
            .await?
            .context("server readiness")?;
        let ready: Value = serde_json::from_str(&ready)?;
        process = Some(child);
        (
            ready["url"].as_str().context("server URL")?.to_owned(),
            "Bearer http-board-test-token".to_owned(),
        )
    };
    let host = if in_memory {
        "127.0.0.1"
    } else {
        "board.example"
    };
    let endpoint = json!({"url":url,"http_headers":{"Authorization":authorization,"Host":host},"required":true,"default_tools_approval_mode":"approve"});
    if plugin {
        let manifest = codex_utils_cargo_bin::find_resource!(
            "../message-board-mcp/plugin/.codex-plugin/plugin.json"
        )?;
        let declaration_path =
            codex_utils_cargo_bin::find_resource!("../message-board-mcp/plugin/.mcp.json")?;
        let installed = home.path().join("plugins/cache/test/message-board/local");
        std::fs::create_dir_all(installed.join(".codex-plugin"))?;
        std::fs::copy(manifest, installed.join(".codex-plugin/plugin.json"))?;
        let mut declaration: Value = serde_json::from_slice(&std::fs::read(declaration_path)?)?;
        if !in_memory {
            declaration["mcpServers"]["codex_message_board"] = endpoint.clone();
        }
        std::fs::write(
            installed.join(".mcp.json"),
            serde_json::to_vec(&declaration)?,
        )?;
        std::fs::write(
            home.path().join("config.toml"),
            "[features]\nplugins = true\n[plugins.\"message-board@test\"]\nenabled = true\n",
        )?;
    }
    let configured_endpoint = endpoint.clone();
    let configured_sqlite = sqlite.clone();
    let configure_board = move |config: &mut codex_core::config::Config| {
        configure(config);
        config.sqlite = configured_sqlite.clone();
        config.ephemeral = in_memory;
        config
            .features
            .disable(Feature::AgentMessageBoard)
            .expect("disable native board");
        if !plugin || in_memory {
            // The runner endpoint overrides the packaged plugin endpoint.
            let mut servers = config.mcp_servers.get().clone();
            servers.insert(
                "codex_message_board".into(),
                serde_json::from_value(configured_endpoint.clone()).expect("HTTP config"),
            );
            config.mcp_servers.set(servers).expect("configure endpoint");
        }
    };
    let mut builder = test_codex()
        .with_home(home)
        .with_config(configure_board.clone());
    let root = builder.build_with_auto_env(&server).await?;
    responses::mount_sse_sequence(
        &server,
        vec![
            tool(
                "spawn-worker",
                "spawn_agent",
                json!({"task_name":"worker","message":"Say ready.","fork_turns":"none"}),
            ),
            done(),
            done(),
        ],
    )
    .await;
    root.submit_turn("Spawn a worker and finish your turn.")
        .await?;
    let root_id = root.session_configured.thread_id;
    let child_id = root
        .thread_manager
        .list_thread_ids()
        .await
        .into_iter()
        .find(|id| *id != root_id)
        .expect("child runtime");
    let child = root.thread_manager.get_thread(child_id).await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;

    if let Some(local) = &local {
        local.add_board(
            root_id.into(),
            Arc::new(board_host::Host {
                members: [
                    (root_id, codex_protocol::AgentPath::root()),
                    (
                        child_id,
                        codex_protocol::AgentPath::root()
                            .join("worker")
                            .expect("worker path"),
                    ),
                ]
                .into(),
                clock: Default::default(),
                fail_clock: Default::default(),
                agent_path_calls: Default::default(),
                active: Default::default(),
                fail_notifications: Default::default(),
                notifications: Default::default(),
            }),
        )?;
    }
    let board_turn = |id: &str, name: &str, arguments: Value| {
        vec![
            sse(vec![
                responses::ev_tool_search_call("find-board", &json!({"query":name,"limit":1})),
                ev_completed("find-board"),
            ]),
            sse(vec![
                ev_function_call_with_namespace(
                    id,
                    "mcp__codex_message_board",
                    name,
                    &arguments.to_string(),
                ),
                ev_completed(id),
            ]),
            done(),
        ]
    };
    let board_result = |request: &ResponsesRequest, id: &str| -> anyhow::Result<Value> {
        let output = request
            .function_call_output_text(id)
            .with_context(|| format!("missing board result {id}"))?;
        let json = output
            .split_once("\nOutput:\n")
            .map_or(output.as_str(), |(_, json)| json);
        serde_json::from_str(json).with_context(|| format!("board result: {output}"))
    };
    let posted = responses::mount_sse_sequence(
        &server,
        board_turn(
            "worker-post",
            "post",
            json!({"new_channel_name":"design","text":"Worker's durable decision."}),
        ),
    )
    .await;
    child
        .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
            text: "Find the board tools and post your decision.".into(),
            text_elements: vec![],
        }]))
        .await?;
    wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
    let post = board_result(
        posted.requests().last().context("child follow-up")?,
        "worker-post",
    )?;
    assert_eq!(post["author"], "/root/worker");
    if in_memory {
        assert_eq!(post["created_at"], "2026-09-18T12:00:00Z");
    }
    child.shutdown_and_wait().await?;

    // Persistent boards survive a parent restart; the in-memory runner owns its lifetime.
    let resumed = if in_memory {
        root
    } else {
        test_codex()
            .with_config(configure_board)
            .restart_with_auto_env(&server, &root)
            .await?
    };
    // A read must not initialize the schema or acquire the database writer lock.
    let writer = if in_memory {
        None
    } else {
        let pool = sqlite
            .open_read_write_pool(&sqlite.home().join("agent_message_board_1.sqlite"))
            .await?;
        Some(pool.begin_with("BEGIN IMMEDIATE").await?)
    };
    let read = responses::mount_sse_sequence(
        &server,
        board_turn(
            "root-read",
            "search_posts",
            json!({"channel_name":"design"}),
        ),
    )
    .await;
    resumed
        .submit_turn("Find the board tools and read the worker's decision.")
        .await?;
    if let Some(writer) = writer {
        writer.rollback().await?;
    }
    let result = board_result(
        read.requests().last().context("parent follow-up")?,
        "root-read",
    )?;
    assert_eq!(
        serde_json::from_value::<codex_message_board::Page<codex_message_board::PostPreview>>(
            result
        )?,
        codex_message_board::Page {
            results: vec![codex_message_board::PostPreview {
                metadata: serde_json::from_value(post.clone())?,
                text_preview: "Worker's durable decision.".into(),
                n_chars: "Worker's durable decision.".chars().count(),
                truncated: false,
            }],
            next_cursor: None,
        }
    );
    for request in [&posted.requests()[1], &read.requests()[1]] {
        assert!(
            request
                .tool_search_output("find-board")
                .to_string()
                .contains("mcp__codex_message_board")
        );
        assert!(request.tool_by_name("collaboration", "post").is_none());
    }
    if !in_memory {
        let timestamp = post["created_at"].as_str().context("post timestamp")?;
        let mut bodies = posted
            .requests()
            .iter()
            .chain(read.requests().iter())
            .map(ResponsesRequest::body_json)
            .collect::<Vec<_>>();
        for body in &mut bodies {
            for item in body["input"].as_array_mut().context("request input")? {
                if item["type"] == "function_call_output"
                    && let Some(output) = item["output"].as_str()
                {
                    item["output"] = Value::String(
                        context_snapshot::normalize_json_lines(output)
                            .replace(timestamp, "<TIMESTAMP>"),
                    );
                }
            }
        }
        insta::assert_snapshot!(
            if plugin {
                "agent_message_board_plugin_context"
            } else {
                "agent_message_board_mcp_context"
            },
            context_snapshot::format_context_snapshot(
                "A child posts through the HTTP MCP executable; its restarted parent discovers the same board and reads the decision.",
                &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    // The endpoint runs on the app host, including when the executor is remote.
    // Exercise authentication and membership through a host-local HTTP client.
    use codex_rmcp_client::RmcpClient;
    let url = endpoint["url"].as_str().context("endpoint URL")?;
    let valid_token = endpoint["http_headers"]["Authorization"]
        .as_str()
        .context("authorization")?
        .strip_prefix("Bearer ")
        .context("bearer token")?;
    for (token, host) in [
        ("wrong-token", host),
        (valid_token, "unlisted.example"),
        (valid_token, "127.0.0.1"),
    ] {
        let client = RmcpClient::new_streamable_http_client(
            "board-auth-test",
            url,
            Some(token.to_owned()),
            Some([("Host".into(), host.into())].into()),
            /*env_http_headers*/ None,
            codex_config::types::OAuthCredentialsStoreMode::File,
            Default::default(),
            codex_exec_server::Environment::default_for_tests().get_http_client(),
            /*auth_provider*/ None,
        )
        .await?;
        let initialized = client
            .initialize(
                Default::default(),
                /*timeout*/ None,
                Box::new(|_, _| Box::pin(async { anyhow::bail!("unexpected elicitation") })),
            )
            .await;
        if token == "wrong-token" || host == "unlisted.example" {
            assert!(initialized.is_err());
            continue;
        }
        initialized?;
        for metadata in [
            None,
            Some(json!({"threadId":root_id, "sessionId":child_id})),
        ] {
            let error = client
                .call_tool(
                    "get_channels".into(),
                    /*arguments*/ None,
                    |_| Ok(metadata.clone()),
                    /*timeout*/ None,
                )
                .await
                .expect_err("invalid identity must fail");
            assert_eq!(
                codex_rmcp_client::mcp_error(&error).map(|error| error.code),
                Some(rmcp::model::ErrorCode::INVALID_PARAMS)
            );
        }
        client.shutdown().await;
    }
    resumed.codex.shutdown_and_wait().await?;
    if let Some(local) = local {
        assert!(
            !resumed
                .config
                .sqlite_config()
                .home()
                .join("agent_message_board_1.sqlite")
                .exists()
        );
        local.shutdown().await?;
    }
    if let Some(mut process) = process {
        process.kill().await?;
        process.wait().await?;
    }
    Ok(())
}
