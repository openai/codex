use super::*;
use crate::mention_codec::LinkedMention;
use crate::mention_codec::decode_history_mentions_with_at_mentions;
use crate::mention_codec::encode_history_mentions_at_elements;
use codex_protocol::user_input::TextElement;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio_tungstenite::tungstenite::Message;

#[tokio::test]
async fn remote_task_search_cancels_without_delaying_results() {
    for timeout in [false, true] {
        check_remote_task_search_cancellation(timeout).await;
    }
}

async fn check_remote_task_search_cancellation(timeout: bool) {
    use codex_app_server_client::RemoteAppServerClient;
    use codex_app_server_client::RemoteAppServerConnectArgs;
    use codex_app_server_client::RemoteAppServerEndpoint;

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let websocket_url = format!("ws://{}", listener.local_addr().unwrap());
    let (search_started, started) = tokio::sync::oneshot::channel();
    let (cancel_sent, cancelled) = tokio::sync::oneshot::channel();
    let rpc = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
        let mut search_started = Some(search_started);
        let mut search_id = None;
        while let Some(Ok(Message::Text(text))) = socket.next().await {
            let request: serde_json::Value = serde_json::from_str(&text).unwrap();
            let result = match request["method"].as_str() {
                Some("initialize") => {
                    json!({"userAgent":"task-search-test", "codexHome":"/unused"})
                }
                Some("thread/list") => json!({
                    "data": [{
                        "id": "task-123", "sessionId": "task-123", "preview": "needle",
                        "name": "needle title", "ephemeral": false, "modelProvider": "openai",
                        "createdAt": 0, "updatedAt": 0, "status": {"type": "notLoaded"},
                        "cwd": std::env::temp_dir(), "cliVersion": "test", "source": "cli",
                        "turns": []
                    }],
                    "nextCursor": null
                }),
                Some("thread/search") => {
                    search_id = Some(request["id"].clone());
                    search_started.take().unwrap().send(()).unwrap();
                    continue;
                }
                Some("thread/search/cancel") => {
                    assert_eq!(Some(&request["params"]["requestId"]), search_id.as_ref());
                    cancel_sent.send(()).unwrap();
                    // Leave cancellation unanswered to verify it cannot delay title results.
                    std::future::pending::<()>().await;
                    return;
                }
                _ => continue,
            };
            socket
                .send(Message::Text(
                    json!({"jsonrpc":"2.0", "id":request["id"], "result":result})
                        .to_string()
                        .into(),
                ))
                .await
                .unwrap();
        }
        panic!("remote disconnected before cancelling the search");
    });
    let remote = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
        required_human_approval: false,
        endpoint: RemoteAppServerEndpoint::WebSocket {
            websocket_url,
            auth_token: None,
        },
        client_name: "task-search-test".into(),
        client_version: "test".into(),
        experimental_api: true,
        mcp_server_openai_form_elicitation: false,
        opt_out_notification_methods: Vec::new(),
        channel_capacity: 8,
    })
    .await
    .unwrap();
    let handle = AppServerRequestHandle::Remote(remote.request_handle());
    let (tx, mut events) = tokio::sync::mpsc::unbounded_channel();
    let current = ThreadId::from_u128(/*value*/ 1);
    let mut search = TaskSearch::default();
    search.spawn(
        handle.clone(),
        "needle".into(),
        current,
        PathBuf::from("/tmp"),
        AppEventSender::new(tx.clone()),
    );
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), started)
        .await
        .unwrap()
        .unwrap();
    if timeout {
        tokio::time::pause();
        let deadline = tokio::time::Instant::now() + SEARCH_TIMEOUT;
        tokio::time::advance(SEARCH_TIMEOUT).await;
        let event = events.recv().await.unwrap();
        assert!(
            matches!(event, AppEvent::TaskSearchResult { query, matches, .. }
                if query == "needle" && matches.len() == 1 && matches[0].title == "needle title")
        );
        assert!(tokio::time::Instant::now() < deadline + CANCEL_TIMEOUT / 2);
        tokio::time::resume();
    } else {
        search.spawn(
            handle,
            String::new(),
            current,
            PathBuf::from("/tmp"),
            AppEventSender::new(tx),
        );
        let event = events.recv().await.unwrap();
        assert!(
            matches!(event, AppEvent::TaskSearchResult { query, matches, .. } if query.is_empty() && matches.is_empty())
        );
    }
    tokio::time::timeout(Duration::from_secs(/*secs*/ 5), cancelled)
        .await
        .unwrap()
        .unwrap();
    rpc.abort();
    let _ = remote.shutdown().await;
}

