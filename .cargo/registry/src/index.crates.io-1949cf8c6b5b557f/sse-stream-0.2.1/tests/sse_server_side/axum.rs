use axum::{
    response::sse::{Event, Sse},
    routing::get,
    Router,
};
use futures_util::{stream::repeat_with, Stream, StreamExt};

use anyhow::Result;
use std::time::Duration;
use tokio::io::{self};

fn router() -> Router {
    Router::new().route("/", get(sse_handler))
}

pub const MESSAGE_TOTAL_COUNT: usize = 100000;
async fn sse_handler() -> Sse<impl Stream<Item = Result<Event, io::Error>>> {
    tracing::info!("sse connection");
    let mut repeat_count = 0;
    let stream = repeat_with(move || {
        repeat_count += 1;
        Ok(Event::default()
            .event("hello")
            .id(repeat_count.to_string())
            .comment("whatever")
            .retry(Duration::from_millis(1000))
            .data(format!("world-{repeat_count}")))
    })
    .take(MESSAGE_TOTAL_COUNT);
    Sse::new(stream)
}

pub async fn start_serve(addr: &str) -> io::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;

    tracing::debug!("listening on {}", listener.local_addr()?);
    tokio::spawn(async move { axum::serve(listener, router()).await });
    Ok(())
}
