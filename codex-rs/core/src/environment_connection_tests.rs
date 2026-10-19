//! Exercise lifecycle forwarding against real recovering exec-server transports.

use std::sync::Arc;
use std::time::Duration;

use codex_exec_server::Environment;
use codex_exec_server::EnvironmentConnectionState;
use codex_protocol::protocol::EventMsg;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio::sync::oneshot;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;
use tokio_util::task::AbortOnDropHandle;

use super::ThreadEnvironments;
use super::tests::read_websocket_json;

// Keep paused time from advancing during socket I/O; only the test advances it.
fn freeze_clock() -> std::sync::mpsc::Sender<()> {
    tokio::time::pause();
    let (guard, dropped) = std::sync::mpsc::channel();
    tokio::task::spawn_blocking(move || {
        let _ = dropped.recv();
    });
    guard
}

async fn initialize(
    socket: &mut WebSocketStream<TcpStream>,
    resume_session_id: Option<&str>,
    result: Result<(), i64>,
) {
    let request = read_websocket_json(socket).await;
    assert_eq!(request["method"], "initialize");
    assert_eq!(
        request["params"]["resumeSessionId"],
        json!(resume_session_id)
    );
    let response = match result {
        Ok(()) => json!({"id": request["id"], "result": {"sessionId": "session"}}),
        Err(code) => json!({"id": request["id"], "error": {
            "code": code, "message": "cannot resume"
        }}),
    };
    socket
        .send(Message::Text(response.to_string().into()))
        .await
        .unwrap();
    if result.is_ok() {
        assert_eq!(read_websocket_json(socket).await["method"], "initialized");
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Recovery {
    Succeeds,
    Rejected,
    TimesOut,
}

#[test_case::test_case(0, Recovery::Succeeds; "fast")]
#[test_case::test_case(3, Recovery::Succeeds; "near_deadline")]
#[test_case::test_case(0, Recovery::TimesOut; "retry_exhaustion")]
#[test_case::test_case(0, Recovery::Rejected; "nonretryable_failure")]
#[tokio::test]
async fn lifecycle_waits_for_recovery(retries: usize, recovery: Recovery) {
    let _clock = freeze_clock();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let environment = Arc::new(
        Environment::create_for_tests(Some(format!("ws://{}", listener.local_addr().unwrap())))
            .unwrap(),
    );
    let (events, received) = async_channel::unbounded();
    let _forwarding = ThreadEnvironments::spawn_connection_event_listener(
        &environment,
        "remote".to_owned(),
        events,
    )
    .unwrap();
    let mut raw = environment.subscribe_connection_state().unwrap();
    let (disconnect, disconnected) = oneshot::channel();
    let (resume, resumed) = oneshot::channel();
    let (ready, ready_rx) = oneshot::channel();
    let (retrying, mut retried) = mpsc::unbounded_channel();
    let (response_sent, response_rx) = oneshot::channel();
    let (finish, finished) = oneshot::channel();
    let server = AbortOnDropHandle::new(tokio::spawn(async move {
        let mut socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        initialize(&mut socket, /*resume_session_id*/ None, Ok(())).await;
        disconnected.await.unwrap();
        socket.close(None).await.unwrap();
        for _ in 0..retries {
            let mut socket = accept_async(listener.accept().await.unwrap().0)
                .await
                .unwrap();
            initialize(&mut socket, Some("session"), Err(-32010)).await;
            // Wait for the client to process the rejection before advancing time.
            while let Some(Ok(message)) = socket.next().await {
                if matches!(message, Message::Close(_)) {
                    break;
                }
            }
            retrying.send(()).unwrap();
        }
        let mut socket = accept_async(listener.accept().await.unwrap().0)
            .await
            .unwrap();
        ready.send(()).unwrap();
        resumed.await.unwrap();
        let result = match recovery {
            Recovery::Succeeds => Ok(()),
            Recovery::Rejected => Err(-32000),
            Recovery::TimesOut => Err(-32010),
        };
        initialize(&mut socket, Some("session"), result).await;
        let _ = response_sent.send(());
        finished.await.unwrap();
    }));
    environment.wait_until_ready().await.unwrap();
    assert!(matches!(
        received.recv().await.unwrap().msg,
        EventMsg::EnvironmentConnected(_)
    ));
    raw.borrow_and_update();
    let disconnected_at = tokio::time::Instant::now();
    disconnect.send(()).unwrap();
    for _ in 0..retries {
        retried.recv().await.unwrap();
        assert_eq!(
            *raw.borrow_and_update(),
            EnvironmentConnectionState::Recovering
        );
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(8)).await;
    }
    ready_rx.await.unwrap();
    assert_eq!(
        disconnected_at.elapsed(),
        Duration::from_secs(8 * retries as u64)
    );
    raw.wait_for(|state| *state == EnvironmentConnectionState::Recovering)
        .await
        .unwrap();
    assert_eq!(
        *raw.borrow_and_update(),
        EnvironmentConnectionState::Recovering
    );
    assert!(received.try_recv().is_err());
    resume.send(()).unwrap();
    if recovery == Recovery::TimesOut {
        response_rx.await.unwrap();
        assert!(received.try_recv().is_err());
        tokio::time::advance(Duration::from_secs(26)).await;
    }
    let event = received.recv().await.unwrap().msg;
    match recovery {
        Recovery::Succeeds => {
            assert!(matches!(event, EventMsg::EnvironmentConnected(_)));
        }
        Recovery::Rejected | Recovery::TimesOut => {
            assert!(matches!(event, EventMsg::EnvironmentDisconnected(_)));
            assert_eq!(
                *raw.borrow_and_update(),
                EnvironmentConnectionState::Disconnected
            );
        }
    }
    assert!(received.try_recv().is_err());
    finish.send(()).unwrap();
    server.await.unwrap();
}
