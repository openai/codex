//! The local stderr reader must close when its MCP transport ends, even when
//! a descendant outside the server process group still has stderr open.

#![cfg(unix)]

use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::Result;
use codex_rmcp_client::ElicitationAction;
use codex_rmcp_client::ElicitationResponse;
use codex_rmcp_client::LocalStdioServerLauncher;
use codex_rmcp_client::RmcpClient;
use futures::FutureExt as _;
use pretty_assertions::assert_eq;
use rmcp::model::ClientCapabilities;
use rmcp::model::Implementation;
use rmcp::model::InitializeRequestParams;
use rmcp::model::ProtocolVersion;

struct EscapedChild(i32);

enum Cleanup {
    Shutdown,
    Drop,
}

#[derive(Clone)]
struct TestLogWriter(Arc<Mutex<Vec<u8>>>);

impl Write for TestLogWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .map_err(|_| std::io::Error::other("log buffer lock"))?
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl Drop for EscapedChild {
    fn drop(&mut self) {
        // Only this test's PID, written after setsid(), is eligible for cleanup.
        // The fixture also exits independently after 30 seconds.
        unsafe { libc::kill(self.0, libc::SIGKILL) };
    }
}

fn fd_count() -> Result<usize> {
    let directory = if cfg!(target_os = "linux") {
        "/proc/self/fd"
    } else {
        "/dev/fd"
    };
    Ok(fs::read_dir(directory)?.count())
}

fn read_pid(path: &Path) -> Result<i32> {
    let pid = fs::read_to_string(path)?.trim().parse::<i32>()?;
    anyhow::ensure!(pid > 1, "invalid fixture PID in {}", path.display());
    Ok(pid)
}

#[tokio::test(flavor = "current_thread")]
async fn client_teardown_closes_stderr_fd_and_logs_queued_diagnostics() -> Result<()> {
    let logs = Arc::new(Mutex::new(Vec::new()));
    let writer_logs = Arc::clone(&logs);
    // The reader task runs on this thread, so its logs use this subscriber.
    let _subscriber_guard = tracing::subscriber::set_default(
        tracing_subscriber::fmt()
            .with_ansi(false)
            .with_max_level(tracing::Level::INFO)
            .with_writer(move || TestLogWriter(Arc::clone(&writer_logs)))
            .finish(),
    );
    // FD counts are process-wide, so run the two teardown paths sequentially.
    assert_stderr_reader_closed(Cleanup::Shutdown, &logs).await?;
    assert_stderr_reader_closed(Cleanup::Drop, &logs).await
}

