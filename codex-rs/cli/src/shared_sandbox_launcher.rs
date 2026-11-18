//! Private stdio supervisor for helpers that would otherwise each retain a `codex sandbox` process.
//!
//! Each launch goes through the ordinary host sandbox configuration and gets its own proxy. The caller
//! must start the supervisor with the same environment that it would give `codex sandbox`.

use std::collections::HashMap;
use std::ops::ControlFlow;
use std::os::fd::AsFd;
use std::os::fd::AsRawFd;
use std::os::fd::OwnedFd;
use std::os::unix::fs::FileTypeExt;
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64;
use codex_config::LoaderOverrides;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_cli::CliConfigOverrides;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;
use serde_json::json;
use tokio::io::AsyncBufReadExt;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWrite;
use tokio::io::AsyncWriteExt;
use tokio::io::BufReader;
use tokio::io::Interest;
use tokio::io::unix::AsyncFd;
use tokio::sync::Semaphore;
use tokio::sync::mpsc;
use tokio::sync::watch;

#[cfg(target_os = "linux")]
use crate::LinuxSandboxCommand as HostSandboxCommand;
use crate::SandboxStateArgs;
#[cfg(target_os = "macos")]
use crate::SeatbeltCommand as HostSandboxCommand;
#[cfg(target_os = "linux")]
use crate::debug_sandbox::start_command_under_linux_sandbox as start_command_under_host_sandbox;
#[cfg(target_os = "macos")]
use crate::debug_sandbox::start_command_under_seatbelt as start_command_under_host_sandbox;

const MAX_MESSAGE_BYTES: usize = 16 * 1024 * 1024;
const OUTPUT_WINDOW: usize = 64;

