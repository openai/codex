use std::io::Read;
use std::io::Write;
use std::sync::Arc;

use codex_utils_pty::SpawnedProcess;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

const STDIN_FORWARD_CHUNK_SIZE: usize = 8 * 1024;
const OUTPUT_DRAIN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

pub async fn forward_sandbox_session_stdio(spawned: SpawnedProcess) -> i32 {
    let session = Arc::new(spawned.session);
    let tokio_runtime = tokio::runtime::Handle::current();
    let (stdin_eof_tx, stdin_eof_rx) = oneshot::channel();

    drop(spawn_input_forwarder(
        std::io::stdin(),
        session.writer_sender(),
        stdin_eof_tx,
    ));
    let (stdout_forwarder, stdout_forwarder_done_rx) =
        spawn_output_forwarder(tokio_runtime.clone(), spawned.stdout_rx, std::io::stdout());
    drop(stdout_forwarder);
    let (stderr_forwarder, stderr_forwarder_done_rx) =
        spawn_output_forwarder(tokio_runtime, spawned.stderr_rx, std::io::stderr());
    drop(stderr_forwarder);

    let stdin_close_task = tokio::spawn({
        let session = Arc::clone(&session);
        async move {
            let _ = stdin_eof_rx.await;
            session.close_stdin();
        }
    });

    let mut exit_rx = spawned.exit_rx;
    let exit_code = tokio::select! {
        res = &mut exit_rx => res.unwrap_or(-1),
        res = tokio::signal::ctrl_c() => {
            if let Ok(()) = res {
                session.request_terminate();
            }
            exit_rx.await.unwrap_or(-1)
        }
    };

    stdin_close_task.abort();
    let _ = tokio::time::timeout(OUTPUT_DRAIN_TIMEOUT, async {
        let _ = stdout_forwarder_done_rx.await;
        let _ = stderr_forwarder_done_rx.await;
    })
    .await;
    exit_code
}

fn spawn_input_forwarder<R>(
    mut input: R,
    writer_tx: mpsc::Sender<Vec<u8>>,
    stdin_eof_tx: oneshot::Sender<()>,
) -> std::thread::JoinHandle<()>
where
    R: Read + Send + 'static,
{
    std::thread::spawn(move || {
        let mut buffer = [0_u8; STDIN_FORWARD_CHUNK_SIZE];
        loop {
            match input.read(&mut buffer) {
                Ok(0) => break,
                Ok(n) => {
                    if writer_tx.blocking_send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::Interrupted => continue,
                Err(err) => {
                    eprintln!("windows sandbox stdin forwarder failed: {err}");
                    break;
                }
            }
        }
        let _ = stdin_eof_tx.send(());
    })
}

fn spawn_output_forwarder<W>(
    tokio_runtime: tokio::runtime::Handle,
    output_rx: mpsc::Receiver<Vec<u8>>,
    mut writer: W,
) -> (std::thread::JoinHandle<()>, oneshot::Receiver<()>)
where
    W: Write + Send + 'static,
{
    let (done_tx, done_rx) = oneshot::channel();
    let handle = std::thread::spawn(move || {
        let mut output_rx = output_rx;
        while let Some(chunk) = tokio_runtime.block_on(output_rx.recv()) {
            if let Err(err) = writer.write_all(&chunk) {
                eprintln!("windows sandbox output forwarder failed to write: {err}");
                break;
            }
            if let Err(err) = writer.flush() {
                eprintln!("windows sandbox output forwarder failed to flush: {err}");
                break;
            }
        }
        let _ = done_tx.send(());
    });
    (handle, done_rx)
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use pretty_assertions::assert_eq;

    use super::*;

    #[tokio::test]
    async fn input_forwarder_sends_chunks_and_reports_eof() -> anyhow::Result<()> {
        let (writer_tx, mut writer_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(4);
        let (stdin_closed_tx, stdin_closed_rx) = tokio::sync::oneshot::channel();
        let input = std::io::Cursor::new(b"first\nsecond\n".to_vec());

        let forwarder = spawn_input_forwarder(input, writer_tx, stdin_closed_tx);
        let mut received = Vec::new();
        while let Some(chunk) = writer_rx.recv().await {
            received.extend_from_slice(&chunk);
        }
        stdin_closed_rx.await?;
        forwarder.join().expect("stdin forwarder should finish");

        assert_eq!(received, b"first\nsecond\n".to_vec());
        Ok(())
    }

    #[tokio::test]
    async fn output_forwarder_writes_all_chunks() -> anyhow::Result<()> {
        #[derive(Clone, Default)]
        struct SharedWriter(std::sync::Arc<Mutex<Vec<u8>>>);

        impl std::io::Write for SharedWriter {
            fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
                let mut guard = self
                    .0
                    .lock()
                    .map_err(|_| std::io::Error::other("writer poisoned"))?;
                guard.extend_from_slice(buf);
                Ok(buf.len())
            }

            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }

        let runtime = tokio::runtime::Handle::current();
        let (output_tx, output_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(4);
        let writer = SharedWriter::default();
        let sink = std::sync::Arc::clone(&writer.0);

        let (forwarder, done_rx) = spawn_output_forwarder(runtime, output_rx, writer);
        output_tx.send(b"alpha".to_vec()).await?;
        output_tx.send(b"beta".to_vec()).await?;
        drop(output_tx);
        forwarder.join().expect("output forwarder should finish");
        done_rx.await?;

        let output = sink
            .lock()
            .map_err(|_| anyhow::anyhow!("writer poisoned"))?
            .clone();
        assert_eq!(output, b"alphabeta".to_vec());
        Ok(())
    }
}
