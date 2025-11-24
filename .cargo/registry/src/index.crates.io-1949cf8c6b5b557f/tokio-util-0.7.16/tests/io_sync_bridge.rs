#![cfg(feature = "io-util")]
#![cfg(not(target_os = "wasi"))] // Wasi doesn't support threads

use std::error::Error;
use std::io::{Cursor, Read, Result as IoResult, Write};
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio_util::io::SyncIoBridge;

async fn test_reader_len(
    r: impl AsyncRead + Unpin + Send + 'static,
    expected_len: usize,
) -> IoResult<()> {
    let mut r = SyncIoBridge::new(r);
    let res = tokio::task::spawn_blocking(move || {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf)?;
        Ok::<_, std::io::Error>(buf)
    })
    .await?;
    assert_eq!(res?.len(), expected_len);
    Ok(())
}

#[tokio::test]
async fn test_async_read_to_sync() -> Result<(), Box<dyn Error>> {
    test_reader_len(tokio::io::empty(), 0).await?;
    let buf = b"hello world";
    test_reader_len(Cursor::new(buf), buf.len()).await?;
    Ok(())
}

#[tokio::test]
async fn test_async_write_to_sync() -> Result<(), Box<dyn Error>> {
    let mut dest = Vec::new();
    let src = b"hello world";
    let dest = tokio::task::spawn_blocking(move || -> Result<_, String> {
        let mut w = SyncIoBridge::new(Cursor::new(&mut dest));
        std::io::copy(&mut Cursor::new(src), &mut w).map_err(|e| e.to_string())?;
        Ok(dest)
    })
    .await??;
    assert_eq!(dest.as_slice(), src);
    Ok(())
}

#[tokio::test]
async fn test_into_inner() -> Result<(), Box<dyn Error>> {
    let mut buf = Vec::new();
    SyncIoBridge::new(tokio::io::empty())
        .into_inner()
        .read_to_end(&mut buf)
        .await
        .unwrap();
    assert_eq!(buf.len(), 0);
    Ok(())
}

#[tokio::test]
async fn test_shutdown() -> Result<(), Box<dyn Error>> {
    let (s1, mut s2) = tokio::io::duplex(1024);
    let (_rh, wh) = tokio::io::split(s1);
    tokio::task::spawn_blocking(move || -> std::io::Result<_> {
        let mut wh = SyncIoBridge::new(wh);
        wh.write_all(b"hello")?;
        wh.shutdown()?;
        assert!(wh.write_all(b" world").is_err());
        Ok(())
    })
    .await??;
    let mut buf = vec![];
    s2.read_to_end(&mut buf).await?;
    assert_eq!(buf, b"hello");
    Ok(())
}
