//! Two real agents share the detached board through ordinary HTTP MCP configuration.

use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;

#[path = "../../../message-board/tests/common/mod.rs"]
mod board_host;

#[derive(Clone, Copy, PartialEq, Eq)]
enum BoardMode {
    Config,
    Plugin,
    InMemory,
    Feature,
}

#[test_case::test_case(BoardMode::Config; "config")]
#[test_case::test_case(BoardMode::Plugin; "plugin")]
#[test_case::test_case(BoardMode::InMemory; "in_memory")]
#[test_case::test_case(BoardMode::Feature; "feature")]
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_board_is_shared_with_children(mode: BoardMode) -> anyhow::Result<()> {
    let plugin = matches!(mode, BoardMode::Plugin | BoardMode::InMemory);
    let in_memory = matches!(mode, BoardMode::InMemory | BoardMode::Feature);
    let server = responses::start_mock_server().await;
    let home = Arc::new(tempfile::tempdir()?);
    let sqlite = codex_state::SqliteConfig::new_for_testing(home.path().to_path_buf().try_into()?);
    let mut process = None;
    let mut local = None;
    let (url, authorization) = if mode == BoardMode::Feature {
        (String::new(), String::new())
    } else if in_memory {
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
    if mode == BoardMode::Feature {
        std::fs::write(
            home.path().join("config.toml"),
            "[features]\nagent_message_board_mcp = true\n",
        )?;
    }
    let configured_endpoint = endpoint.clone();
    let configured_sqlite = sqlite.clone();
    let configure_board = move |config: &mut codex_core::config::Config| {
        configure(config);
        config.sqlite = configured_sqlite.clone();
        config.ephemeral = in_memory;
        if mode == BoardMode::Feature {
            return; // The feature supplies the endpoint and suppresses the native board.
        }
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

    let mut host_state = None;
    if let Some(local) = &local {
        let host = Arc::new(board_host::Host {
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
        });
        host_state = Some(Arc::downgrade(&host));
        local.add_board(root_id.into(), host)?;
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
    let mut post_arguments =
        json!({"new_channel_name":"design","text":"Worker's durable decision."});
    if mode == BoardMode::Feature {
        post_arguments["agents_to_notify"] = json!(["/root"]);
    }
    let posted =
        responses::mount_sse_sequence(&server, board_turn("worker-post", "post", post_arguments))
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
    if mode == BoardMode::InMemory {
        assert_eq!(post["created_at"], "2026-09-18T12:00:00Z");
    }
    if mode != BoardMode::Feature {
        child.shutdown_and_wait().await?;
    }

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
    if mode == BoardMode::Feature {
        assert!(read.requests()[1].body_contains_text("Message Type: CHANNEL_POST"));
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
    if mode != BoardMode::InMemory {
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
            if mode == BoardMode::Feature {
                "agent_message_board_mcp_feature_context"
            } else if plugin {
                "agent_message_board_plugin_context"
            } else {
                "agent_message_board_mcp_context"
            },
            context_snapshot::format_context_snapshot(
                if mode == BoardMode::Feature {
                    "The config flag starts an in-memory MCP board shared by a parent and child."
                } else {
                    "A child posts through the HTTP MCP executable; its restarted parent discovers the same board and reads the decision."
                },
                &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
                &ContextSnapshotOptions::default().rewrite_known_segments(),
            )
        );
    }
    if mode == BoardMode::Feature {
        // The child must keep the shared board usable after its original host unloads.
        resumed.codex.shutdown_and_wait().await?;
        assert!(
            resumed
                .thread_manager
                .remove_thread(&root_id)
                .await
                .is_some()
        );
        for (name, enabled) in [("subscribe", true), ("unsubscribe", false)] {
            let subscribed = responses::mount_sse_sequence(
                &server,
                board_turn(name, name, json!({"channel_name":"design"})),
            )
            .await;
            child
                .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                    text: format!("{name} to the design channel."),
                    text_elements: vec![],
                }]))
                .await?;
            wait_for_event(&child, |event| matches!(event, EventMsg::TurnComplete(_))).await;
            assert_eq!(
                board_result(
                    subscribed
                        .requests()
                        .last()
                        .context("subscription result")?,
                    name
                )?,
                json!({
                    "channel_name":"design", "thread_id":null,
                    "target_agent":"/root/worker", "enabled":enabled,
                    "last_message_id":post["message_id"],
                }),
            );
        }
        // Offer two mutations together and observe the real MCP execution intervals.
        // Native board writes are exclusive even though reads may run concurrently.
        let arguments = json!({
            "channel_name":"design", "text":"The child still owns its host.",
            "agents_to_notify":["/root/worker"],
        });
        let mut mutation_turn = board_turn("survivor-a", "post", arguments.clone());
        mutation_turn[1] = sse(vec![
            ev_function_call_with_namespace(
                "survivor-a",
                "mcp__codex_message_board",
                "post",
                &arguments.to_string(),
            ),
            ev_function_call_with_namespace(
                "survivor-b",
                "mcp__codex_message_board",
                "post",
                &arguments.to_string(),
            ),
            ev_completed("survivor-posts"),
        ]);
        let mutations = responses::mount_sse_sequence(&server, mutation_turn).await;
        child
            .start_or_steer_turn(TurnInputRequest::user_input(vec![UserInput::Text {
                text: "Post twice, notifying yourself.".into(),
                text_elements: vec![],
            }]))
            .await?;
        let mut active_call = None;
        let mut completed_calls = 0;
        wait_for_event(&child, |event| {
            match event {
                EventMsg::McpToolCallBegin(begin) => {
                    assert_eq!(
                        active_call.replace(begin.call_id.clone()),
                        None,
                        "board mutations overlapped"
                    );
                }
                EventMsg::McpToolCallEnd(end) => {
                    assert_eq!(active_call.take(), Some(end.call_id.clone()));
                    completed_calls += 1;
                }
                _ => {}
            }
            matches!(event, EventMsg::TurnComplete(_))
        })
        .await;
        assert_eq!(completed_calls, 2);
        for id in ["survivor-a", "survivor-b"] {
            let result = board_result(
                mutations
                    .requests()
                    .last()
                    .context("surviving child follow-up")?,
                id,
            )?;
            assert_eq!(result["author"], "/root/worker");
        }
        child.shutdown_and_wait().await?;
        assert!(!sqlite.home().join("agent_message_board_1.sqlite").exists());
        return Ok(());
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
    let mut connected_client = None;
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
                rmcp::model::InitializeRequestParams::default()
                    .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_11_25),
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
        connected_client = Some(client);
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
        // The legacy client remains connected: shutdown must release the board
        // without waiting for a client DELETE or the session idle timeout.
        local.shutdown().await?;
        assert!(host_state.context("in-memory host")?.upgrade().is_none());
    }
    if let Some(mut process) = process {
        process.kill().await?;
        process.wait().await?;
    }
    if let Some(client) = connected_client {
        client.shutdown().await;
    }
    Ok(())
}

