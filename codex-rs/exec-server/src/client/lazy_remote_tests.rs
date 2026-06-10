use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;
use std::time::Duration;

use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCResponse;
use futures::FutureExt;
use futures::SinkExt;
use futures::StreamExt;
use pretty_assertions::assert_eq;
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::oneshot;
use tokio::sync::watch;
use tokio::time::timeout;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::Message;

use super::LazyRemoteExecServerClient;
use super::RemoteConnectionSource;
use crate::EnvironmentManager;
use crate::ExecServerClient;
use crate::ExecServerError;
use crate::client_api::ExecServerTransportParams;
use crate::protocol::ENVIRONMENT_INFO_METHOD;
use crate::protocol::EnvironmentInfo;
use crate::protocol::INITIALIZE_METHOD;
use crate::protocol::INITIALIZED_METHOD;
use crate::protocol::InitializeResponse;
use crate::protocol::ShellInfo;

async fn accept_websocket(listener: &TcpListener) -> WebSocketStream<TcpStream> {
    let (stream, _) = listener.accept().await.expect("listener should accept");
    accept_async(stream)
        .await
        .expect("websocket handshake should succeed")
}

async fn read_jsonrpc_websocket(websocket: &mut WebSocketStream<TcpStream>) -> JSONRPCMessage {
    loop {
        match timeout(Duration::from_secs(1), websocket.next())
            .await
            .expect("json-rpc websocket read should not time out")
            .expect("websocket should stay open")
            .expect("websocket frame should read")
        {
            Message::Text(text) => {
                return serde_json::from_str(text.as_ref())
                    .expect("json-rpc text frame should parse");
            }
            Message::Binary(bytes) => {
                return serde_json::from_slice(bytes.as_ref())
                    .expect("json-rpc binary frame should parse");
            }
            Message::Ping(_) | Message::Pong(_) => {}
            other => panic!("expected json-rpc websocket frame, got {other:?}"),
        }
    }
}

async fn write_jsonrpc_websocket(
    websocket: &mut WebSocketStream<TcpStream>,
    message: JSONRPCMessage,
) {
    let encoded = serde_json::to_string(&message).expect("json-rpc should serialize");
    websocket
        .send(Message::Text(encoded.into()))
        .await
        .expect("json-rpc websocket frame should write");
}

async fn complete_websocket_initialize(
    websocket: &mut WebSocketStream<TcpStream>,
    session_id: &str,
) {
    let initialize = read_jsonrpc_websocket(websocket).await;
    let request = match initialize {
        JSONRPCMessage::Request(request) if request.method == INITIALIZE_METHOD => request,
        other => panic!("expected initialize request, got {other:?}"),
    };
    let params: crate::protocol::InitializeParams =
        serde_json::from_value(request.params.expect("initialize params should exist"))
            .expect("initialize params should deserialize");
    assert_eq!(params.resume_session_id, None);
    write_jsonrpc_websocket(
        websocket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: request.id,
            result: serde_json::to_value(InitializeResponse {
                session_id: session_id.to_string(),
            })
            .expect("initialize response should serialize"),
        }),
    )
    .await;

    let initialized = read_jsonrpc_websocket(websocket).await;
    match initialized {
        JSONRPCMessage::Notification(notification) if notification.method == INITIALIZED_METHOD => {
        }
        other => panic!("expected initialized notification, got {other:?}"),
    }
}

async fn complete_environment_info(websocket: &mut WebSocketStream<TcpStream>) {
    let request = match read_jsonrpc_websocket(websocket).await {
        JSONRPCMessage::Request(request) if request.method == ENVIRONMENT_INFO_METHOD => request,
        other => panic!("expected environment info request, got {other:?}"),
    };
    write_jsonrpc_websocket(
        websocket,
        JSONRPCMessage::Response(JSONRPCResponse {
            id: request.id,
            result: serde_json::to_value(EnvironmentInfo {
                shell: ShellInfo {
                    name: "sh".to_string(),
                    path: "/bin/sh".to_string(),
                },
            })
            .expect("environment info response should serialize"),
        }),
    )
    .await;
}

