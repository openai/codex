use futures_util::StreamExt;
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
    let client = reqwest::Client::new();
    let response = client.get("http://127.0.0.1:8080/").send().await?;
    let mut sse_body = SseStream::from_byte_stream(response.bytes_stream());
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
