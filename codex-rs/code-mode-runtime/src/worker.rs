use std::collections::HashMap;
use std::io::BufRead;
use std::io::Write;
use std::path::Path;
use std::process::Child;
use std::process::ChildStdin;
use std::process::Stdio;
use std::sync::Arc;
use std::sync::Mutex;
use std::sync::mpsc as std_mpsc;
use std::thread;
use std::time::Duration;

use serde::Deserialize;
use serde::Serialize;
use serde_json::Value as JsonValue;
use tokio::sync::mpsc;
use tracing::warn;

use crate::ExecuteRequest;
use crate::runtime::PendingRuntimeMode;
use crate::runtime::RuntimeCommand;
use crate::runtime::RuntimeControlCommand;
use crate::runtime::RuntimeEvent;
use crate::runtime::RuntimeOptions;
use crate::runtime::spawn_runtime_with_options;

pub const CODEX_CODE_MODE_WORKER_ARG1: &str = "--codex-run-as-code-mode-worker";
pub const CODEX_CODE_MODE_V8_MAX_HEAP_BYTES_ENV_VAR: &str = "CODEX_CODE_MODE_V8_MAX_HEAP_BYTES";

#[derive(Deserialize, Serialize)]
struct WorkerStart {
    stored_values: HashMap<String, JsonValue>,
    request: ExecuteRequest,
    pending_mode: PendingRuntimeMode,
    options: RuntimeOptions,
}

#[derive(Deserialize, Serialize)]
enum WorkerCommand {
    Runtime(RuntimeCommand),
    Control(RuntimeControlCommand),
}

pub(crate) struct SubprocessRuntimeHandle {
    child: Arc<Mutex<Child>>,
}

struct ChildKillGuard(Option<Child>);

impl Drop for ChildKillGuard {
    fn drop(&mut self) {
        if let Some(mut child) = self.0.take() {
            let _ = child.kill();
        }
    }
}

impl SubprocessRuntimeHandle {
    pub(crate) fn terminate_execution(&self) -> bool {
        let Ok(mut child) = self.child.lock() else {
            return false;
        };
        child.kill().is_ok()
    }
}

impl Drop for SubprocessRuntimeHandle {
    fn drop(&mut self) {
        self.terminate_execution();
    }
}

pub(crate) fn spawn_subprocess_runtime(
    executable: &Path,
    stored_values: HashMap<String, JsonValue>,
    request: ExecuteRequest,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    pending_mode: PendingRuntimeMode,
) -> Result<
    (
        std_mpsc::Sender<RuntimeCommand>,
        std_mpsc::Sender<RuntimeControlCommand>,
        SubprocessRuntimeHandle,
    ),
    String,
> {
    let child = std::process::Command::new(executable)
        .arg(CODEX_CODE_MODE_WORKER_ARG1)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|err| format!("failed to spawn code mode runtime worker: {err}"))?;
    let mut child_guard = ChildKillGuard(Some(child));
    let child = child_guard
        .0
        .as_mut()
        .expect("child exists until guard is disarmed");
    let stdin = child
        .stdin
        .take()
        .ok_or_else(|| "code mode runtime worker has no stdin".to_string())?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| "code mode runtime worker has no stdout".to_string())?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| "code mode runtime worker has no stderr".to_string())?;
    let stdin = Arc::new(Mutex::new(stdin));

    write_worker_message(
        &stdin,
        &WorkerStart {
            stored_values,
            request,
            pending_mode,
            options: runtime_options_from_env(),
        },
    )?;

    let child = child_guard
        .0
        .take()
        .expect("child exists until guard is disarmed");
    let (runtime_tx, runtime_rx) = std_mpsc::channel();
    let (control_tx, control_rx) = std_mpsc::channel();
    let child = Arc::new(Mutex::new(child));
    spawn_command_writer(
        Arc::clone(&stdin),
        runtime_rx,
        WorkerCommand::Runtime,
        Arc::clone(&child),
    );
    spawn_command_writer(
        stdin,
        control_rx,
        WorkerCommand::Control,
        Arc::clone(&child),
    );
    spawn_event_reader(stdout, event_tx, Arc::clone(&child));
    spawn_stderr_reader(stderr);
    spawn_child_reaper(Arc::clone(&child));

    Ok((runtime_tx, control_tx, SubprocessRuntimeHandle { child }))
}

fn runtime_options_from_env() -> RuntimeOptions {
    let max_heap_size_bytes = std::env::var(CODEX_CODE_MODE_V8_MAX_HEAP_BYTES_ENV_VAR)
        .ok()
        .and_then(|value| match value.parse::<usize>() {
            Ok(value) => Some(value),
            Err(err) => {
                warn!("ignoring invalid {CODEX_CODE_MODE_V8_MAX_HEAP_BYTES_ENV_VAR} value: {err}");
                None
            }
        });
    RuntimeOptions {
        max_heap_size_bytes,
    }
}

