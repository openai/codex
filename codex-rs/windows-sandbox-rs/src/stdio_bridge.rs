use std::io::Read;
use std::io::Write;
use std::sync::Arc;
use std::time::Duration;

use codex_utils_pty::SpawnedProcess;
use tokio::sync::mpsc;
use tokio::sync::oneshot;

/// Forwards this process' stdio to a Windows sandbox session and returns the
/// session exit code.
pub async fn forward_sandbox_session_stdio(spawned: SpawnedProcess) -> i32 {
    let session = Arc::new(spawned.session);
    let tokio_runtime = tokio::runtime::Handle::current();
    // Give large or slow tail output a better chance to finish draining without
    // letting rare EOF issues hang the wrapper indefinitely.
    let output_drain_timeout = Duration::from_secs(5);
    // A helper thread watches our stdin. When the input source closes it, the
    // thread tells the main async code so we can also close stdin for the
    // sandboxed child process.
    let (stdin_eof_tx, stdin_eof_rx) = oneshot::channel();

    // Start background threads that copy stdin/stdout/stderr. We intentionally
    // do not keep their JoinHandles; dropping the handle does not stop the
    // thread, it just means we are not going to wait on it later.
    drop(spawn_input_forwarder(
        std::io::stdin(),
        session.writer_sender(),
        stdin_eof_tx,
    ));
    let (stdout_forwarder, stdout_forwarder_done_rx) =
        spawn_output_forwarder(tokio_runtime.clone(), spawned.stdout_rx, std::io::stdout());
    drop(stdout_forwarder);
    let (stderr_forwarder, stderr_forwarder_done_rx) =
        spawn_output_forwarder(tokio_runtime.clone(), spawned.stderr_rx, std::io::stderr());
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
    let _ = tokio::time::timeout(output_drain_timeout, async {
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
    const STDIN_FORWARD_CHUNK_SIZE: usize = 8 * 1024;
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
    // The sandbox session emits output on Tokio channels, but writing to the
    // caller's stdio is simplest from a dedicated blocking thread.
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
#[path = "stdio_bridge_tests.rs"]
mod tests;
