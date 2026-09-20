//! Consumer HTTP contract, descendant discovery, and partial-result regression coverage.

use super::super::client::tests::live;
use super::*;
use pretty_assertions::assert_eq;
use serde_json::json;
use wiremock::Mock;
use wiremock::MockServer;
use wiremock::ResponseTemplate;
use wiremock::matchers::method;
use wiremock::matchers::path;

#[tokio::test]
async fn consumer_roots_include_paginated_archived_descendants_and_keep_missing_usage() {
    for (unsupported, status, switch_account) in [
        (false, 200, false),
        (true, 200, false),
        (false, 404, false),
        (false, 503, false),
        (false, 200, true),
    ] {
        use codex_app_server_client::RemoteAppServerClient;
        use codex_app_server_client::RemoteAppServerConnectArgs;
        use codex_app_server_client::RemoteAppServerEndpoint;
        use futures::SinkExt;
        use tokio_tungstenite::tungstenite::Message;

        let server = MockServer::start().await;
        let (home, live) = live(&server, "plus").await;
        let home_path = home.path().to_path_buf();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let websocket_url = format!("ws://{}", listener.local_addr().unwrap());
        let rpc = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut socket = tokio_tungstenite::accept_async(stream).await.unwrap();
            let mut pages = 0;
            while let Some(Ok(Message::Text(text))) = socket.next().await {
                let request: serde_json::Value = serde_json::from_str(&text).unwrap();
                let result = match request["method"].as_str() {
                    Some("initialize") => {
                        json!({"userAgent":"analytics-test", "codexHome":"/unused"})
                    }
                    Some("thread/list") => {
                        pages += 1;
                        if pages == 1 {
                            tokio::time::pause();
                            tokio::time::advance(std::time::Duration::from_secs(/*secs*/ 31)).await;
                            tokio::time::resume();
                        }
                        let params = &request["params"];
                        assert_eq!(params["modelProviders"], json!([]));
                        assert_ne!(params["useStateDbOnly"], json!(true));
                        let ancestor = params["ancestorThreadId"].as_str();
                        let archived = params["archived"] == true;
                        let cursor = params["cursor"].as_str();
                        let ids = match (ancestor, archived, cursor) {
                            (None, false, None) => vec!["root", "missing"],
                            (Some("root"), false, None) if unsupported => vec!["unrelated"],
                            (Some("root"), false, None) => vec!["child"],
                            (Some("root"), false, Some("next")) => vec!["grandchild"],
                            (Some("root"), true, None) => vec!["archived-child"],
                            (Some("missing"), _, None) => Vec::new(),
                            _ => panic!("Unexpected thread listing: {params}"),
                        };
                        let data = ids.into_iter().map(|id| json!({
                        "id":id, "sessionId":id, "parentThreadId":if id == "unrelated" { None } else if id == "grandchild" { Some("child") } else { ancestor }, "preview":id, "ephemeral":false,
                        "modelProvider":"openai", "createdAt":1788220800_i64,
                        "updatedAt":chrono::Utc::now().timestamp(), "status":{"type":"idle"},
                        "cwd":std::env::temp_dir(), "cliVersion":"test", "source":"cli", "turns":[]
                    })).collect::<Vec<_>>();
                        json!({"data":data, "nextCursor":if ancestor == Some("root") && !archived && cursor.is_none() { Some("next") } else { None }})
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
                if pages == if unsupported { 4 } else { 6 } {
                    break;
                }
            }
            pages
        });
        let remote = RemoteAppServerClient::connect(RemoteAppServerConnectArgs {
            endpoint: RemoteAppServerEndpoint::WebSocket {
                websocket_url,
                auth_token: None,
            },
            client_name: "analytics-test".into(),
            client_version: "test".into(),
            experimental_api: false,
            mcp_server_openai_form_elicitation: false,
            opt_out_notification_methods: Vec::new(),
            channel_capacity: 16,
        })
        .await
        .unwrap();
        Mock::given(method("POST")).and(path("/backend-api/wham/usage/thread_usage/query_v2"))
        .respond_with(move |request:&wiremock::Request| {
            let mut body:serde_json::Value = request.body_json().unwrap();
            body["threads"].as_array_mut().unwrap().sort_by_key(|thread| thread["thread_id"].as_str().unwrap().to_string());
            let mut expected=json!({"threads":[
                {"thread_id":"missing", "created_at":"2026-09-01T00:00:00+00:00", "descendant_thread_ids":[]},
                {"thread_id":"root", "created_at":"2026-09-01T00:00:00+00:00", "descendant_thread_ids":["archived-child", "child", "grandchild"]}
            ]});
            if unsupported { expected["threads"].as_array_mut().unwrap().retain(|thread| thread["thread_id"] == "missing"); }
            assert_eq!(body, expected);
            if switch_account {
                super::super::client::tests::sign_in(&home_path, "account-b", "user-b", "plus");
            }
            let mut response=json!({"data_as_of":null,"threads":[]});
            if !unsupported { response["threads"] = json!([{ "thread_id":"root", "data_status":"partial", "usage_source":"included_plan", "weekly_limit_percent":125, "five_hour_limit_percent":null, "balance_usage_credits":null, "groups":[] }]); }
            ResponseTemplate::new(status).set_body_json(response)
        }).expect(/*r*/ 1).mount(&server).await;
        let handle = AppServerRequestHandle::Remote(remote.request_handle());
        let live = Arc::new(live);
        let result = read(handle, Arc::clone(&live)).await;
        assert_eq!(rpc.await.unwrap(), if unsupported { 4 } else { 6 });
        if switch_account {
            assert_eq!(
                (
                    result.is_err(),
                    live.identity_invalidated
                        .load(std::sync::atomic::Ordering::Relaxed),
                ),
                (true, true)
            );
            continue;
        }
        let chats = result.unwrap().unwrap();
        assert_eq!(
            (
                chats.rows.len(),
                chats
                    .rows
                    .iter()
                    .filter_map(|chat| chat.task.as_ref().map(|task| task.data_status))
                    .collect::<Vec<_>>()
            ),
            (
                2,
                if unsupported || status != 200 {
                    vec![]
                } else {
                    vec![codex_backend_client::TaskUsageStatus::Partial]
                }
            )
        );
    }
}