fn spawn_command_writer<T>(
    stdin: Arc<Mutex<ChildStdin>>,
    receiver: std_mpsc::Receiver<T>,
    wrap: fn(T) -> WorkerCommand,
    child: Arc<Mutex<Child>>,
) where
    T: Send + 'static,
{
    thread::spawn(move || {
        for message in receiver {
            if let Err(err) = write_worker_message(&stdin, &wrap(message)) {
                warn!("failed to write to code mode runtime worker: {err}");
                kill_child(&child);
                break;
            }
        }
    });
}

fn spawn_event_reader(
    stdout: std::process::ChildStdout,
    event_tx: mpsc::UnboundedSender<RuntimeEvent>,
    child: Arc<Mutex<Child>>,
) {
    thread::spawn(move || {
        for line in std::io::BufReader::new(stdout).lines() {
            let event = match line {
                Ok(line) => match serde_json::from_str::<RuntimeEvent>(&line) {
                    Ok(event) => event,
                    Err(err) => {
                        warn!("invalid event from code mode runtime worker: {err}");
                        kill_child(&child);
                        break;
                    }
                },
                Err(err) => {
                    warn!("failed to read from code mode runtime worker: {err}");
                    kill_child(&child);
                    break;
                }
            };
            if event_tx.send(event).is_err() {
                kill_child(&child);
                break;
            }
        }
    });
}

fn spawn_stderr_reader(stderr: std::process::ChildStderr) {
    thread::spawn(move || {
        for line in std::io::BufReader::new(stderr).lines() {
            match line {
                Ok(line) => warn!("code mode runtime worker stderr: {line}"),
                Err(err) => {
                    warn!("failed to read code mode runtime worker stderr: {err}");
                    break;
                }
            }
        }
    });
}

fn spawn_child_reaper(child: Arc<Mutex<Child>>) {
    thread::spawn(move || {
        loop {
            let exited = match child.lock() {
                Ok(mut child) => match child.try_wait() {
                    Ok(Some(_)) => true,
                    Ok(None) => false,
                    Err(err) => {
                        warn!("failed to wait for code mode runtime worker: {err}");
                        true
                    }
                },
                Err(_) => true,
            };
            if exited {
                break;
            }
            thread::sleep(Duration::from_millis(50));
        }
    });
}

fn kill_child(child: &Arc<Mutex<Child>>) {
    if let Ok(mut child) = child.lock() {
        let _ = child.kill();
    }
}

fn write_worker_message<T>(stdin: &Arc<Mutex<ChildStdin>>, message: &T) -> Result<(), String>
where
    T: Serialize,
{
    let mut stdin = stdin
        .lock()
        .map_err(|_| "code mode runtime worker stdin lock poisoned".to_string())?;
    serde_json::to_writer(&mut *stdin, message)
        .map_err(|err| format!("failed to serialize code mode runtime worker message: {err}"))?;
    stdin
        .write_all(b"\n")
        .and_then(|()| stdin.flush())
        .map_err(|err| format!("failed to write code mode runtime worker message: {err}"))
}

pub fn run_worker_main() -> ! {
    let exit_code = match run_worker() {
        Ok(()) => 0,
        Err(err) => {
            eprintln!("code mode runtime worker failed: {err}");
            1
        }
    };
    std::process::exit(exit_code);
}

fn run_worker() -> Result<(), String> {
    let mut stdin = std::io::BufReader::new(std::io::stdin());
    let mut start = String::new();
    let bytes_read = stdin
        .read_line(&mut start)
        .map_err(|err| format!("failed to read code mode runtime worker start message: {err}"))?;
    if bytes_read == 0 {
        return Err("missing code mode runtime worker start message".to_string());
    }
    let start: WorkerStart = serde_json::from_str(&start)
        .map_err(|err| format!("invalid code mode runtime worker start message: {err}"))?;
    let (event_tx, mut event_rx) = mpsc::unbounded_channel();
    let (runtime_tx, control_tx, _terminate_handle) = spawn_runtime_with_options(
        start.stored_values,
        start.request,
        event_tx,
        start.pending_mode,
        start.options,
    )?;

    thread::spawn(move || {
        for line in stdin.lines() {
            let command = match line {
                Ok(line) => match serde_json::from_str::<WorkerCommand>(&line) {
                    Ok(command) => command,
                    Err(_) => break,
                },
                Err(_) => break,
            };
            match command {
                WorkerCommand::Runtime(command) => {
                    if runtime_tx.send(command).is_err() {
                        return;
                    }
                }
                WorkerCommand::Control(command) => {
                    if control_tx.send(command).is_err() {
                        return;
                    }
                }
            }
        }
        let _ = runtime_tx.send(RuntimeCommand::Terminate);
        let _ = control_tx.send(RuntimeControlCommand::Terminate);
    });

    let stdout = std::io::stdout();
    let mut stdout = stdout.lock();
    while let Some(event) = event_rx.blocking_recv() {
        serde_json::to_writer(&mut stdout, &event)
            .map_err(|err| format!("failed to serialize code mode runtime event: {err}"))?;
        stdout
            .write_all(b"\n")
            .and_then(|()| stdout.flush())
            .map_err(|err| format!("failed to write code mode runtime event: {err}"))?;
    }
    Ok(())
}
