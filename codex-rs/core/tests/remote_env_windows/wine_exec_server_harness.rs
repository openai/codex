use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;
use std::time::Instant;

use anyhow::Context;
use anyhow::Result;
use codex_utils_pty::ProcessHandle;
use codex_utils_pty::SpawnedProcess;
use codex_utils_pty::TerminalSize;
use tempfile::TempDir;
use tokio::process::Command;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio::time::timeout;

const START_TIMEOUT: Duration = Duration::from_secs(30);
const BLOCKING_CLEANUP_TIMEOUT: Duration = Duration::from_secs(5);

pub(crate) const POWERSHELL_PATH: &str = r"C:\Program Files\PowerShell\7\pwsh.exe";
pub(crate) const POWERSHELL_VERSION: &str = "7.2.24";
pub(crate) const WINDOWS_WORKSPACE: &str = r"C:\workspace";

const POWERSHELL_DIRECTORY: &str = "drive_c/Program Files/PowerShell/7";

pub(crate) struct WineExecServer {
    processes: Option<WineProcesses>,
}

struct WineProcesses {
    cleanup_complete: bool,
    exit_rx: Option<oneshot::Receiver<i32>>,
    prefix: TempDir,
    process: ProcessHandle,
    stderr_task: JoinHandle<()>,
    stdout_task: JoinHandle<()>,
    wineserver: PathBuf,
}

impl WineExecServer {
    pub(crate) async fn start() -> Result<(Self, String)> {
        let prefix = TempDir::new().context("create Wine prefix")?;
        wine_test_support::install_pinned_powershell_runtime(prefix.path())?;
        install_powershell_exe_alias(prefix.path())?;

        let executable = codex_utils_cargo_bin::cargo_bin("wine-windows-exec-server")?;
        let wine = codex_utils_cargo_bin::cargo_bin("wine")?;
        let wine_runtime_marker = codex_utils_cargo_bin::cargo_bin("wine-runtime-marker")?;
        let wine_dll_path = wine_runtime_marker
            .parent()
            .context("locate Wine runtime directory")?;
        let wineserver = codex_utils_cargo_bin::cargo_bin("wineserver")?;
        let mut env = std::env::vars().collect::<HashMap<_, _>>();
        env.remove("DISPLAY");
        env.extend([
            ("HOME".to_string(), prefix.path().to_string_lossy().into_owned()),
            (
                "XDG_RUNTIME_DIR".to_string(),
                prefix.path().to_string_lossy().into_owned(),
            ),
            ("WINEARCH".to_string(), "win64".to_string()),
            (
                "WINEPREFIX".to_string(),
                prefix.path().to_string_lossy().into_owned(),
            ),
            (
                "WINEDLLPATH".to_string(),
                wine_dll_path.to_string_lossy().into_owned(),
            ),
            (
                "WINESERVER".to_string(),
                wineserver.to_string_lossy().into_owned(),
            ),
            ("WINEDEBUG".to_string(), "-all".to_string()),
            (
                "WINEDLLOVERRIDES".to_string(),
                "mscoree,mshtml,winegstreamer=".to_string(),
            ),
            ("LANG".to_string(), "C.UTF-8".to_string()),
            ("LC_ALL".to_string(), "C.UTF-8".to_string()),
            ("LC_CTYPE".to_string(), "C.UTF-8".to_string()),
            ("TEMP".to_string(), r"C:\windows\temp".to_string()),
            ("TMP".to_string(), r"C:\windows\temp".to_string()),
            ("CODEX_HOME".to_string(), r"C:\codex-home".to_string()),
            (
                "WINEPATH".to_string(),
                r"C:\Program Files\PowerShell\7".to_string(),
            ),
            ("DOTNET_CLI_TELEMETRY_OPTOUT".to_string(), "1".to_string()),
            ("DOTNET_NOLOGO".to_string(), "1".to_string()),
            (
                "POWERSHELL_TELEMETRY_OPTOUT".to_string(),
                "1".to_string(),
            ),
            ("POWERSHELL_UPDATECHECK".to_string(), "Off".to_string()),
        ]);
        let wine = wine.to_string_lossy().into_owned();
        let executable = executable.to_string_lossy().into_owned();
        let SpawnedProcess {
            session: process,
            mut stdout_rx,
            mut stderr_rx,
            exit_rx,
        } = codex_utils_pty::spawn_pty_process(
            &wine,
            &[executable],
            prefix.path(),
            &env,
            /*arg0*/ &None,
            TerminalSize::default(),
        )
        .await
        .context("start Windows exec-server under Wine")?;
        let stderr_task = tokio::spawn(async move {
            while let Some(chunk) = stderr_rx.recv().await {
                let _ = std::io::stderr().lock().write_all(&chunk);
            }
        });
        let websocket_url_result = timeout(START_TIMEOUT, async {
            let mut output = Vec::new();
            loop {
                let chunk = stdout_rx
                    .recv()
                    .await
                    .context("Wine exec-server exited before reporting its URL")?;
                output.extend_from_slice(&chunk);
                if output.len() > 64 * 1024 {
                    output.drain(..output.len() - 64 * 1024);
                }
                let rendered = String::from_utf8_lossy(&output);
                if let Some(start) = rendered.find("ws://") {
                    let url = &rendered[start..];
                    let end = url.find(char::is_whitespace).unwrap_or(url.len());
                    return Ok::<_, anyhow::Error>(url[..end].to_string());
                }
            }
        })
        .await
        .context("Wine exec-server startup timed out")
        .and_then(std::convert::identity);
        let stdout_task = tokio::spawn(async move { while stdout_rx.recv().await.is_some() {} });
        let server = Self {
            processes: Some(WineProcesses {
                cleanup_complete: false,
                exit_rx: Some(exit_rx),
                prefix,
                process,
                stderr_task,
                stdout_task,
                wineserver,
            }),
        };

        match websocket_url_result {
            Ok(websocket_url) => Ok((server, websocket_url)),
            Err(start_error) => {
                if let Err(shutdown_error) = server.shutdown().await {
                    return Err(start_error.context(format!(
                        "Wine cleanup after startup failure also failed: {shutdown_error:#}"
                    )));
                }
                Err(start_error)
            }
        }
    }

