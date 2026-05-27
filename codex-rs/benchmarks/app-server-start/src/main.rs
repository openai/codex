use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::process::Child;
use std::process::ChildStdin;
use std::process::ChildStdout;
use std::process::Command;
use std::process::Stdio;

use anyhow::Context;
use anyhow::Result;
use anyhow::bail;
use codex_app_server_protocol::ClientInfo;
use codex_app_server_protocol::ClientRequest;
use codex_app_server_protocol::InitializeCapabilities;
use codex_app_server_protocol::InitializeParams;
use codex_app_server_protocol::InitializeResponse;
use codex_app_server_protocol::JSONRPCMessage;
use codex_app_server_protocol::JSONRPCNotification;
use codex_app_server_protocol::JSONRPCResponse;
use codex_app_server_protocol::RequestId;
use divan::Bencher;

fn main() {
    divan::main();
}

// Process startup is slow enough that 30 samples keep runs practical, and
// sample size 1 lets each measured server be reaped before the next starts.
// The benchmark warms one CODEX_HOME before timing to measure normal restarts.
// This e2e runner receives its separately built Codex binary from just or Bazel.
#[divan::bench(sample_count = 30, sample_size = 1, skip_ext_time)]
#[allow(clippy::expect_used)]
fn initialize_response(bencher: Bencher) {
    let codex_bin = std::env::var_os("CODEX_BIN")
        .map(PathBuf::from)
        .expect("CODEX_BIN must point to the codex binary; run via `just bench-e2e` or Bazel");
    let codex_home = tempfile::tempdir().expect("benchmark CODEX_HOME should be created");
    drop(
        start_until_initialize_response(&codex_bin, codex_home.path())
            .expect("benchmark CODEX_HOME should be initialized"),
    );

    bencher.bench_local(|| {
        start_until_initialize_response(&codex_bin, codex_home.path())
            .expect("codex app-server should return an initialize response")
    });
}

/// A running app-server that has returned a valid `initialize` response.
///
/// Divan drops benchmark outputs after it stops each timer interval. Returning
/// this value from the measured closure keeps the `initialized` notification
/// and forced process reaping outside startup latency.
struct InitializedAppServer {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: Option<BufReader<ChildStdout>>,
    acknowledge_on_drop: bool,
}

/// Spawn a stdio app-server and return once it responds successfully to `initialize`.
fn start_until_initialize_response(
    codex_bin: &Path,
    codex_home: &Path,
) -> Result<InitializedAppServer> {
    let request_id = RequestId::Integer(0);
    let child = Command::new(codex_bin)
        .arg("app-server")
        .arg("--listen")
        .arg("stdio://")
        .env("CODEX_HOME", codex_home)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .with_context(|| format!("failed to spawn `{}` app-server", codex_bin.display()))?;
    let mut server = InitializedAppServer {
        child,
        stdin: None,
        stdout: None,
        acknowledge_on_drop: false,
    };
    server.stdin = Some(
        server
            .child
            .stdin
            .take()
            .context("app-server stdin unavailable")?,
    );
    server.stdout = Some(BufReader::new(
        server
            .child
            .stdout
            .take()
            .context("app-server stdout unavailable")?,
    ));

    let request = ClientRequest::Initialize {
        request_id: request_id.clone(),
        params: InitializeParams {
            client_info: ClientInfo {
                name: "codex-app-server-start-bench".to_string(),
                title: Some("Codex App Server Start Benchmark".to_string()),
                version: env!("CARGO_PKG_VERSION").to_string(),
            },
            capabilities: Some(InitializeCapabilities {
                experimental_api: false,
                request_attestation: false,
                opt_out_notification_methods: None,
            }),
        },
    };
    let stdin = server
        .stdin
        .as_mut()
        .context("app-server stdin unavailable")?;
    writeln!(stdin, "{}", serde_json::to_string(&request)?)?;
    stdin
        .flush()
        .context("failed to flush initialize request")?;

    let mut line = String::new();
    loop {
        line.clear();
        if server
            .stdout
            .as_mut()
            .context("app-server stdout unavailable")?
            .read_line(&mut line)?
            == 0
        {
            bail!("app-server closed stdout before returning initialize response");
        }

        match serde_json::from_str::<JSONRPCMessage>(line.trim())? {
            JSONRPCMessage::Response(JSONRPCResponse { id, result }) if id == request_id => {
                let _: InitializeResponse = serde_json::from_value(result)
                    .context("initialize response missing expected payload")?;
                server.acknowledge_on_drop = true;
                return Ok(server);
            }
            JSONRPCMessage::Error(error) if error.id == request_id => {
                bail!("initialize failed: {error:?}");
            }
            JSONRPCMessage::Request(_)
            | JSONRPCMessage::Response(_)
            | JSONRPCMessage::Notification(_)
            | JSONRPCMessage::Error(_) => {}
        }
    }
}

impl Drop for InitializedAppServer {
    fn drop(&mut self) {
        if self.acknowledge_on_drop
            && let Some(stdin) = self.stdin.as_mut()
        {
            let initialized = JSONRPCMessage::Notification(JSONRPCNotification {
                method: "initialized".to_string(),
                params: None,
            });
            if let Ok(payload) = serde_json::to_string(&initialized) {
                let _ = writeln!(stdin, "{payload}");
                let _ = stdin.flush();
            }
        }
        let _ = self.stdin.take();
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
