//! Exercises the MCP adapter against the existing board and host fixtures.

use super::*;
use codex_message_board_mcp::MessageBoardMcpServer;
use codex_tools::ToolSpec;
use pretty_assertions::assert_eq;
use rmcp::ServerHandler;
use rmcp::ServiceExt;
use rmcp::model::CallToolRequestParams;
use rmcp::model::CallToolResponse;
use rmcp::model::CallToolResult;
use rmcp::model::RequestId;
use rmcp::model::Tool;
use rmcp::service::RequestContext;
use serde_json::json;

#[tokio::test]
async fn discovery_and_calls_reuse_board_schemas_identity_and_bounded_results() {
    for backend in [Backend::Local, Backend::InMemory] {
        let dir = tempfile::tempdir().unwrap();
        let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
        let caller = ThreadId::new();
        let path = AgentPath::root().join("worker").unwrap();
        let host = Arc::new(Host {
            clock: AtomicI64::default(),
            fail_clock: AtomicBool::new(false),
            agent_path_calls: AtomicUsize::default(),
            members: [(caller, path.clone())].into(),
            active: AtomicBool::new(true),
            fail_notifications: AtomicBool::new(false),
            notifications: Mutex::default(),
        });
        let board = backend.open(&sqlite, caller.into(), host).await;
        let native = message_board_tools(
            &MessageBoardTool::DISCUSSION,
            board.clone(),
            caller,
            path.clone(),
            /*namespace*/ None,
            "",
            /*tool_messages*/ None,
        );
        let adapter = MessageBoardMcpServer::new(
            &MessageBoardTool::DISCUSSION,
            board.clone(),
            caller,
            path.clone(),
        );
        let (server_io, client_io) = tokio::io::duplex(/*max_buf_size*/ 65536);
        let (server, client) = tokio::join!(adapter.serve(server_io), ().serve(client_io));
        let server = server.unwrap();
        let mut client = client.unwrap();
        let expected = native
            .iter()
            .map(|executor| {
                let ToolSpec::Function(spec) = executor.spec() else {
                    panic!("expected function")
                };
                Tool::new(
                    spec.name,
                    spec.description,
                    Arc::new(
                        serde_json::from_value(serde_json::to_value(spec.parameters).unwrap())
                            .unwrap(),
                    ),
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(client.list_all_tools().await.unwrap(), expected);

        let text = "é\"".repeat(6_000);
        let posted = client
            .call_tool(
                CallToolRequestParams::new("post").with_arguments(
                    json!({"new_channel_name":"work", "text":text})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(posted.is_error, Some(false));
        let metadata: PostMetadata =
            serde_json::from_value(posted.structured_content.unwrap()).unwrap();
        assert_eq!(metadata.author, path);
        let arguments = json!({"message_id":metadata.message_id});
        let read = client
            .call_tool(
                CallToolRequestParams::new("read_post")
                    .with_arguments(arguments.as_object().unwrap().clone()),
            )
            .await
            .unwrap();
        let payload = board_tool_call("read_post", arguments);
        let native_read = native
            .iter()
            .find(|executor| executor.tool_name().name == "read_post")
            .unwrap()
            .handle(payload.clone())
            .await
            .unwrap();
        let mut expected =
            CallToolResult::structured(native_read.code_mode_result(&payload.payload));
        // The negotiated legacy wire format omits the newer resultType discriminator.
        expected.result_type = None;
        assert_eq!(read, expected);
        let value = read.structured_content.unwrap();
        assert!(value.to_string().len() <= 8_000);
        let content: PostContent = serde_json::from_value(value).unwrap();
        assert!(content.next_offset_chars < content.n_chars);
        assert_eq!(
            content.text,
            text.chars()
                .take(content.next_offset_chars)
                .collect::<String>()
        );

        // Identity is bound by the host, never supplied in model arguments.
        let invalid = client
            .call_tool(
                CallToolRequestParams::new("post").with_arguments(
                    json!({"new_channel_name":"spoofed", "text":"bad", "caller":ThreadId::new()})
                        .as_object()
                        .unwrap()
                        .clone(),
                ),
            )
            .await
            .unwrap();
        assert_eq!(invalid.is_error, Some(true));
        let channels = board
            .list_channels(
                caller,
                ChannelQuery {
                    query: Some("spoofed".into()),
                    direction: SortDirection::NewestFirst,
                    page: PageRequest::default(),
                },
            )
            .await
            .unwrap();
        assert_eq!(channels.channels.results, Vec::new());
        assert!(
            client
                .call_tool(CallToolRequestParams::new("unknown"))
                .await
                .is_err()
        );
        client.close().await.unwrap();
        server.waiting().await.unwrap();
    }
}

#[tokio::test]
async fn request_identity_deduplicates_within_a_session_but_not_across_sessions() {
    let caller = ThreadId::new();
    let host = Arc::new(Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(caller, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = Arc::new(InMemoryAgentMessageBoard::new(caller.into(), host));
    board
        .create_channel(
            caller,
            CreateChannelRequest {
                channel_name: "general".into(),
                description: None,
                subscription: SubscriptionChange::Unsubscribe,
            },
        )
        .await
        .unwrap();
    let mut posts = Vec::new();
    for _ in 0..2 {
        let adapter = MessageBoardMcpServer::new(
            &[MessageBoardTool::Post],
            board.clone(),
            caller,
            AgentPath::root(),
        );
        let (server_io, client_io) = tokio::io::duplex(/*max_buf_size*/ 65536);
        let (server, client) = tokio::join!(adapter.serve(server_io), ().serve(client_io));
        let server = server.unwrap();
        let mut client = client.unwrap();
        // Use the handler's real session with explicit IDs to exercise retransmission
        // and distinguish the JSON-RPC number 7 from the string "7".
        for id in [RequestId::Number(7), RequestId::String("7".into())] {
            let request = CallToolRequestParams::new("post").with_arguments(
                json!({"channel_name":"general", "text":"same request"})
                    .as_object()
                    .unwrap()
                    .clone(),
            );
            let context = RequestContext::new(id, server.peer().clone());
            let first = server
                .service()
                .call_tool(request.clone(), context.clone())
                .await
                .unwrap();
            let retry = server.service().call_tool(request, context).await.unwrap();
            let CallToolResponse::Complete(first) = first else {
                panic!("expected completed call")
            };
            let CallToolResponse::Complete(retry) = retry else {
                panic!("expected completed retry")
            };
            assert_eq!(first.is_error, Some(false));
            assert_eq!(first, retry);
            posts.push(first);
        }
        client.close().await.unwrap();
        server.waiting().await.unwrap();
    }
    for (index, post) in posts.iter().enumerate() {
        assert!(posts[..index].iter().all(|previous| previous != post));
    }
    let stored = board
        .search_posts(
            caller,
            PostQuery {
                channel_name: Some("general".into()),
                query: Some("same request".into()),
                after_message_id: None,
                author: None,
                max_chars_per_post: NonZeroU32::new(100).unwrap(),
                page: PageRequest::default(),
            },
        )
        .await
        .unwrap();
    assert_eq!(stored.results.len(), 4);
}

#[tokio::test]
async fn read_only_connection_returns_the_existing_structured_permission_denial() {
    let dir = tempfile::tempdir().unwrap();
    let sqlite = SqliteConfig::new_for_testing(dir.path().to_path_buf().try_into().unwrap());
    let caller = ThreadId::new();
    let host = Arc::new(Host {
        clock: AtomicI64::default(),
        fail_clock: AtomicBool::new(false),
        agent_path_calls: AtomicUsize::default(),
        members: [(caller, AgentPath::root())].into(),
        active: AtomicBool::new(true),
        fail_notifications: AtomicBool::new(false),
        notifications: Mutex::default(),
    });
    let board = Arc::new(
        LocalBoard::new(sqlite, caller.into())
            .read_only()
            .open(host)
            .await
            .unwrap(),
    );
    let adapter = MessageBoardMcpServer::new(
        &MessageBoardTool::DISCUSSION,
        board,
        caller,
        AgentPath::root(),
    );
    let (server_io, client_io) = tokio::io::duplex(/*max_buf_size*/ 65536);
    let (server, client) = tokio::join!(adapter.serve(server_io), ().serve(client_io));
    let server = server.unwrap();
    let mut client = client.unwrap();
    let result = client
        .call_tool(
            CallToolRequestParams::new("post").with_arguments(
                json!({"new_channel_name":"blocked", "text":"bad"})
                    .as_object()
                    .unwrap()
                    .clone(),
            ),
        )
        .await
        .unwrap();
    let mut expected = CallToolResult::structured_error(json!({"error": {
        "code":"permission_denied", "message":"This board connection is read-only.",
        "action":"channel.post", "resource":format!("board:{}", SessionId::from(caller)),
    }}));
    expected.result_type = None;
    assert_eq!(result, expected);
    client.close().await.unwrap();
    server.waiting().await.unwrap();
}