async fn wait_for_disconnect(client: &ExecServerClient) {
    timeout(Duration::from_secs(1), async {
        while !client.is_disconnected() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("client should observe disconnect");
}

#[tokio::test]
async fn replaces_disconnected_websocket_client_with_one_shared_connection() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!(
        "ws://{}",
        listener.local_addr().expect("listener should have address")
    );
    let server = tokio::spawn(async move {
        let mut first = accept_websocket(&listener).await;
        complete_websocket_initialize(&mut first, "session-1").await;
        first
            .close(None)
            .await
            .expect("first websocket should close");

        let mut second = accept_websocket(&listener).await;
        complete_websocket_initialize(&mut second, "session-2").await;
    });

    let client = LazyRemoteExecServerClient::new(RemoteConnectionSource::Fixed(
        ExecServerTransportParams::WebSocketUrl {
            websocket_url,
            connect_timeout: Duration::from_secs(1),
            initialize_timeout: Duration::from_secs(1),
        },
    ));
    let first = client.get().await.expect("first client should connect");
    wait_for_disconnect(&first).await;

    let (replacement_a, replacement_b) = tokio::join!(client.get(), client.get());
    let replacement_a = replacement_a.expect("first replacement should connect");
    let replacement_b = replacement_b.expect("second replacement should reuse client");
    assert_eq!(replacement_a.session_id().as_deref(), Some("session-2"));
    assert_eq!(replacement_b.session_id().as_deref(), Some("session-2"));
    assert!(Arc::ptr_eq(&replacement_a.inner, &replacement_b.inner));

    server.await.expect("server task should finish");
}

#[tokio::test]
async fn refreshes_websocket_url_for_reconnect() {
    let first_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let second_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_urls = Arc::new([
        format!(
            "ws://{}",
            first_listener
                .local_addr()
                .expect("listener should have address")
        ),
        format!(
            "ws://{}",
            second_listener
                .local_addr()
                .expect("listener should have address")
        ),
    ]);
    let server = tokio::spawn(async move {
        let mut first = accept_websocket(&first_listener).await;
        complete_websocket_initialize(&mut first, "session-1").await;
        first
            .close(None)
            .await
            .expect("first websocket should close");

        let mut second = accept_websocket(&second_listener).await;
        complete_websocket_initialize(&mut second, "session-2").await;
    });
    let provider_calls = Arc::new(AtomicUsize::new(0));
    let client =
        LazyRemoteExecServerClient::new(RemoteConnectionSource::RefreshingWebSocket(Arc::new({
            let provider_calls = Arc::clone(&provider_calls);
            let websocket_urls = Arc::clone(&websocket_urls);
            move || {
                let call = provider_calls.fetch_add(1, Ordering::Relaxed);
                let websocket_url = websocket_urls.get(call).cloned().ok_or_else(|| {
                    ExecServerError::Protocol("unexpected URL provider call".to_string())
                });
                async move { websocket_url }.boxed()
            }
        })));

    let first = client.get().await.expect("first client should connect");
    wait_for_disconnect(&first).await;
    let second = client.get().await.expect("replacement should connect");

    assert_eq!(second.session_id().as_deref(), Some("session-2"));
    assert_eq!(provider_calls.load(Ordering::Relaxed), 2);
    server.await.expect("server task should finish");
}

#[tokio::test]
async fn environment_manager_provider_is_lazy_and_refreshes_after_disconnect() {
    let first_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let second_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_urls = Arc::new([
        format!(
            "ws://{}",
            first_listener
                .local_addr()
                .expect("listener should have address")
        ),
        format!(
            "ws://{}",
            second_listener
                .local_addr()
                .expect("listener should have address")
        ),
    ]);
    let server = tokio::spawn(async move {
        let mut first = accept_websocket(&first_listener).await;
        complete_websocket_initialize(&mut first, "session-1").await;
        complete_environment_info(&mut first).await;
        first
            .close(None)
            .await
            .expect("first websocket should close");

        let mut second = accept_websocket(&second_listener).await;
        complete_websocket_initialize(&mut second, "session-2").await;
        complete_environment_info(&mut second).await;
    });
    let provider_calls = Arc::new(AtomicUsize::new(0));
    let manager = EnvironmentManager::without_environments();
    manager
        .upsert_environment_with_url_provider("executor-a".to_string(), {
            let provider_calls = Arc::clone(&provider_calls);
            let websocket_urls = Arc::clone(&websocket_urls);
            move || {
                let call = provider_calls.fetch_add(1, Ordering::Relaxed);
                let websocket_url = websocket_urls.get(call).cloned().ok_or_else(|| {
                    ExecServerError::Protocol("unexpected URL provider call".to_string())
                });
                async move { websocket_url }
            }
        })
        .expect("provider-backed environment should be installed");
    let environment = manager
        .get_environment("executor-a")
        .expect("provider-backed environment should exist");
    assert_eq!(provider_calls.load(Ordering::Relaxed), 0);

    let first_info = environment.info().await.expect("first info should succeed");
    assert_eq!(first_info.shell.name, "sh");
    let second_info = timeout(Duration::from_secs(1), async {
        loop {
            match environment.info().await {
                Ok(info) => break info,
                Err(ExecServerError::Closed | ExecServerError::Disconnected(_)) => {
                    tokio::task::yield_now().await;
                }
                Err(error) => panic!("unexpected reconnect error: {error}"),
            }
        }
    })
    .await
    .expect("provider-backed environment should reconnect");

    assert_eq!(second_info, first_info);
    assert_eq!(provider_calls.load(Ordering::Relaxed), 2);
    assert!(Arc::ptr_eq(
        &environment,
        &manager
            .get_environment("executor-a")
            .expect("environment should remain installed")
    ));
    server.await.expect("server task should finish");
}

#[tokio::test]
async fn redacts_refreshing_websocket_url_connection_errors() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let address = listener.local_addr().expect("listener should have address");
    let websocket_url = format!("ws://{address}/environment?sig=secret");
    drop(listener);
    let client = LazyRemoteExecServerClient::new(RemoteConnectionSource::RefreshingWebSocket(
        Arc::new(move || {
            let websocket_url = websocket_url.clone();
            async move { Ok(websocket_url) }.boxed()
        }),
    ));

    let error = match client.get().await {
        Ok(_) => panic!("connection should fail"),
        Err(error) => error,
    };
    let ExecServerError::WebSocketConnect { url, .. } = error else {
        panic!("expected websocket connection error");
    };
    assert_eq!(url, format!("ws://{address}/environment"));
}