#[derive(Deserialize)]
#[serde(tag = "method", content = "params", rename_all = "camelCase")]
enum Request {
    Start(Start),
    Write { id: String, data: String },
    CloseStdin { id: String },
    Terminate { id: String },
    OutputConsumed { id: String },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Start {
    id: String,
    cwd: PathBuf,
    command: Vec<String>,
    #[serde(default)]
    config_overrides: Vec<String>,
    #[serde(default)]
    allow_unix_sockets: Vec<AbsolutePathBuf>,
}

struct Process {
    stdin: mpsc::Sender<Option<Vec<u8>>>,
    stop: mpsc::Sender<()>,
    output: Arc<Semaphore>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ProcessExitStatus {
    Exit { code: u32 },
    Signal { signal: i32, core_dumped: bool },
}

struct Finished {
    id: String,
    status: Result<ProcessExitStatus, String>,
}

/// Run a sandbox launcher on private stdin/stdout until the controlling connection closes.
pub async fn run(codex_linux_sandbox_exe: Option<PathBuf>) -> anyhow::Result<()> {
    let (out_tx, mut out_rx) = mpsc::channel::<Value>(32);
    let stdin = controller_stdin()?;
    let disconnect = AsyncFd::with_interest(
        std::io::stdin().as_fd().try_clone_to_owned()?,
        Interest::READABLE,
    )?;
    let mut stdout = controller_stdout()?;
    let writer = tokio::spawn(async move {
        while let Some(value) = out_rx.recv().await {
            let mut bytes = serde_json::to_vec(&value)?;
            bytes.push(b'\n');
            stdout.write_all(&bytes).await?;
            stdout.flush().await?;
        }
        Ok::<_, anyhow::Error>(())
    });
    let (done_tx, mut done_rx) = mpsc::unbounded_channel::<Finished>();
    let mut processes = HashMap::<String, Process>::new();
    let mut input = BufReader::new(stdin).lines();
    send(
        &out_tx,
        json!({"method":"ready","params":{"protocolVersion":4,"outputWindow":OUTPUT_WINDOW,"stdinWindow":1}}),
    )
    .await;
    let result = loop {
        let dispatch = async {
            tokio::select! {
                line = input.next_line() => {
                    let Some(line) = line? else {
                        return Ok(ControlFlow::Break(()));
                    };
                    anyhow::ensure!(line.len() <= MAX_MESSAGE_BYTES, "sandbox launcher message too large");
                    let request: Request = serde_json::from_str(&line).context("invalid sandbox launcher request")?;
                    match request {
                        Request::Start(start) => {
                            let id = start.id.clone();
                            if id.is_empty() || id.len() > 256 || processes.contains_key(&id) {
                                send(&out_tx, json!({"method":"error","params":{"id":id,"message":"invalid or duplicate process id"}})).await;
                                return Ok(ControlFlow::Continue(()));
                            }
                            match start_process(start, &out_tx, &done_tx, codex_linux_sandbox_exe.clone()).await {
                                Ok(process) => { processes.insert(id, process); }
                                Err(error) => send(&out_tx, json!({"method":"error","params":{"id":id,"message":error.to_string()}})).await,
                            }
                        }
                        Request::Write { id, data } => {
                            if let Some(process) = processes.get(&id) {
                                let data = BASE64.decode(data).context("invalid stdin data")?;
                                if process.stdin.try_send(Some(data)).is_err() {
                                    send(&out_tx, json!({"method":"error","params":{"id":id,"message":"helper stdin window exceeded or closed"}})).await;
                                }
                            }
                        }
                        Request::CloseStdin { id } => {
                            if let Some(process) = processes.get(&id) && process.stdin.try_send(None).is_err() {
                                send(&out_tx, json!({"method":"error","params":{"id":id,"message":"helper stdin window exceeded or closed"}})).await;
                            }
                        }
                        Request::Terminate { id } => {
                            if let Some(process) = processes.get(&id) { process.output.close(); let _ = process.stop.try_send(()); }
                        }
                        Request::OutputConsumed { id } => {
                            if let Some(process) = processes.get(&id) && process.output.available_permits() < OUTPUT_WINDOW { process.output.add_permits(1); }
                        }
                    }
                }
                Some(finished) = done_rx.recv() => {
                    processes.remove(&finished.id);
                    match finished.status {
                        Ok(status) => send(&out_tx, json!({"method":"exited","params":{"id":finished.id,"status":status}})).await,
                        Err(message) => send(&out_tx, json!({"method":"error","params":{"id":finished.id,"message":message}})).await,
                    }
                }
            }
            Ok::<_, anyhow::Error>(ControlFlow::Continue(()))
        };
        tokio::select! {
            result = controller_eof(&disconnect) => break result.map_err(Into::into),
            result = dispatch => match result {
                Ok(ControlFlow::Continue(())) => {},
                Ok(ControlFlow::Break(())) => break Ok(()),
                Err(error) => break Err(error),
            },
        }
    };
    // Losing the private controller must not leave helpers or their proxies resident.
    writer.abort();
    for process in processes.values() {
        process.output.close();
        let _ = process.stop.try_send(());
    }
    while !processes.is_empty() {
        if let Some(finished) = done_rx.recv().await {
            processes.remove(&finished.id);
        } else {
            break;
        }
    }
    drop(out_tx);
    let _ = writer.await;
    result
}

/// Observe a controller hangup even when unread requests remain in stdin.
async fn controller_eof(fd: &AsyncFd<OwnedFd>) -> std::io::Result<()> {
    loop {
        let mut event = fd.readable().await?;
        if event.ready().is_read_closed() {
            return Ok(());
        }
        event.clear_ready();
    }
}

/// Read controller stdin without leaving an uncancellable blocking read during shutdown.
fn controller_stdin() -> std::io::Result<Box<dyn AsyncRead + Unpin + Send>> {
    let fd = std::io::stdin().as_fd().try_clone_to_owned()?;
    let file = std::fs::File::from(fd);
    let ty = file.metadata()?.file_type();
    if ty.is_fifo() {
        Ok(Box::new(tokio::net::unix::pipe::Receiver::from_owned_fd(
            file.into(),
        )?))
    } else if ty.is_socket() {
        let socket = std::os::unix::net::UnixStream::from(std::os::fd::OwnedFd::from(file));
        socket.set_nonblocking(true)?;
        Ok(Box::new(tokio::net::UnixStream::from_std(socket)?))
    } else {
        Err(std::io::Error::other(
            "sandbox launcher stdin must be a pipe or Unix socket",
        ))
    }
}

/// Use nonblocking controller stdout so disconnect can cancel a stalled write on pipes or sockets.
fn controller_stdout() -> std::io::Result<Box<dyn AsyncWrite + Unpin + Send>> {
    let fd = std::io::stdout().as_fd().try_clone_to_owned()?;
    let file = std::fs::File::from(fd);
    let ty = file.metadata()?.file_type();
    if ty.is_fifo() {
        Ok(Box::new(tokio::net::unix::pipe::Sender::from_owned_fd(
            file.into(),
        )?))
    } else if ty.is_socket() {
        let socket = std::os::unix::net::UnixStream::from(std::os::fd::OwnedFd::from(file));
        socket.set_nonblocking(true)?;
        Ok(Box::new(tokio::net::UnixStream::from_std(socket)?))
    } else {
        Err(std::io::Error::other(
            "sandbox launcher stdout must be a pipe or Unix socket",
        ))
    }
}

/// Launch one helper with the same config, sandbox transform and proxy as `codex sandbox`.
async fn start_process(
    start: Start,
    output: &mpsc::Sender<Value>,
    done: &mpsc::UnboundedSender<Finished>,
    codex_linux_sandbox_exe: Option<PathBuf>,
) -> anyhow::Result<Process> {
    anyhow::ensure!(start.cwd.is_absolute(), "helper cwd must be absolute");
    let cwd = std::fs::canonicalize(&start.cwd).context("resolve helper cwd")?;
    anyhow::ensure!(
        !start.command.is_empty(),
        "helper command must not be empty"
    );
    #[cfg(target_os = "linux")]
    anyhow::ensure!(
        start.allow_unix_sockets.is_empty(),
        "Unix socket allowances are only supported on macOS"
    );
    let command = HostSandboxCommand {
        #[cfg(target_os = "linux")]
        transparent_tcp: false,
        sandbox_state: SandboxStateArgs::default(),
        permissions_profile: None,
        config_profile: None,
        cwd: Some(cwd),
        include_managed_config: false,
        #[cfg(target_os = "macos")]
        allow_unix_sockets: start.allow_unix_sockets,
        #[cfg(target_os = "macos")]
        log_denials: false,
        config_overrides: CliConfigOverrides {
            raw_overrides: start.config_overrides,
        },
        command: start.command,
    };
    let mut launch = start_command_under_host_sandbox(
        command,
        codex_linux_sandbox_exe,
        LoaderOverrides::default(),
        /*piped*/ true,
    )
    .await?;
    let pid = launch
        .child_mut()
        .id()
        .context("helper has no process id")?;
    let mut child_stdin = launch
        .child_mut()
        .stdin
        .take()
        .context("helper stdin missing")?;
    let (stdin, mut input) = mpsc::channel::<Option<Vec<u8>>>(1);
    let input_output = output.clone();
    let input_id = start.id.clone();
    let stdin_task = tokio::spawn(async move {
        while let Some(Some(data)) = input.recv().await {
            if child_stdin.write_all(&data).await.is_err() {
                send(
                    &input_output,
                    json!({"method":"stdinClosed","params":{"id":input_id}}),
                )
                .await;
                break;
            }
            send(
                &input_output,
                json!({"method":"stdinConsumed","params":{"id":input_id}}),
            )
            .await;
        }
    });
    let stdout = launch
        .child_mut()
        .stdout
        .take()
        .context("helper stdout missing")?;
    let stderr = launch
        .child_mut()
        .stderr
        .take()
        .context("helper stderr missing")?;
    let id = start.id;
    send(
        output,
        json!({"method":"started","params":{"id":id,"pid":pid}}),
    )
    .await;
    let (exit_tx, exit_rx) = watch::channel(false);
    let credits = Arc::new(Semaphore::new(OUTPUT_WINDOW));
    let stdout_task = tokio::spawn(copy_output(
        stdout,
        id.clone(),
        "stdout",
        output.clone(),
        Arc::clone(&credits),
        exit_rx.clone(),
    ));
    let stderr_task = tokio::spawn(copy_output(
        stderr,
        id.clone(),
        "stderr",
        output.clone(),
        Arc::clone(&credits),
        exit_rx,
    ));
    let (stop_tx, mut stop_rx) = mpsc::channel(1);
    let done = done.clone();
    tokio::spawn(async move {
        let status = tokio::select! {
            status = launch.child_mut().wait() => status,
            _ = stop_rx.recv() => {
                let _ = launch.child_mut().start_kill();
                launch.child_mut().wait().await
            }
        };
        stdin_task.abort();
        let _ = stdin_task.await;
        // The ordinary CLI drops its proxy when this process exits, even if a descendant kept a
        // pipe open. Retain that lifetime instead of tying the proxy to output-stream closure.
        drop(launch);
        exit_tx.send_replace(true);
        let (stdout, stderr) = tokio::join!(stdout_task, stderr_task);
        let error = [stdout, stderr]
            .into_iter()
            .find_map(|result| match result {
                Ok(Ok(())) => None,
                Ok(Err(error)) => Some(error.to_string()),
                Err(error) => Some(error.to_string()),
            });
        let status = match (error, status) {
            (Some(error), _) => Err(error),
            (None, Err(error)) => Err(error.to_string()),
            (None, Ok(status)) => match status.code() {
                Some(code) => Ok(ProcessExitStatus::Exit { code: code as u32 }),
                None => status
                    .signal()
                    .map(|signal| ProcessExitStatus::Signal {
                        signal,
                        core_dumped: status.core_dumped(),
                    })
                    .ok_or_else(|| "unknown helper exit status".to_string()),
            },
        };
        let _ = done.send(Finished { id, status });
    });
    Ok(Process {
        stdin,
        stop: stop_tx,
        output: credits,
    })
}

/// Forward one child stream without letting it access the supervisor's private control stream.
async fn copy_output(
    mut stream: impl AsyncRead + AsRawFd + Unpin,
    id: String,
    name: &'static str,
    output: mpsc::Sender<Value>,
    credits: Arc<Semaphore>,
    mut exited: watch::Receiver<bool>,
) -> anyhow::Result<()> {
    let mut buffer = vec![0; 16 * 1024];
    let mut remaining = None;
    loop {
        // Snapshot the bytes already buffered when we observe direct-child exit. A descendant
        // may keep writing, but it cannot make this finite drain budget grow.
        if remaining.is_none() && *exited.borrow() {
            remaining = Some(buffered_bytes(stream.as_raw_fd())?);
        }
        let capacity = remaining.unwrap_or(buffer.len()).min(buffer.len());
        if capacity == 0 {
            break;
        }
        let read = if remaining.is_some() {
            stream.read(&mut buffer[..capacity]).await
        } else {
            tokio::select! {
                result = stream.read(&mut buffer[..capacity]) => result,
                _ = exited.changed() => continue,
            }
        };
        let count = read?;
        if count == 0 {
            break;
        }
        if let Some(remaining) = remaining.as_mut() {
            *remaining -= count;
        }
        let Ok(permit) = credits.acquire().await else {
            break;
        };
        if output.send(json!({"method":"output","params":{"id":id,"stream":name,"data":BASE64.encode(&buffer[..count])}})).await.is_err() { break; }
        permit.forget();
    }
    Ok(())
}

/// Return the bytes currently buffered on a helper pipe without waiting for descendants.
fn buffered_bytes(fd: std::os::fd::RawFd) -> std::io::Result<usize> {
    let mut bytes: libc::c_int = 0;
    // SAFETY: `fd` belongs to the live stream and FIONREAD writes one integer to `bytes`.
    if unsafe { libc::ioctl(fd, libc::FIONREAD, &mut bytes) } < 0 {
        return Err(std::io::Error::last_os_error());
    }
    usize::try_from(bytes).map_err(std::io::Error::other)
}

/// Queue a protocol event; a closed controller will stop the supervisor on stdin EOF.
async fn send(output: &mpsc::Sender<Value>, event: Value) {
    let _ = output.send(event).await;
}
