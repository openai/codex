//! Two real agents share the detached board through ordinary HTTP MCP configuration.

use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;
use tokio::io::AsyncBufReadExt;

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn http_board_is_shared_with_children() -> anyhow::Result<()> {
    let server = responses::start_mock_server().await;
    let home = Arc::new(tempfile::tempdir()?);
    let sqlite = codex_state::SqliteConfig::new_for_testing(home.path().to_path_buf().try_into()?);
    let _state = codex_state::StateRuntime::init(sqlite.clone(), "openai".into()).await?;
    let command = codex_utils_cargo_bin::cargo_bin("codex-message-board-mcp")?;
    let mut process =
        tokio::process::Command::from(codex_utils_process::background_command(command))
            .arg("--sqlite-home")
            .arg(sqlite.home())
            .args(["--listen", "127.0.0.1:0", "--allowed-host", "board.example"])
            .env("CODEX_MESSAGE_BOARD_TOKEN", "http-board-test-token")
            .stdout(std::process::Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
    let ready = tokio::io::BufReader::new(process.stdout.take().context("server stdout")?)
        .lines()
        .next_line()
        .await?
        .context("server readiness")?;
    let ready: Value = serde_json::from_str(&ready)?;
    let endpoint = json!({"url":ready["url"],"http_headers":{"Authorization":"Bearer http-board-test-token","Host":"board.example"},"required":true,"default_tools_approval_mode":"approve"});
    let configured_endpoint = endpoint.clone();
    let configured_sqlite = sqlite.clone();
    let configure_board = move |config: &mut codex_core::config::Config| {
        configure(config);
        config.sqlite = configured_sqlite.clone();
        config
            .features
            .disable(Feature::AgentMessageBoard)
            .expect("disable native board");
        let mut servers = config.mcp_servers.get().clone();
        servers.insert(
            "codex_message_board".into(),
            serde_json::from_value(configured_endpoint.clone()).expect("HTTP config"),
        );
        config.mcp_servers.set(servers).expect("configure endpoint");
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
    child.shutdown_and_wait().await?;

    // A restarted parent discovers and reads the same persisted board.
    let resumed = test_codex()
        .with_config(configure_board)
        .restart_with_auto_env(&server, &root)
        .await?;
    // A read must not initialize the schema or acquire the database writer lock.
    let pool = sqlite
        .open_read_write_pool(&sqlite.home().join("agent_message_board_1.sqlite"))
        .await?;
    let writer = pool.begin_with("BEGIN IMMEDIATE").await?;
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
    writer.rollback().await?;
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
        "agent_message_board_mcp_context",
        context_snapshot::format_context_snapshot(
            "A child posts through the HTTP MCP executable; its restarted parent discovers the same board and reads the decision.",
            &bodies.iter().map(SnapshotEntry::body).collect::<Vec<_>>(),
            &ContextSnapshotOptions::default().rewrite_known_segments(),
        )
    );
    // The endpoint runs on the app host, including when the executor is remote.
    // Exercise authentication and membership through a host-local HTTP client.
    use codex_rmcp_client::RmcpClient;
    let url = endpoint["url"].as_str().context("endpoint URL")?;
    for (token, host) in [
        ("wrong-token", "board.example"),
        ("http-board-test-token", "unlisted.example"),
        ("http-board-test-token", "127.0.0.1"),
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
    process.kill().await?;
    process.wait().await?;
    Ok(())
}