async fn assert_stderr_reader_closed(cleanup: Cleanup, logs: &Arc<Mutex<Vec<u8>>>) -> Result<()> {
    let temporary = tempfile::tempdir()?;
    let server_pid_file = temporary.path().join("server.pid");
    let child_pid_file = temporary.path().join("escaped.pid");
    let trigger_file = temporary.path().join("emit-stderr");
    let written_file = temporary.path().join("stderr-written");
    let diagnostic = match &cleanup {
        Cleanup::Shutdown => "queued diagnostic before shutdown",
        Cleanup::Drop => "queued diagnostic before drop",
    };
    let script = temporary.path().join("server.py");
    fs::write(
        &script,
        r#"import json, os, pathlib, signal, sys, threading, time
# Keep either process bounded if test cleanup fails.
MAX_FIXTURE_LIFETIME_SECONDS = 30
server_pid_path = pathlib.Path(sys.argv[1])
escaped_pid_path = pathlib.Path(sys.argv[2])
trigger_path = pathlib.Path(sys.argv[3])
written_path = pathlib.Path(sys.argv[4])
signal.alarm(MAX_FIXTURE_LIFETIME_SECONDS)
server_pid_path.write_text(str(os.getpid()))
ready_reader, ready_writer = os.pipe()
if os.fork() == 0:  # fork() returns zero in the child.
    os.close(ready_reader)
    os.setsid()
    signal.alarm(MAX_FIXTURE_LIFETIME_SECONDS)
    os.close(sys.stdin.fileno())
    os.close(sys.stdout.fileno())
    escaped_pid_path.write_text(str(os.getpid()))
    os.write(ready_writer, b"1")
    os.close(ready_writer)
    # Exit even if SIGALRM was ignored; the test normally kills this child sooner.
    time.sleep(MAX_FIXTURE_LIFETIME_SECONDS)
    os._exit(os.EX_OK)
os.close(ready_writer)
with os.fdopen(ready_reader, "rb") as ready:
    assert ready.read(1) == b"1"
def emit_diagnostic():
    while not trigger_path.exists():
        time.sleep(0.01)
    print(sys.argv[5], file=sys.stderr, flush=True)
    written_path.write_text("written")
threading.Thread(target=emit_diagnostic, daemon=True).start()
for line in sys.stdin:
    message = json.loads(line)
    if message.get("method") == "initialize":
        result = {"protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "stderr-reader-fixture", "version": "1"}}
        print(json.dumps({"jsonrpc": "2.0", "id": message["id"], "result": result}), flush=True)
"#,
    )?;
    let baseline = fd_count()?;
    let client = RmcpClient::new_stdio_client(
        which::which("python3")?.into(),
        vec![
            script.into_os_string(),
            server_pid_file.clone().into_os_string(),
            child_pid_file.clone().into_os_string(),
            trigger_file.clone().into_os_string(),
            written_file.clone().into_os_string(),
            diagnostic.into(),
        ],
        /*env*/ None,
        &[],
        /*cwd*/ None,
        Arc::new(LocalStdioServerLauncher::new(std::env::current_dir()?)),
    )
    .await?;
    client
        .initialize(
            InitializeRequestParams::new(
                ClientCapabilities::default(),
                Implementation::new("stderr-reader-test", "1"),
            )
            .with_protocol_version(ProtocolVersion::V_2025_06_18),
            Some(Duration::from_secs(5)),
            Box::new(|_, _| {
                async {
                    Ok(ElicitationResponse {
                        action: ElicitationAction::Decline,
                        content: None,
                        meta: None,
                    })
                }
                .boxed()
            }),
        )
        .await?;
    let server_pid = read_pid(&server_pid_file)?;
    let escaped = EscapedChild(read_pid(&child_pid_file)?);
    assert_eq!(unsafe { libc::getpgid(server_pid) }, server_pid);
    assert_eq!(unsafe { libc::getpgid(escaped.0) }, escaped.0);
    // Keep the current-thread Tokio runtime blocked until the server has
    // written stderr. The reader cannot run before teardown starts.
    fs::write(&trigger_file, "")?;
    for _ in 0..250 {
        if written_file.exists() {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(written_file.exists(), "fixture did not write stderr");
    // For Shutdown, keep the client alive until after the FD assertion so
    // final Drop cannot hide missing cleanup in the explicit shutdown path.
    if matches!(cleanup, Cleanup::Shutdown) {
        client.shutdown().await;
    } else {
        drop(client);
    }
    let mut after_shutdown = fd_count()?;
    for _ in 0..250 {
        if after_shutdown == baseline {
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
        after_shutdown = fd_count()?;
    }
    assert_eq!(
        after_shutdown, baseline,
        "stderr reader still owns a file descriptor while the escaped child is alive"
    );
    assert_eq!(unsafe { libc::kill(escaped.0, 0) }, 0);
    let captured_logs = String::from_utf8(
        logs.lock()
            .map_err(|_| anyhow::anyhow!("log buffer lock"))?
            .clone(),
    )?;
    assert!(
        captured_logs.contains(diagnostic),
        "stderr diagnostic written before teardown was lost: {captured_logs}"
    );
    drop(escaped);
    Ok(())
}
