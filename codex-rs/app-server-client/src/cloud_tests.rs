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
