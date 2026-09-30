//! Regression coverage for bounded, cancellable reads and regular-file validation.

#[cfg(unix)]
use super::CancellableReader;
use super::MAX_READ_FILE_BYTES;
use crate::ReadFileOptions;
use crate::local_file_system::LOCAL_FS;
use codex_utils_path_uri::PathUri;
use pretty_assertions::assert_eq;
use tempfile::TempDir;
use test_case::test_case;

#[test_case(true ; "follow")]
#[test_case(false ; "no_follow")]
#[tokio::test]
async fn reads_file_larger_than_tokio_buffer(follow_symlinks: bool) -> anyhow::Result<()> {
    let directory = TempDir::new()?;
    let path = directory.path().canonicalize()?.join("large.bin");
    let contents = vec![0x5a; 4 * 1024 * 1024];
    std::fs::write(&path, &contents)?;
    assert_eq!(
        LOCAL_FS
            .read_file(
                &PathUri::from_host_native_path(&path)?,
                ReadFileOptions { follow_symlinks },
                /*sandbox*/ None,
            )
            .await?,
        contents,
    );
    Ok(())
}

#[test_case(true ; "follow")]
#[test_case(false ; "no_follow")]
#[tokio::test]
async fn rejects_oversized_file(follow_symlinks: bool) -> anyhow::Result<()> {
    let directory = TempDir::new()?;
    let path = directory.path().canonicalize()?.join("oversized.bin");
    std::fs::File::create(&path)?.set_len(MAX_READ_FILE_BYTES + 1)?;
    let error = LOCAL_FS
        .read_file(
            &PathUri::from_host_native_path(&path)?,
            ReadFileOptions { follow_symlinks },
            /*sandbox*/ None,
        )
        .await
        .expect_err("oversized file must be rejected before allocation");
    assert_eq!(error.kind(), std::io::ErrorKind::InvalidInput);
    Ok(())
}

#[cfg(unix)]
#[test]
fn cancellation_stops_a_reader_after_its_first_chunk() -> anyhow::Result<()> {
    use std::io::Read;
    use std::io::Write;
    use std::os::unix::net::UnixStream;
    use tokio_util::sync::CancellationToken;

    let (socket, mut writer) = UnixStream::pair()?;
    let cancelled = CancellationToken::new();
    let (reached_tx, reached_rx) = std::sync::mpsc::channel();
    let (resume_tx, resume_rx) = std::sync::mpsc::channel::<()>();
    let worker = std::thread::spawn({
        let cancelled = cancelled.clone();
        move || -> std::io::Result<()> {
            let mut reader = CancellableReader {
                reader: socket,
                cancelled,
            };
            let mut first = [0];
            reader.read_exact(&mut first)?;
            let _ = reached_tx.send(first);
            let _ = resume_rx.recv();
            reader.read_to_end(&mut Vec::new()).map(|_| ())
        }
    });
    writer.write_all(b"x")?;
    let first = reached_rx.recv()?;
    cancelled.cancel();
    // Closing the peer also makes a missing cancellation check fail without hanging the test.
    drop(writer);
    drop(resume_tx);
    let error = worker
        .join()
        .unwrap()
        .expect_err("cancelled read must stop");
    assert_eq!((first, error.kind()), (*b"x", std::io::ErrorKind::Other));
    Ok(())
}
