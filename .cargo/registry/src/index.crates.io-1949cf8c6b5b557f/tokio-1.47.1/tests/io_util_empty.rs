#![cfg(feature = "full")]
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncSeekExt};

#[tokio::test]
async fn empty_read_is_cooperative() {
    tokio::select! {
        biased;

        _ = async {
            loop {
                let mut buf = [0u8; 4096];
                let _ = tokio::io::empty().read(&mut buf).await;
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}

#[tokio::test]
async fn empty_buf_reads_are_cooperative() {
    tokio::select! {
        biased;

        _ = async {
            loop {
                let mut buf = String::new();
                let _ = tokio::io::empty().read_line(&mut buf).await;
            }
        } => {},
        _ = tokio::task::yield_now() => {}
    }
}

#[tokio::test]
async fn empty_seek() {
    use std::io::SeekFrom;

    let mut empty = tokio::io::empty();

    assert!(matches!(empty.seek(SeekFrom::Start(0)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::Start(1)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::Start(u64::MAX)).await, Ok(0)));

    assert!(matches!(empty.seek(SeekFrom::End(i64::MIN)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::End(-1)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::End(0)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::End(1)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::End(i64::MAX)).await, Ok(0)));

    assert!(matches!(
        empty.seek(SeekFrom::Current(i64::MIN)).await,
        Ok(0)
    ));
    assert!(matches!(empty.seek(SeekFrom::Current(-1)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::Current(0)).await, Ok(0)));
    assert!(matches!(empty.seek(SeekFrom::Current(1)).await, Ok(0)));
    assert!(matches!(
        empty.seek(SeekFrom::Current(i64::MAX)).await,
        Ok(0)
    ));
}
