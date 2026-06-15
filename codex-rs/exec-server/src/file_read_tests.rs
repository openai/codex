use std::io;

use anyhow::Result;
use pretty_assertions::assert_eq;
use tempfile::TempDir;

use super::FileReadBlock;
use super::FileReadHandleManager;
use super::MAX_OPEN_FILE_READS;

#[tokio::test]
async fn reads_blocks_at_non_sequential_offsets() -> Result<()> {
    let tmp = TempDir::new()?;
    let path = tmp.path().join("non-sequential.bin");
    std::fs::write(&path, b"0123456789")?;
    let manager = FileReadHandleManager::default();
    let handle_id = manager.open(tokio::fs::File::open(&path).await?).await?;

    assert_eq!(
        manager
            .read_block(&handle_id, /*offset*/ 6, /*len*/ 3)
            .await?,
        FileReadBlock {
            bytes: b"678".to_vec(),
            eof: false,
        }
    );
    assert_eq!(
        manager
            .read_block(&handle_id, /*offset*/ 1, /*len*/ 2)
            .await?,
        FileReadBlock {
            bytes: b"12".to_vec(),
            eof: false,
        }
    );
    assert_eq!(
        manager
            .read_block(&handle_id, /*offset*/ 8, /*len*/ 4)
            .await?,
        FileReadBlock {
            bytes: b"89".to_vec(),
            eof: true,
        }
    );
    Ok(())
}

#[tokio::test]
async fn limits_open_files_and_releases_capacity_on_close() -> Result<()> {
    let tmp = TempDir::new()?;
    let path = tmp.path().join("limited.bin");
    std::fs::write(&path, b"limited")?;
    let manager = FileReadHandleManager::default();
    let mut handles = Vec::with_capacity(MAX_OPEN_FILE_READS);
    for _ in 0..MAX_OPEN_FILE_READS {
        handles.push(manager.open(tokio::fs::File::open(&path).await?).await?);
    }

    let error = manager
        .open(tokio::fs::File::open(&path).await?)
        .await
        .expect_err("opening beyond the limit should fail");
    assert_eq!(
        (error.kind(), error.to_string()),
        (
            io::ErrorKind::InvalidInput,
            format!("at most {MAX_OPEN_FILE_READS} file reads may be open per connection"),
        )
    );

    manager.close(&handles[0]).await;
    manager.open(tokio::fs::File::open(&path).await?).await?;
    Ok(())
}