    pub(crate) async fn shutdown(mut self) -> Result<()> {
        let result = self
            .processes
            .as_mut()
            .context("Wine process guard is missing")?
            .shutdown()
            .await;
        self.processes.take();
        result
    }
}

fn install_powershell_exe_alias(prefix: &std::path::Path) -> Result<()> {
    let directory = prefix.join(POWERSHELL_DIRECTORY);
    let source = directory.join("pwsh.exe");
    let target = directory.join("powershell.exe");
    if fs::hard_link(&source, &target).is_err() {
        fs::copy(&source, &target).with_context(|| {
            format!(
                "copy PowerShell executable alias from {} to {}",
                source.display(),
                target.display()
            )
        })?;
    }
    Ok(())
}

impl Drop for WineExecServer {
    fn drop(&mut self) {
        if self.processes.is_some() && !std::thread::panicking() {
            panic!("WineExecServer dropped without calling async shutdown");
        }
    }
}

impl WineProcesses {
    async fn shutdown(&mut self) -> Result<()> {
        self.process.request_terminate();
        let wait_result = async {
            let exit_rx = self
                .exit_rx
                .take()
                .context("Wine process exit receiver is missing")?;
            timeout(START_TIMEOUT, exit_rx)
                .await
                .context("wait for Windows exec-server process timed out")?
                .context("wait for Windows exec-server process")?;
            Ok::<_, anyhow::Error>(())
        }
        .await;
        self.stderr_task.abort();
        self.stdout_task.abort();
        let wineserver_result = timeout(START_TIMEOUT, async {
            let status = Command::new(&self.wineserver)
                .args(["-k", "-w"])
                .env("HOME", self.prefix.path())
                .env("WINEPREFIX", self.prefix.path())
                .env("XDG_RUNTIME_DIR", self.prefix.path())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .await
                .context("stop isolated wineserver")?;
            anyhow::ensure!(status.success(), "wineserver exited with {status}");
            Ok::<_, anyhow::Error>(())
        })
        .await
        .context("stop isolated wineserver timed out")
        .and_then(std::convert::identity);

        let result = wait_result.and(wineserver_result);
        if result.is_ok() {
            self.cleanup_complete = true;
        } else {
            self.shutdown_blocking();
        }
        result
    }

    fn shutdown_blocking(&mut self) {
        self.stderr_task.abort();
        self.stdout_task.abort();
        self.process.terminate();
        let Ok(mut child) = std::process::Command::new(&self.wineserver)
            .arg("-k")
            .env("HOME", self.prefix.path())
            .env("WINEPREFIX", self.prefix.path())
            .env("XDG_RUNTIME_DIR", self.prefix.path())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            self.cleanup_complete = true;
            return;
        };
        let deadline = Instant::now() + BLOCKING_CLEANUP_TIMEOUT;
        loop {
            match child.try_wait() {
                Ok(Some(_)) => break,
                Ok(None) if Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Ok(None) | Err(_) => {
                    let _ = child.kill();
                    let _ = child.wait();
                    break;
                }
            }
        }
        self.cleanup_complete = true;
    }
}

impl Drop for WineProcesses {
    fn drop(&mut self) {
        if !self.cleanup_complete && std::thread::panicking() {
            self.shutdown_blocking();
        }
    }
}
