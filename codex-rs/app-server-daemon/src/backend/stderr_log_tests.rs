use super::MAX_CURRENT_LOG_BYTES;
use super::MAX_PREVIOUS_LOG_BYTES;
use super::preserve;
use super::prune;
use pretty_assertions::assert_eq;

#[cfg(any(unix, windows))]
#[tokio::test]
async fn pruning_resets_the_write_position_of_a_legacy_nonappend_writer() -> anyhow::Result<()> {
    use std::io::Write;

    const CHILD_LOG: &str = "CODEX_TEST_LEGACY_STDERR_LOG";
    if let Some(path) = std::env::var_os(CHILD_LOG) {
        std::io::stderr().write_all(b"before pruning\n")?;
        let mut writer = super::LogFile {
            path: path.into(),
            file: None,
            can_prune: None,
        };
        writer.write_all(b"queued logging\n")?;
        std::io::stderr().write_all(b"still logging\n")?;
        return Ok(());
    }

    let home = tempfile::tempdir()?;
    let path = home.path().join("daemon.stderr.log");
    let mut writer = std::fs::File::create(&path)?;
    writer.write_all(&vec![b'x'; MAX_CURRENT_LOG_BYTES as usize + 1])?;
    // Isolate the process-wide stderr replacement and inherit the legacy writer's offset.
    let status = tokio::process::Command::new(std::env::current_exe()?)
        .args([
            "--exact",
            "backend::stderr_log::tests::pruning_resets_the_write_position_of_a_legacy_nonappend_writer",
            "--nocapture",
        ])
        .env(CHILD_LOG, &path)
        .stderr(writer)
        .kill_on_drop(true)
        .status()
        .await?;
    assert!(status.success(), "legacy stderr child failed: {status}");
    assert_eq!(
        tokio::fs::read(&path).await?,
        b"queued logging\nstill logging\n"
    );
    Ok(())
}

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

#[tokio::test]
#[allow(clippy::await_holding_invalid_type)] // Exercise pruning exclusion during handoff I/O.
async fn prunes_live_log_without_disconnecting_inherited_append_handles() -> anyhow::Result<()> {
    use tokio::io::AsyncWriteExt;

    let pruning_enabled = tokio::sync::Mutex::new(true);
    let home = tempfile::tempdir()?;
    let path = home.path().join("daemon.stderr.log");
    let mut writer = tokio::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .await?;
    writer.write_all(b"startup\n").await?;
    writer.flush().await?;
    prune(&path, &pruning_enabled)?;
    assert_eq!(tokio::fs::read(&path).await?, b"startup\n");

    let mut bytes = vec![b'x'; MAX_CURRENT_LOG_BYTES as usize];
    bytes.extend_from_slice(b"latest failure\n");
    writer.write_all(&bytes).await?;
    writer.flush().await?;

    // A failed tail publication must leave the live log untouched.
    let previous = path.with_extension("log.previous");
    tokio::fs::create_dir(&previous).await?;
    assert!(prune(&path, &pruning_enabled).is_err());
    assert_eq!(
        tokio::fs::read(&path).await?,
        [b"startup\n".as_slice(), bytes.as_slice()].concat()
    );
    tokio::fs::remove_dir(&previous).await?;

    // Handoff excludes pruning even while the shared log exceeds the limit.
    let mut handoff = pruning_enabled.try_lock()?;
    *handoff = false;
    prune(&path, &pruning_enabled)?;
    assert_eq!(
        tokio::fs::read(&path).await?,
        [b"startup\n".as_slice(), bytes.as_slice()].concat()
    );

    // Failed handoff restores ownership, so retention resumes after unlocking.
    *handoff = true;
    drop(handoff);
    prune(&path, &pruning_enabled)?;
    assert_eq!(
        tokio::fs::read(previous).await?,
        bytes[bytes.len() - MAX_PREVIOUS_LOG_BYTES as usize..]
    );
    writer.write_all(b"still logging\n").await?;
    writer.flush().await?;
    assert_eq!(tokio::fs::read(&path).await?, b"still logging\n");

    // Successful handoff keeps predecessor pruning disabled after unlocking.
    let mut handoff = pruning_enabled.try_lock()?;
    *handoff = false;
    writer.write_all(&bytes).await?;
    writer.write_all(b"successor startup\n").await?;
    writer.flush().await?;
    drop(handoff);
    prune(&path, &pruning_enabled)?;
    assert_eq!(
        tokio::fs::read(&path).await?,
        [
            b"still logging\n".as_slice(),
            bytes.as_slice(),
            b"successor startup\n",
        ]
        .concat()
    );
    Ok(())
}

#[test]
fn logging_does_not_wait_for_a_stalled_worker() -> anyhow::Result<()> {
    use std::io::Write;
    use std::sync::Barrier;
    use std::sync::mpsc;

    const CHILD: &str = "CODEX_TEST_STALLED_LOG_WORKER";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe()?)
            .args([
                "--exact",
                "backend::stderr_log::tests::logging_does_not_wait_for_a_stalled_worker",
            ])
            .env(CHILD, "1")
            .output()?;
        assert!(
            output.status.success(),
            "logging child failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(output.stderr, Vec::<u8>::new());
        return Ok(());
    }

    struct StalledWriter(mpsc::Sender<()>);
    impl Write for StalledWriter {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            self.0.send(()).unwrap();
            // No second participant: the worker stays stalled until process exit.
            Barrier::new(/* n */ 2).wait();
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let (entered_tx, entered_rx) = mpsc::channel();
    assert!(
        super::LOGGER
            .set(super::nonblocking(StalledWriter(entered_tx)))
            .is_ok()
    );
    // The updater's diagnostic starts the stalled write.
    crate::diagnostics::event("stalled-worker", serde_json::json!({}));
    entered_rx.recv()?;
    // Exercise the same tracing formatter/writer combination used by app-server.
    let subscriber = tracing_subscriber::fmt()
        .with_writer(super::stderr_writer)
        .finish();
    tracing::subscriber::with_default(subscriber, || tracing::warn!("app-server event"));
    super::stderr_writer().write_all(&vec![b'x'; super::MAX_RECORD_BYTES + 1])?;
    for _ in 1..super::QUEUED_RECORDS {
        super::stderr_writer().write_all(&vec![b'x'; super::MAX_RECORD_BYTES])?;
    }
    super::stderr_writer().write_all(b"queue is full\n")?;
    super::stderr_writer().flush()?;
    assert_eq!(
        super::LOGGER
            .get()
            .unwrap()
            .0
            .error_counter()
            .dropped_lines(),
        1
    );
    // Returning also proves process shutdown does not wait for the stalled worker.
    Ok(())
}
