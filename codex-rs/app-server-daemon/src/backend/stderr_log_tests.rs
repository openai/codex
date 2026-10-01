use super::MAX_PREVIOUS_LOG_BYTES;
use super::preserve;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn preserves_bounded_tail_and_keeps_it_after_empty_launch() -> anyhow::Result<()> {
    let home = tempfile::tempdir()?;
    let path = home.path().join("daemon.stderr.log");
    let previous = path.with_extension("log.previous");
    preserve(&path).await;
    assert!(!previous.exists());
    let mut bytes = vec![b'x'; MAX_PREVIOUS_LOG_BYTES as usize + 100];
    bytes.extend_from_slice(b"latest failure");
    tokio::fs::write(&path, &bytes).await?;
    preserve(&path).await;
    assert_eq!(
        tokio::fs::read(&previous).await?,
        bytes[bytes.len() - MAX_PREVIOUS_LOG_BYTES as usize..]
    );
    tokio::fs::write(&path, b"").await?;
    preserve(&path).await;
    assert_eq!(
        tokio::fs::read(&previous).await?,
        bytes[bytes.len() - MAX_PREVIOUS_LOG_BYTES as usize..]
    );
    tokio::fs::write(&path, b"next launch").await?;
    preserve(&path).await;
    assert_eq!(tokio::fs::read(&previous).await?, b"next launch");
    Ok(())
}