#[test]
fn task_mention_history_round_trips_multiword_and_escaped_titles() {
    let title = "Fix ]( parser \\ safely";
    let text = format!("Inspect x@{title} next");
    let mention = LinkedMention {
        sigil: '@',
        mention: title.to_string(),
        path: "thread://task-123".to_string(),
    };

    let encoded = encode_history_mentions_at_elements(
        &text,
        std::slice::from_ref(&mention),
        &[TextElement::new(
            ("Inspect x".len().."Inspect x@".len() + title.len()).into(),
            /*placeholder*/ None,
        )],
    );
    assert_eq!(
        encoded,
        "Inspect x[@Fix \\]\\( parser \\\\ safely](thread://task-123) next"
    );
    let decoded =
        decode_history_mentions_with_at_mentions(&encoded, /*at_mentions_enabled*/ true);
    assert_eq!(decoded.text, text);
    assert_eq!(decoded.mentions, vec![mention]);
    assert!(
        parse_task_link(
            &format!(
                "[@{}](thread://task-123)",
                "x".repeat(MAX_TASK_TITLE_CHARS + 1)
            ),
            /*start*/ 0,
        )
        .is_none()
    );
    for path in ["thread://", "thread://../settings", "thread://task?target"] {
        assert_eq!(valid_thread_path(path), None);
    }
    assert!(valid_thread_path(&format!("thread://{}", "x".repeat(65))).is_none());
    let malformed = "[@é](thread://task-123)";
    for end in [3, malformed.len() + 1] {
        assert_eq!(
            decode_task_links(
                malformed,
                vec![TextElement::new((0..end).into(), /*placeholder*/ None)],
            ),
            (malformed.to_string(), Vec::new())
        );
    }
    let literal = "[@actual](thread://task-456)";
    assert_eq!(
        decode_task_links(
            &format!("{literal} {literal}"),
            vec![
                TextElement::new((0..literal.len()).into(), /*placeholder*/ None),
                TextElement::new(
                    (literal.len() + 1..literal.len() * 2 + 1).into(),
                    Some("@actual".to_string()),
                ),
            ],
        )
        .0,
        format!("{literal} @actual")
    );
}

#[test]
fn task_reference_context_deduplicates_and_merges_with_ide_context() {
    let title = "Review the migration";
    let visible = format!("Compare plain @{title} with plugin @{title} and selected @{title}x");
    let binding = MentionBinding {
        sigil: '@',
        mention: title.to_string(),
        path: "thread://task-123".to_string(),
    };
    let text = format!("# IDE: @{title}\n## My request for Codex:\n{visible}");
    let plugin_start = text.find("plugin @").expect("plugin mention") + "plugin ".len();
    let selected_start = text.rfind(&format!("@{title}")).expect("selected task");
    let mut items = vec![UserInput::Text {
        text,
        text_elements: [plugin_start, selected_start]
            .into_iter()
            .map(|start| {
                codex_app_server_protocol::TextElement::new(
                    ByteRange {
                        start,
                        end: start + title.len() + 1,
                    },
                    Some(format!("@{title}")),
                )
            })
            .collect(),
    }];

    apply_task_references(
        &mut items,
        &[
            MentionBinding {
                sigil: '@',
                mention: title.to_string(),
                path: "plugin://sample@test".to_string(),
            },
            binding.clone(),
            binding,
        ],
        /*current_thread_id*/ None,
    );

    let [UserInput::Text { text, .. }] = items.as_slice() else {
        panic!("expected text with deduplicated task references");
    };
    assert!(text.starts_with("# IDE: @Review the migration\n## Referenced chats"));
    assert_eq!(text.matches("## My request for Codex:").count(), 1);
    assert_eq!(text.matches("\"threadId\":\"task-123\"").count(), 1);
    assert!(text.contains("MUST call `read_thread`"));
    assert!(text.ends_with(
        "Compare plain @Review the migration with plugin @Review the migration and selected [@Review the migration](thread://task-123)x"
    ));

    for id_len in [36, 64] {
        let bindings = (0..MAX_REFERENCED_TASKS)
            .map(|index| MentionBinding {
                sigil: '@',
                mention: "task".to_string(),
                path: format!("thread://{index:0id_len$}"),
            })
            .collect::<Vec<_>>();
        let mut items = [UserInput::Text {
            text: String::new(),
            text_elements: Vec::new(),
        }];
        apply_task_references(&mut items, &bindings, /*current_thread_id*/ None);
        let UserInput::Text { text, .. } = &items[0] else {
            unreachable!();
        };
        assert_eq!(
            text.matches("\"threadId\":").count(),
            MAX_REFERENCED_TASKS.min(MAX_REFERENCED_THREAD_ID_BYTES / id_len)
        );
    }
}

#[test]
fn task_reference_heading_inside_selected_title_is_not_a_context_boundary() {
    let title = format!("Review {REQUEST_HEADING} carefully");
    let mut items = vec![UserInput::Text {
        text: format!("@{title}"),
        text_elements: vec![codex_app_server_protocol::TextElement::new(
            ByteRange {
                start: 0,
                end: title.len() + 1,
            },
            /*placeholder*/ None,
        )],
    }];

    apply_task_references(
        &mut items,
        &[MentionBinding {
            sigil: '@',
            mention: title.clone(),
            path: "thread://task-123".to_string(),
        }],
        /*current_thread_id*/ None,
    );

    let [UserInput::Text { text, .. }] = items.as_slice() else {
        panic!("expected text with a task reference");
    };
    assert!(text.starts_with("## Referenced chats with Codex:"));
    assert!(text.ends_with(&format!("[@{title}](thread://task-123)")));
}
