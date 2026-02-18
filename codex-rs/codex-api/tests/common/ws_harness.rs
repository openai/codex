use std::collections::HashMap;
use std::future::Future;
use std::time::Duration;

use codex_api::Provider;
use codex_api::provider::RetryConfig;
use http::HeaderMap;
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::WebSocketStream;
use tokio_tungstenite::accept_async;

pub(crate) async fn spawn_ws_server<F, Fut>(handler: F) -> (String, JoinHandle<()>)
where
    F: FnOnce(WebSocketStream<tokio::net::TcpStream>) -> Fut + Send + 'static,
    Fut: Future<Output = ()> + Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("local addr");
    let server = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let ws = accept_async(stream).await.expect("accept ws");
        handler(ws).await;
    });
    (format!("ws://{addr}"), server)
}

pub(crate) fn test_provider() -> Provider {
    Provider {
        name: "test".to_string(),
        base_url: "http://localhost".to_string(),
        query_params: Some(HashMap::new()),
        headers: HeaderMap::new(),
        retry: RetryConfig {
            max_attempts: 1,
            base_delay: Duration::from_millis(1),
            retry_429: false,
            retry_5xx: false,
            retry_transport: false,
        },
        stream_idle_timeout: Duration::from_secs(5),
    }
}