#[tokio::test]
async fn http_shutdown_releases_pending_binding() -> anyhow::Result<()> {
    use codex_message_board::MessageBoardHost;
    use codex_rmcp_client::RmcpClient;
    use tokio_util::sync::CancellationToken;

    let caller = codex_protocol::ThreadId::new();
    let host = Arc::new(board_host::Host {
        members: [(caller, codex_protocol::AgentPath::root())].into(),
        clock: Default::default(),
        fail_clock: Default::default(),
        agent_path_calls: Default::default(),
        active: Default::default(),
        fail_notifications: Default::default(),
        notifications: Default::default(),
    });
    let host_state = Arc::downgrade(&host);
    let board = Arc::new(codex_message_board::InMemoryAgentMessageBoard::new(
        caller.into(),
        host.clone(),
    ));
    let binding_started = Arc::new(tokio::sync::Notify::new());
    let resume_binding = Arc::new(tokio::sync::Notify::new());
    let started = binding_started.clone();
    let resume = resume_binding.clone();
    let cancellation = CancellationToken::new();
    let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
    let url = format!("http://{}/mcp", listener.local_addr()?);
    let serving = tokio::spawn(codex_message_board_mcp::serve_http(
        listener,
        "shutdown-test-token".into(),
        rmcp::transport::StreamableHttpServerConfig::default()
            .with_cancellation_token(cancellation.clone()),
        move |caller, _session_id| {
            let host = host.clone();
            let board = board.clone();
            let started = started.clone();
            let resume = resume.clone();
            async move {
                // Pause the real board binding before it can dispatch a tool.
                started.notify_one();
                resume.notified().await;
                let path = host.agent_path(caller).await.map_err(|error| {
                    rmcp::ErrorData::invalid_params(error.to_string(), /*data*/ None)
                })?;
                Ok(codex_message_board_mcp::MessageBoardMcpServer::new(
                    &codex_message_board::MessageBoardTool::DISCUSSION,
                    board,
                    caller,
                    path,
                ))
            }
        },
    ));
    let client = RmcpClient::new_streamable_http_client(
        "board-shutdown-test",
        &url,
        Some("shutdown-test-token".into()),
        /*http_headers*/ None,
        /*env_http_headers*/ None,
        codex_config::types::OAuthCredentialsStoreMode::File,
        Default::default(),
        codex_exec_server::Environment::default_for_tests().get_http_client(),
        /*auth_provider*/ None,
    )
    .await?;
    client
        .initialize(
            rmcp::model::InitializeRequestParams::default()
                .with_protocol_version(rmcp::model::ProtocolVersion::V_2025_11_25),
            /*timeout*/ None,
            Box::new(|_, _| Box::pin(async { anyhow::bail!("unexpected elicitation") })),
        )
        .await?;
    {
        let call = client.call_tool(
            "post".into(),
            Some(json!({"new_channel_name":"design", "text":"Must not be dispatched."})),
            |_| Ok(Some(json!({"threadId":caller, "sessionId":caller}))),
            /*timeout*/ None,
        );
        tokio::pin!(call);
        tokio::select! {
            _ = binding_started.notified() => {}
            result = &mut call => panic!("call completed before binding resumed: {result:?}"),
        }
        cancellation.cancel();
        serving.await??;
        // Cleanup must drop the pending binding as well as the idle session's closure.
        // Resuming it after shutdown must never dispatch the post.
        assert!(host_state.upgrade().is_none());
        resume_binding.notify_one();
    }
    client.shutdown().await;
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_shutdown_releases_factory_with_keep_alive_connection() -> anyhow::Result<()> {
    use codex_message_board::MessageBoardHost;
    use tokio::io::AsyncWriteExt;
    use tokio_util::sync::CancellationToken;

    // Exercise the cross-worker race between connection completion and service drop.
    for iteration in 0..256 {
        let caller = codex_protocol::ThreadId::new();
        let host = Arc::new(board_host::Host {
            members: [(caller, codex_protocol::AgentPath::root())].into(),
            clock: Default::default(),
            fail_clock: Default::default(),
            agent_path_calls: Default::default(),
            active: Default::default(),
            fail_notifications: Default::default(),
            notifications: Default::default(),
        });
        let host_state = Arc::downgrade(&host);
        let board = Arc::new(codex_message_board::InMemoryAgentMessageBoard::new(
            caller.into(),
            host.clone(),
        ));
        let listener = tokio::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        let cancellation = CancellationToken::new();
        let shutdown = cancellation.clone();
        let connection = tokio::spawn(async move {
            let _shutdown = shutdown.drop_guard();
            let mut stream = tokio::net::TcpStream::connect(address).await?;
            // An unauthenticated request retains the HTTP service without ever
            // creating an MCP handler. HTTP/1.1 keeps this connection alive.
            stream
                .write_all(b"GET /mcp HTTP/1.1\r\nHost: localhost\r\n\r\n")
                .await?;
            let mut stream = tokio::io::BufReader::new(stream);
            let mut line = String::new();
            stream.read_line(&mut line).await?;
            assert_eq!(line, "HTTP/1.1 401 Unauthorized\r\n");
            while line != "\r\n" {
                line.clear();
                anyhow::ensure!(
                    stream.read_line(&mut line).await? != 0,
                    "incomplete response"
                );
            }
            // The completed task retains the open socket until shutdown returns.
            Ok::<_, anyhow::Error>(stream)
        });
        codex_message_board_mcp::serve_http(
            listener,
            "keep-alive-test-token".into(),
            rmcp::transport::StreamableHttpServerConfig::default()
                .with_cancellation_token(cancellation),
            move |caller, _session_id| {
                let host = host.clone();
                let board = board.clone();
                async move {
                    let path = host.agent_path(caller).await.map_err(|error| {
                        rmcp::ErrorData::invalid_params(error.to_string(), /*data*/ None)
                    })?;
                    Ok(codex_message_board_mcp::MessageBoardMcpServer::new(
                        &codex_message_board::MessageBoardTool::DISCUSSION,
                        board,
                        caller,
                        path,
                    ))
                }
            },
        )
        .await?;
        assert!(
            host_state.upgrade().is_none(),
            "factory retained the board on iteration {iteration}",
        );
        drop(connection.await??);
    }
    Ok(())
}
