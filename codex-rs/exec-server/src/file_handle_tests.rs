//! Exercises permit cleanup and progress without relying on I/O timing.

use std::future::pending;
use std::future::ready;
use std::io;
use std::io::Write;
use std::task::Poll;

use anyhow::Result;
use pretty_assertions::assert_eq;
use tokio::fs::File;
use tokio::sync::oneshot;

use super::FileHandleManager;
use super::FileReadBlock;
use super::MAX_OPEN_FILES;

/// A stalled open must let unrelated handles open and read without waiting for it.
#[tokio::test]
async fn stalled_open_does_not_block_other_handles() -> Result<()> {
    let manager = FileHandleManager::default();
    let mut stalled = Box::pin(manager.open("stalled".to_string(), pending()));
    assert!(futures::poll!(stalled.as_mut()).is_pending());

    let mut other = Box::pin(manager.open(
        "other".to_string(),
        ready(Ok(File::from_std(tempfile::tempfile()?))),
    ));
    let Poll::Ready(result) = futures::poll!(other.as_mut()) else {
        anyhow::bail!("an unrelated open was blocked by the stalled open");
    };
    assert_eq!(result?, "other");
    manager.read_block("other", /*offset*/ 0, /*len*/ 1).await?;
    Ok(())
}

/// Pending opens consume capacity before polling another file open.
#[tokio::test]
async fn pending_opens_reserve_capacity() -> Result<()> {
    let manager = FileHandleManager::default();
    let mut opens = Vec::new();
    for index in 0..MAX_OPEN_FILES {
        let mut open = Box::pin(manager.open(index.to_string(), pending()));
        assert!(futures::poll!(open.as_mut()).is_pending());
        opens.push(open);
    }
    let mut rejected = Box::pin(manager.open("overflow".to_string(), pending()));
    let Poll::Ready(Err(error)) = futures::poll!(rejected.as_mut()) else {
        anyhow::bail!("admission must fail before polling the file open");
    };
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    Ok(())
}

/// Failed and cancelled opens must return their permits for reuse.
#[test_case::test_case(true; "cancelled")]
#[test_case::test_case(false; "failed")]
#[tokio::test]
async fn unfinished_opens_release_capacity(cancelled: bool) -> Result<()> {
    let manager = FileHandleManager::default();
    for _ in 0..=MAX_OPEN_FILES {
        let mut open = Box::pin(manager.open("reused".to_string(), async {
            if cancelled {
                pending::<()>().await;
            }
            Err(io::Error::other("open failed"))
        }));
        let result = futures::poll!(open.as_mut());
        if cancelled {
            assert!(result.is_pending());
        } else {
            let Poll::Ready(Err(error)) = result else {
                anyhow::bail!("the open error must reach the caller");
            };
            assert_eq!(error.kind(), io::ErrorKind::Other);
        }
        drop(open);
    }
    Ok(())
}

/// Closing an entry frees capacity even while an operation retains its file descriptor.
#[tokio::test]
async fn close_releases_capacity_while_the_file_is_still_in_use() -> Result<()> {
    let manager = FileHandleManager::default();
    let file = tempfile::tempfile()?;
    for index in 0..MAX_OPEN_FILES {
        manager
            .open(
                index.to_string(),
                ready(Ok(File::from_std(file.try_clone()?))),
            )
            .await?;
    }
    let retained = manager.get("0")?;
    manager.close("0");
    manager
        .open("next".to_string(), ready(Ok(File::from_std(file))))
        .await?;
    drop(retained);
    Ok(())
}

/// Concurrent opens of one ID must reject the loser without changing the winning file.
#[tokio::test]
async fn concurrent_opens_reject_duplicate_ids_without_replacing_the_file() -> Result<()> {
    let manager = FileHandleManager::default();
    let (send, receive) = oneshot::channel();
    let mut late = Box::pin(manager.open("shared".to_string(), async {
        receive.await.map_err(io::Error::other)
    }));
    assert!(futures::poll!(late.as_mut()).is_pending());

    let mut file = tempfile::tempfile()?;
    file.write_all(b"first")?;
    manager
        .open("shared".to_string(), ready(Ok(File::from_std(file))))
        .await?;
    send.send(File::from_std(tempfile::tempfile()?))
        .map_err(|_| anyhow::anyhow!("the pending open dropped its receiver"))?;

    let error = late.await.expect_err("the duplicate open must fail");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert_eq!(
        manager
            .read_block("shared", /*offset*/ 0, /*len*/ 6)
            .await?,
        FileReadBlock {
            bytes: b"first".to_vec(),
            eof: true,
        }
    );
    Ok(())
}
