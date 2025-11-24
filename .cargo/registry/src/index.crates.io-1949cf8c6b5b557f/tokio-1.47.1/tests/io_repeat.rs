#![warn(rust_2018_idioms)]
#![cfg(all(feature = "full", not(miri)))]

use tokio::io::AsyncReadExt;

#[tokio::test]
async fn repeat_poll_read_is_cooperative() {
    tokio::select! {
        biased;
        _ = async {
            loop {
                let mut buf = [0u8; 4096];
                tokio::io::repeat(0b101).read_exact(&mut buf).await.unwrap();
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}
