use super::*;
use crate::AppServerClient;
use crate::AppServerEvent;
use codex_app_server_protocol::ThreadReadParams;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn retained_request_and_reply_stay_with_their_cloud_backend() {
    let (first, mut first_commands, first_events) = CloudClient::channel(/*capacity*/ 1);
    let (second, mut second_commands, _second_events) = CloudClient::channel(/*capacity*/ 1);
    let mut first = AppServerClient::Cloud(first);
    let second = AppServerClient::Cloud(second);
    let handle = first.request_handle();
    let reply = handle.server_request_reply_handle(RequestId::Integer(7));
    let worker = tokio::spawn(async move {
        let command = first_commands.recv().await.unwrap();
        let CloudOperation::Request(request) = command.operation else {
            panic!("expected thread read");
        };
        let ClientRequest::ThreadRead { params, .. } = *request else {
            panic!("expected thread read");
        };
        assert_eq!(params.thread_id, "thread-1");
        first_events
            .send(AppServerEvent::Disconnected {
                message: "stream ended".into(),
            })
            .unwrap();
        command
            .response
            .send(Ok(Ok(serde_json::json!({"owner": "first"}))))
            .unwrap();
        let command = first_commands.recv().await.unwrap();
        let CloudOperation::Resolve { request_id, result } = command.operation else {
            panic!("expected retained reply");
        };
        assert_eq!(
            (request_id, result),
            (
                RequestId::Integer(7),
                serde_json::json!({"decision": "accept"})
            )
        );
        command
            .response
            .send(Ok(Ok(serde_json::Value::Null)))
            .unwrap();
    });
    assert_eq!(
        handle
            .request(ClientRequest::ThreadRead {
                request_id: RequestId::Integer(1),
                params: ThreadReadParams {
                    thread_id: "thread-1".into(),
                    include_turns: false
                },
            })
            .await
            .unwrap()
            .unwrap(),
        serde_json::json!({"owner": "first"})
    );
    reply
        .resolve(serde_json::json!({"decision": "accept"}))
        .await
        .unwrap();
    assert!(second_commands.try_recv().is_err());
    assert!(matches!(
        first.next_event().await,
        Some(AppServerEvent::Disconnected { .. })
    ));
    worker.await.unwrap();
    assert!(
        handle
            .request(ClientRequest::ThreadRead {
                request_id: RequestId::Integer(2),
                params: ThreadReadParams {
                    thread_id: "thread-1".into(),
                    include_turns: false
                },
            })
            .await
            .is_err()
    );
    drop(second);
}

#[tokio::test(start_paused = true)]
async fn cloud_shutdown_times_out_when_owner_does_not_respond() {
    let (client, mut commands, events) = CloudClient::channel(/*capacity*/ 1);
    let error = tokio::time::timeout(
        crate::SHUTDOWN_TIMEOUT + std::time::Duration::from_secs(1),
        AppServerClient::Cloud(client).shutdown(),
    )
    .await
    .expect("shutdown should be bounded")
    .expect_err("an unresponsive cloud worker should time out");
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let command = commands.try_recv().expect("shutdown was admitted");
    assert!(matches!(command.operation, CloudOperation::Shutdown));
    assert!(command.response.is_closed());
    assert!(events.is_closed());
    assert!(commands.recv().await.is_none());
}

#[tokio::test]
async fn native_model_list_bypasses_the_cloud_command_queue() -> color_eyre::Result<()> {
    use codex_app_server_protocol::ModelListResponse;
    use wiremock::Mock;
    use wiremock::MockServer;
    use wiremock::ResponseTemplate;
    use wiremock::matchers::header;
    use wiremock::matchers::method;
    use wiremock::matchers::path;
    use wiremock::matchers::query_param;

    let server = MockServer::start().await;
    let home = tempfile::tempdir()?;
    let config = crate::legacy_core::config::ConfigBuilder::default()
        .codex_home(home.path().to_owned())
        .fallback_cwd(Some(home.path().to_owned()))
        .without_project_context()
        .build()
        .await?;
    let native = codex_cloud_client::Client::new(
        &config.http_client_factory(),
        &server.uri(),
        codex_cloud_client::Credentials::new("synthetic-bearer", "synthetic-account")?,
    )?;
    let response = serde_json::json!({
        "data": [{
            "id": "test-model",
            "model": "test-model",
            "displayName": "Test model",
            "description": "Cloud catalog fixture",
            "hidden": true,
            "supportedReasoningEfforts": [],
            "defaultReasoningEffort": "medium",
            "isDefault": false
        }],
        "nextCursor": "next"
    });
    Mock::given(method("GET"))
        .and(path("/v2/models"))
        .and(header("authorization", "Bearer synthetic-bearer"))
        .and(header("chatgpt-account-id", "synthetic-account"))
        .and(query_param("cursor", "previous"))
        .and(query_param("limit", "2"))
        .and(query_param("includeHidden", "true"))
        .respond_with(ResponseTemplate::new(200).set_body_json(&response))
        .expect(1)
        .mount(&server)
        .await;

    // Leave the command receiver idle. The catalog must finish without its worker.
    let (client, mut commands, _events) =
        CloudClient::channel_with_catalog(/*capacity*/ 1, Some(native));
    let handle = AppServerClient::Cloud(client).request_handle();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(/*secs*/ 5),
        handle.request_typed::<ModelListResponse>(ClientRequest::ModelList {
            request_id: RequestId::Integer(1),
            params: ModelListParams {
                cursor: Some("previous".into()),
                limit: Some(2),
                include_hidden: Some(true),
                model_provider: None,
            },
        }),
    )
    .await??;
    assert_eq!(
        result,
        serde_json::from_value::<ModelListResponse>(response)?
    );
    assert!(matches!(
        commands.try_recv(),
        Err(mpsc::error::TryRecvError::Empty)
    ));
    Ok(())
}
