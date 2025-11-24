use futures_util::StreamExt;
use hyper::Request;
use hyper_util::rt::TokioIo;
use sse_stream::SseStream;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod sse_server_side;
#[tokio::test]
async fn test_axum_with_reqwest() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("info,{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
    sse_server_side::axum::start_serve("127.0.0.1:8080").await?;
    let tcp_stream = tokio::net::TcpStream::connect("127.0.0.1:8080").await?;
    let (mut s, c) =
        hyper::client::conn::http1::handshake::<_, String>(TokioIo::new(tcp_stream)).await?;
    tokio::spawn(c.with_upgrades());
    let req = Request::new(String::new());
    let response = s.send_request(req).await?;
    let mut sse_body = SseStream::new(response);
    let mut receive_count = 0;
    while let Some(Ok(sse)) = sse_body.next().await {
        assert!(sse.data.is_some());
        assert!(sse.event.is_some());
        assert!(sse.id.is_some());
        assert!(sse.retry.is_some());
        receive_count += 1;
    }
    tracing::info!("receive_count: {}", receive_count);
    assert_eq!(receive_count, sse_server_side::axum::MESSAGE_TOTAL_COUNT);
    Ok(())
}
