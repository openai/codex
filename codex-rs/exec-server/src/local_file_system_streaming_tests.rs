use super::*;
use pretty_assertions::assert_eq;

#[tokio::test]
async fn close_waits_for_in_flight_blocking_write() -> io::Result<()> {
    let temp_dir = tempfile::TempDir::new()?;
    let path = temp_dir.path().join("output");
    let file = std::fs::File::create(&path)?;
    let (started_tx, started_rx) = std::sync::mpsc::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let write_task = tokio::task::spawn_blocking(move || {
        started_tx.send(()).expect("signal write start");
        release_rx.recv().expect("release write");
        let mut file = file;
        let result = std::io::Write::write_all(&mut file, b"x");
        (file, result)
    });
    let handle = Arc::new(DirectFileWriteHandle {
        state: tokio::sync::Mutex::new(DirectFileWriteState {
            file: None,
            write_task: Some(write_task),
        }),
    });
    tokio::task::spawn_blocking(move || started_rx.recv())
        .await
        .expect("join write start")
        .expect("wait for write start");

    let close = {
        let handle = Arc::clone(&handle);
        tokio::spawn(async move { handle.close().await })
    };
    tokio::task::yield_now().await;
    assert!(!close.is_finished());

    release_tx.send(()).expect("release write");
    close.await.expect("join close")?;
    assert_eq!(std::fs::read(path)?, b"x");
    Ok(())
}
