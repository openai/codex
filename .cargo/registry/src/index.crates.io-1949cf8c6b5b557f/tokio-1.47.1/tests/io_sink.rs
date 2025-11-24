#![warn(rust_2018_idioms)]
#![cfg(feature = "full")]

use tokio::io::AsyncWriteExt;

#[tokio::test]
async fn sink_poll_write_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                let buf = vec![1, 2, 3];
                tokio::io::sink().write_all(&buf).await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}

#[tokio::test]
async fn sink_poll_flush_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                tokio::io::sink().flush().await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}

#[tokio::test]
async fn sink_poll_shutdown_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                tokio::io::sink().shutdown().await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
