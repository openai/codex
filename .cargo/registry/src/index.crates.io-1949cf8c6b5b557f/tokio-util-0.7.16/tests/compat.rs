#![cfg(feature = "compat")]
#![cfg(not(target_os = "wasi"))] // WASI does not support all fs operations
#![warn(rust_2018_idioms)]

use futures_io::SeekFrom;
use futures_util::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tempfile::NamedTempFile;
use tokio::fs::OpenOptions;
use tokio_util::compat::TokioAsyncWriteCompatExt;

#[tokio::test]
async fn compat_file_seek() -> futures_util::io::Result<()> {
    let temp_file = NamedTempFile::new()?;
    let mut file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(true)
        .open(temp_file)
        .await?
        .compat_write();

    file.write_all(&[0, 1, 2, 3, 4, 5]).await?;
    file.write_all(&[6, 7]).await?;

    assert_eq!(file.stream_position().await?, 8);

    // Modify elements at position 2.
    assert_eq!(file.seek(SeekFrom::Start(2)).await?, 2);
    file.write_all(&[8, 9]).await?;

    file.flush().await?;

    // Verify we still have 8 elements.
    assert_eq!(file.seek(SeekFrom::End(0)).await?, 8);
    // Seek back to the start of the file to read and verify contents.
    file.seek(SeekFrom::Start(0)).await?;

    let mut buf = Vec::new();
    let num_bytes = file.read_to_end(&mut buf).await?;
    assert_eq!(&buf[..num_bytes], &[0, 1, 8, 9, 4, 5, 6, 7]);

    Ok(())
}