#[tokio::test]
async fn coalesces_only_concurrent_provider_failures() {
    let provider_calls = Arc::new(AtomicUsize::new(0));
    let (started_tx, started_rx) = oneshot::channel();
    let started_tx = Arc::new(StdMutex::new(Some(started_tx)));
    let (release_tx, release_rx) = watch::channel(false);
    let client =
        LazyRemoteExecServerClient::new(RemoteConnectionSource::RefreshingWebSocket(Arc::new({
            let provider_calls = Arc::clone(&provider_calls);
            let started_tx = Arc::clone(&started_tx);
            move || {
                let call = provider_calls.fetch_add(1, Ordering::Relaxed);
                let started_tx = Arc::clone(&started_tx);
                let mut release_rx = release_rx.clone();
                async move {
                    if call == 0 {
                        if let Some(started_tx) = started_tx
                            .lock()
                            .unwrap_or_else(std::sync::PoisonError::into_inner)
                            .take()
                        {
                            let _ = started_tx.send(());
                        }
                        release_rx
                            .wait_for(|released| *released)
                            .await
                            .expect("release sender should stay open");
                    }
                    Err(ExecServerError::Protocol(
                        "environment registry unavailable".to_string(),
                    ))
                }
                .boxed()
            }
        })));

    let first = tokio::spawn({
        let client = client.clone();
        async move { client.get().await }
    });
    started_rx.await.expect("provider should start");
    let second = tokio::spawn({
        let client = client.clone();
        async move { client.get().await }
    });
    tokio::task::yield_now().await;
    release_tx.send(true).expect("provider should be waiting");

    let first = first.await.expect("first task should finish");
    let second = second.await.expect("second task should finish");
    match (first, second) {
        (Err(ExecServerError::Protocol(first)), Err(ExecServerError::Protocol(second))) => {
            assert_eq!(first, second);
        }
        _ => panic!("both callers should receive the same protocol error"),
    }
    assert_eq!(provider_calls.load(Ordering::Relaxed), 1);

    assert!(client.get().await.is_err());
    assert_eq!(provider_calls.load(Ordering::Relaxed), 2);
}

#[tokio::test]
async fn waiter_completes_when_connection_starter_is_cancelled() {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("listener should bind");
    let websocket_url = format!(
        "ws://{}",
        listener.local_addr().expect("listener should have address")
    );
    let (server_release_tx, server_release_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut websocket = accept_websocket(&listener).await;
        complete_websocket_initialize(&mut websocket, "session-1").await;
        let _ = server_release_rx.await;
    });

    let provider_calls = Arc::new(AtomicUsize::new(0));
    let (provider_started_tx, provider_started_rx) = oneshot::channel();
    let provider_started_tx = Arc::new(StdMutex::new(Some(provider_started_tx)));
    let (provider_release_tx, provider_release_rx) = watch::channel(false);
    let client =
        LazyRemoteExecServerClient::new(RemoteConnectionSource::RefreshingWebSocket(Arc::new({
            let provider_calls = Arc::clone(&provider_calls);
            move || {
                provider_calls.fetch_add(1, Ordering::Relaxed);
                let provider_started_tx = Arc::clone(&provider_started_tx);
                let mut provider_release_rx = provider_release_rx.clone();
                let websocket_url = websocket_url.clone();
                async move {
                    if let Some(provider_started_tx) = provider_started_tx
                        .lock()
                        .unwrap_or_else(std::sync::PoisonError::into_inner)
                        .take()
                    {
                        let _ = provider_started_tx.send(());
                    }
                    provider_release_rx
                        .wait_for(|released| *released)
                        .await
                        .expect("provider release sender should stay open");
                    Ok(websocket_url)
                }
                .boxed()
            }
        })));

    let starter = tokio::spawn({
        let client = client.clone();
        async move { client.get().await }
    });
    provider_started_rx
        .await
        .expect("connection provider should start");
    let waiter = tokio::spawn({
        let client = client.clone();
        async move { client.get().await }
    });
    tokio::task::yield_now().await;
    starter.abort();
    match starter.await {
        Err(error) if error.is_cancelled() => {}
        _ => panic!("connection starter should be cancelled"),
    }
    provider_release_tx
        .send(true)
        .expect("connection provider should be waiting");

    let connected = waiter
        .await
        .expect("waiter task should finish")
        .expect("waiter should complete the shared connection");
    let cached = client
        .get()
        .await
        .expect("connected client should be cached");
    assert!(Arc::ptr_eq(&connected.inner, &cached.inner));
    assert_eq!(provider_calls.load(Ordering::Relaxed), 1);

    let _ = server_release_tx.send(());
    server.await.expect("server task should finish");
}
