#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use std::collections::HashMap;
use std::io;
use std::path::Path;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Child;
use tokio_util::sync::CancellationToken;

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::get_platform_sandbox;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::ExecEnv;
use crate::sandboxing::SandboxManager;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use crate::text_encoding::bytes_to_string_smart;

pub const DEFAULT_EXEC_COMMAND_TIMEOUT_MS: u64 = 10_000;

// Hardcode these since it does not seem worth including the libc crate just
// for these.
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128; // conventional shell: 128 + signal
const EXEC_TIMEOUT_EXIT_CODE: i32 = 124; // conventional timeout exit code

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192; // bytes per read
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024; // 8 KiB

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub expiration: ExecExpiration,
    pub env: HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

/// Mechanism to terminate an exec invocation before it finishes naturally.
#[derive(Debug)]
pub enum ExecExpiration {
    Timeout(Duration),
    DefaultTimeout,
    Cancellation(CancellationToken),
}

impl From<Option<u64>> for ExecExpiration {
    fn from(timeout_ms: Option<u64>) -> Self {
        timeout_ms.map_or(ExecExpiration::DefaultTimeout, |timeout_ms| {
            ExecExpiration::Timeout(Duration::from_millis(timeout_ms))
        })
    }
}

impl From<u64> for ExecExpiration {
    fn from(timeout_ms: u64) -> Self {
        ExecExpiration::Timeout(Duration::from_millis(timeout_ms))
    }
}

impl ExecExpiration {
    async fn wait(self) {
        match self {
            ExecExpiration::Timeout(duration) => tokio::time::sleep(duration).await,
            ExecExpiration::DefaultTimeout => {
                tokio::time::sleep(Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)).await
            }
            ExecExpiration::Cancellation(cancel) => {
                cancel.cancelled().await;
            }
        }
    }

    /// If ExecExpiration is a timeout, returns the timeout in milliseconds.
    pub(crate) fn timeout_ms(&self) -> Option<u64> {
        match self {
            ExecExpiration::Timeout(duration) => Some(duration.as_millis() as u64),
            ExecExpiration::DefaultTimeout => Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
            ExecExpiration::Cancellation(_) => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SandboxType {
    None,

    /// Only available on macOS.
    MacosSeatbelt,

    /// Only available on Linux.
    LinuxSeccomp,

    /// Only available on Windows.
    WindowsRestrictedToken,
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let sandbox_type = match &sandbox_policy {
        SandboxPolicy::DangerFullAccess => SandboxType::None,
        _ => get_platform_sandbox().unwrap_or(SandboxType::None),
    };
    tracing::debug!("Sandbox type: {sandbox_type:?}");

    let ExecParams {
        command,
        cwd,
        expiration,
        env,
        with_escalated_permissions,
        justification,
        arg0: _,
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;

    let spec = CommandSpec {
        program: program.clone(),
        args: args.to_vec(),
        cwd,
        env,
        expiration,
        with_escalated_permissions,
        justification,
    };

    let manager = SandboxManager::new();
    let exec_env = manager
        .transform(
            spec,
            sandbox_policy,
            sandbox_type,
            sandbox_cwd,
            codex_linux_sandbox_exe.as_ref(),
        )
        .map_err(CodexErr::from)?;

    // Route through the sandboxing module for a single, unified execution path.
    crate::sandboxing::execute_env(exec_env, sandbox_policy, stdout_stream).await
}

pub(crate) async fn execute_exec_env(
    env: ExecEnv,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let ExecEnv {
        command,
        cwd,
        env,
        expiration,
        sandbox,
        with_escalated_permissions,
        justification,
        arg0,
    } = env;

    let params = ExecParams {
        command,
        cwd,
        expiration,
        env,
        with_escalated_permissions,
        justification,
        arg0,
    };

    let start = Instant::now();
    let raw_output_result = exec(params, sandbox, sandbox_policy, stdout_stream).await;
    let duration = start.elapsed();
    finalize_exec_result(raw_output_result, sandbox, duration)
}

#[cfg(target_os = "windows")]
async fn exec_windows_sandbox(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
) -> Result<RawExecToolCallOutput> {
    use crate::config::find_codex_home;
    use codex_windows_sandbox::run_windows_sandbox_capture;

    let ExecParams {
        command,
        cwd,
        env,
        expiration,
        ..
    } = params;
    // TODO(iceweasel-oai): run_windows_sandbox_capture should support all
    // variants of ExecExpiration, not just timeout.
    let timeout_ms = expiration.timeout_ms();

    let policy_str = serde_json::to_string(sandbox_policy).map_err(|err| {
        CodexErr::Io(io::Error::other(format!(
            "failed to serialize Windows sandbox policy: {err}"
        )))
    })?;
    let sandbox_cwd = cwd.clone();
    let codex_home = find_codex_home().map_err(|err| {
        CodexErr::Io(io::Error::other(format!(
            "windows sandbox: failed to resolve codex_home: {err}"
        )))
    })?;
    let spawn_res = tokio::task::spawn_blocking(move || {
        run_windows_sandbox_capture(
            policy_str.as_str(),
            &sandbox_cwd,
            codex_home.as_ref(),
            command,
            &cwd,
            env,
            timeout_ms,
        )
    })
    .await;

    let capture = match spawn_res {
        Ok(Ok(v)) => v,
        Ok(Err(err)) => {
            return Err(CodexErr::Io(io::Error::other(format!(
                "windows sandbox: {err}"
            ))));
        }
        Err(join_err) => {
            return Err(CodexErr::Io(io::Error::other(format!(
                "windows sandbox join error: {join_err}"
            ))));
        }
    };

    let exit_status = synthetic_exit_status(capture.exit_code);
    let stdout = StreamOutput {
        text: capture.stdout,
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: capture.stderr,
        truncated_after_lines: None,
    };
    // Best-effort aggregate: stdout then stderr
    let mut aggregated = Vec::with_capacity(stdout.text.len() + stderr.text.len());
    append_all(&mut aggregated, &stdout.text);
    append_all(&mut aggregated, &stderr.text);
    let aggregated_output = StreamOutput {
        text: aggregated,
        truncated_after_lines: None,
    };

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
        timed_out: capture.timed_out,
    })
}

fn finalize_exec_result(
    raw_output_result: std::result::Result<RawExecToolCallOutput, CodexErr>,
    sandbox_type: SandboxType,
    duration: Duration,
) -> Result<ExecToolCallOutput> {
    match raw_output_result {
        Ok(raw_output) => {
            #[allow(unused_mut)]
            let mut timed_out = raw_output.timed_out;

            #[cfg(target_family = "unix")]
            {
                if let Some(signal) = raw_output.exit_status.signal() {
                    if signal == TIMEOUT_CODE {
                        timed_out = true;
                    } else {
                        return Err(CodexErr::Sandbox(SandboxErr::Signal(signal)));
                    }
                }
            }

            let mut exit_code = raw_output.exit_status.code().unwrap_or(-1);
            if timed_out {
                exit_code = EXEC_TIMEOUT_EXIT_CODE;
            }

            let stdout = raw_output.stdout.from_utf8_lossy();
            let stderr = raw_output.stderr.from_utf8_lossy();
            let aggregated_output = raw_output.aggregated_output.from_utf8_lossy();
            let exec_output = ExecToolCallOutput {
                exit_code,
                stdout,
                stderr,
                aggregated_output,
                duration,
                timed_out,
            };

            if timed_out {
                return Err(CodexErr::Sandbox(SandboxErr::Timeout {
                    output: Box::new(exec_output),
                }));
            }

            if is_likely_sandbox_denied(sandbox_type, &exec_output) {
                return Err(CodexErr::Sandbox(SandboxErr::Denied {
                    output: Box::new(exec_output),
                }));
            }

            Ok(exec_output)
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

pub(crate) mod errors {
    use super::CodexErr;
    use crate::sandboxing::SandboxTransformError;

    impl From<SandboxTransformError> for CodexErr {
        fn from(err: SandboxTransformError) -> Self {
            match err {
                SandboxTransformError::MissingLinuxSandboxExecutable => {
                    CodexErr::LandlockSandboxExecutableNotProvided
                }
                #[cfg(not(target_os = "macos"))]
                SandboxTransformError::SeatbeltUnavailable => CodexErr::UnsupportedOperation(
                    "seatbelt sandbox is only available on macOS".to_string(),
                ),
            }
        }
    }
}

/// We don't have a fully deterministic way to tell if our command failed
/// because of the sandbox - a command in the user's zshrc file might hit an
/// error, but the command itself might fail or succeed for other reasons.
/// For now, we conservatively check for well known command failure exit codes and
/// also look for common sandbox denial keywords in the command output.
pub(crate) fn is_likely_sandbox_denied(
    sandbox_type: SandboxType,
    exec_output: &ExecToolCallOutput,
) -> bool {
    if sandbox_type == SandboxType::None || exec_output.exit_code == 0 {
        return false;
    }

    // Quick rejects: well-known non-sandbox shell exit codes
    // 2: misuse of shell builtins
    // 126: permission denied
    // 127: command not found
    const SANDBOX_DENIED_KEYWORDS: [&str; 7] = [
        "operation not permitted",
        "permission denied",
        "read-only file system",
        "seccomp",
        "sandbox",
        "landlock",
        "failed to write file",
    ];

    let has_sandbox_keyword = [
        &exec_output.stderr.text,
        &exec_output.stdout.text,
        &exec_output.aggregated_output.text,
    ]
    .into_iter()
    .any(|section| {
        let lower = section.to_lowercase();
        SANDBOX_DENIED_KEYWORDS
            .iter()
            .any(|needle| lower.contains(needle))
    });

    if has_sandbox_keyword {
        return true;
    }

    const QUICK_REJECT_EXIT_CODES: [i32; 3] = [2, 126, 127];
    if QUICK_REJECT_EXIT_CODES.contains(&exec_output.exit_code) {
        return false;
    }

    #[cfg(unix)]
    {
        const SIGSYS_CODE: i32 = libc::SIGSYS;
        if sandbox_type == SandboxType::LinuxSeccomp
            && exec_output.exit_code == EXIT_CODE_SIGNAL_BASE + SIGSYS_CODE
        {
            return true;
        }
    }

    false
}

#[derive(Debug, Clone)]
pub struct StreamOutput<T: Clone> {
    pub text: T,
    pub truncated_after_lines: Option<u32>,
}

#[derive(Debug)]
struct RawExecToolCallOutput {
    pub exit_status: ExitStatus,
    pub stdout: StreamOutput<Vec<u8>>,
    pub stderr: StreamOutput<Vec<u8>>,
    pub aggregated_output: StreamOutput<Vec<u8>>,
    pub timed_out: bool,
}

impl StreamOutput<String> {
    pub fn new(text: String) -> Self {
        Self {
            text,
            truncated_after_lines: None,
        }
    }
}

impl StreamOutput<Vec<u8>> {
    pub fn from_utf8_lossy(&self) -> StreamOutput<String> {
        StreamOutput {
            text: bytes_to_string_smart(&self.text),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[inline]
fn append_all(dst: &mut Vec<u8>, src: &[u8]) {
    dst.extend_from_slice(src);
}

#[derive(Clone, Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
    pub timed_out: bool,
}

#[cfg_attr(not(target_os = "windows"), allow(unused_variables))]
async fn exec(
    params: ExecParams,
    sandbox: SandboxType,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    #[cfg(target_os = "windows")]
    if sandbox == SandboxType::WindowsRestrictedToken
        && !matches!(sandbox_policy, SandboxPolicy::DangerFullAccess)
    {
        return exec_windows_sandbox(params, sandbox_policy).await;
    }
    let ExecParams {
        command,
        cwd,
        env,
        arg0,
        expiration,
        ..
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;
    let arg0_ref = arg0.as_deref();
    let child = spawn_child_async(
        PathBuf::from(program),
        args.into(),
        arg0_ref,
        cwd,
        sandbox_policy,
        StdioPolicy::RedirectForShellTool,
        env,
    )
    .await?;
    consume_truncated_output(child, expiration, stdout_stream).await
}

/// Consumes the output of a child process, truncating it so it is suitable for
/// use as the output of a `shell` tool call. Also enforces specified timeout.
async fn consume_truncated_output(
    mut child: Child,
    expiration: ExecExpiration,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    // Both stdout and stderr were configured with `Stdio::piped()`
    // above, therefore `take()` should normally return `Some`.  If it doesn't
    // we treat it as an exceptional I/O error

    let stdout_reader = child.stdout.take().ok_or_else(|| {
        CodexErr::Io(io::Error::other(
            "stdout pipe was unexpectedly not available",
        ))
    })?;
    let stderr_reader = child.stderr.take().ok_or_else(|| {
        CodexErr::Io(io::Error::other(
            "stderr pipe was unexpectedly not available",
        ))
    })?;

    let (agg_tx, agg_rx) = async_channel::unbounded::<Vec<u8>>();

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(stdout_reader),
        stdout_stream.clone(),
        false,
        Some(agg_tx.clone()),
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(stderr_reader),
        stdout_stream.clone(),
        true,
        Some(agg_tx.clone()),
    ));

    let (exit_status, timed_out) = tokio::select! {
        status_result = child.wait() => {
            let exit_status = status_result?;
            (exit_status, false)
        }
        _ = expiration.wait() => {
            kill_child_process_group(&mut child)?;
            child.start_kill()?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE), true)
        }
        _ = tokio::signal::ctrl_c() => {
            kill_child_process_group(&mut child)?;
            child.start_kill()?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE), false)
        }
    };

    // Wait for the stdout/stderr collection tasks but guard against them
    // hanging forever. In the normal case, both pipes are closed once the child
    // terminates so the tasks exit quickly. However, if the child process
    // spawned grandchildren that inherited its stdout/stderr file descriptors
    // those pipes may stay open after we `kill` the direct child on timeout.
    // That would cause the `read_capped` tasks to block on `read()`
    // indefinitely, effectively hanging the whole agent.

    const IO_DRAIN_TIMEOUT_MS: u64 = 2_000; // 2 s should be plenty for local pipes

    // We need mutable bindings so we can `abort()` them on timeout.
    use tokio::task::JoinHandle;

    async fn await_with_timeout(
        handle: &mut JoinHandle<std::io::Result<StreamOutput<Vec<u8>>>>,
        timeout: Duration,
    ) -> std::io::Result<StreamOutput<Vec<u8>>> {
        match tokio::time::timeout(timeout, &mut *handle).await {
            Ok(join_res) => match join_res {
                Ok(io_res) => io_res,
                Err(join_err) => Err(std::io::Error::other(join_err)),
            },
            Err(_elapsed) => {
                // Timeout: abort the task to avoid hanging on open pipes.
                handle.abort();
                Ok(StreamOutput {
                    text: Vec::new(),
                    truncated_after_lines: None,
                })
            }
        }
    }

    let mut stdout_handle = stdout_handle;
    let mut stderr_handle = stderr_handle;

    let stdout = await_with_timeout(
        &mut stdout_handle,
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS),
    )
    .await?;
    let stderr = await_with_timeout(
        &mut stderr_handle,
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS),
    )
    .await?;

    drop(agg_tx);

    let mut combined_buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    while let Ok(chunk) = agg_rx.recv().await {
        append_all(&mut combined_buf, &chunk);
    }
    let aggregated_output = StreamOutput {
        text: combined_buf,
        truncated_after_lines: None,
    };

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
        timed_out,
    })
}

async fn read_capped<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
    stream: Option<StdoutStream>,
    is_stderr: bool,
    aggregate_tx: Option<Sender<Vec<u8>>>,
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut tmp = [0u8; READ_CHUNK_SIZE];
    let mut emitted_deltas: usize = 0;

    // No caps: append all bytes

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        if let Some(stream) = &stream
            && emitted_deltas < MAX_EXEC_OUTPUT_DELTAS_PER_CALL
        {
            let chunk = tmp[..n].to_vec();
            let msg = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: stream.call_id.clone(),
                stream: if is_stderr {
                    ExecOutputStream::Stderr
                } else {
                    ExecOutputStream::Stdout
                },
                chunk,
            });
            let event = Event {
                id: stream.sub_id.clone(),
                msg,
            };
            #[allow(clippy::let_unit_value)]
            let _ = stream.tx_event.send(event).await;
            emitted_deltas += 1;
        }

        if let Some(tx) = &aggregate_tx {
            let _ = tx.send(tmp[..n].to_vec()).await;
        }

        append_all(&mut buf, &tmp[..n]);
        // Continue reading to EOF to avoid back-pressure
    }

    Ok(StreamOutput {
        text: buf,
        truncated_after_lines: None,
    })
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    // On Windows the raw status is a u32. Use a direct cast to avoid
    // panicking on negative i32 values produced by prior narrowing casts.
    std::process::ExitStatus::from_raw(code as u32)
}

#[cfg(unix)]
fn kill_child_process_group(child: &mut Child) -> io::Result<()> {
    use std::io::ErrorKind;

    if let Some(pid) = child.id() {
        let pid = pid as libc::pid_t;
        let pgid = unsafe { libc::getpgid(pid) };
        if pgid == -1 {
            let err = std::io::Error::last_os_error();
            if err.kind() != ErrorKind::NotFound {
                return Err(err);
            }
            return Ok(());
        }

        let result = unsafe { libc::killpg(pgid, libc::SIGKILL) };
        if result == -1 {
            let err = std::io::Error::last_os_error();
            if err.kind() != ErrorKind::NotFound {
                return Err(err);
            }
        }
    }

    Ok(())
}

#[cfg(not(unix))]
fn kill_child_process_group(_: &mut Child) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn make_exec_output(
        exit_code: i32,
        stdout: &str,
        stderr: &str,
        aggregated: &str,
    ) -> ExecToolCallOutput {
        ExecToolCallOutput {
            exit_code,
            stdout: StreamOutput::new(stdout.to_string()),
            stderr: StreamOutput::new(stderr.to_string()),
            aggregated_output: StreamOutput::new(aggregated.to_string()),
            duration: Duration::from_millis(1),
            timed_out: false,
        }
    }

    #[test]
    fn sandbox_detection_requires_keywords() {
        let output = make_exec_output(1, "", "", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn sandbox_detection_identifies_keyword_in_stderr() {
        let output = make_exec_output(1, "", "Operation not permitted", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[test]
    fn sandbox_detection_respects_quick_reject_exit_codes() {
        let output = make_exec_output(127, "", "command not found", "");
        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn sandbox_detection_ignores_non_sandbox_mode() {
        let output = make_exec_output(1, "", "Operation not permitted", "");
        assert!(!is_likely_sandbox_denied(SandboxType::None, &output));
    }

    #[test]
    fn sandbox_detection_uses_aggregated_output() {
        let output = make_exec_output(
            101,
            "",
            "",
            "cargo failed: Read-only file system when writing target",
        );
        assert!(is_likely_sandbox_denied(
            SandboxType::MacosSeatbelt,
            &output
        ));
    }

    #[cfg(unix)]
    #[test]
    fn sandbox_detection_flags_sigsys_exit_code() {
        let exit_code = EXIT_CODE_SIGNAL_BASE + libc::SIGSYS;
        let output = make_exec_output(exit_code, "", "", "");
        assert!(is_likely_sandbox_denied(SandboxType::LinuxSeccomp, &output));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn kill_child_process_group_kills_grandchildren_on_timeout() -> Result<()> {
        // On Linux/macOS, /bin/bash is typically present; on FreeBSD/OpenBSD,
        // prefer /bin/sh to avoid NotFound errors.
        #[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
        let command = vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 60 & echo $!; sleep 60".to_string(),
        ];
        #[cfg(all(unix, not(any(target_os = "freebsd", target_os = "openbsd"))))]
        let command = vec![
            "/bin/bash".to_string(),
            "-c".to_string(),
            "sleep 60 & echo $!; sleep 60".to_string(),
        ];
        let env: HashMap<String, String> = std::env::vars().collect();
        let params = ExecParams {
            command,
            cwd: std::env::current_dir()?,
            expiration: 500.into(),
            env,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };

        let output = exec(params, SandboxType::None, &SandboxPolicy::ReadOnly, None).await?;
        assert!(output.timed_out);

        let stdout = output.stdout.from_utf8_lossy().text;
        let pid_line = stdout.lines().next().unwrap_or("").trim();
        let pid: i32 = pid_line.parse().map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to parse pid from stdout '{pid_line}': {error}"),
            )
        })?;

        let mut killed = false;
        for _ in 0..20 {
            // Use kill(pid, 0) to check if the process is alive.
            if unsafe { libc::kill(pid, 0) } == -1
                && let Some(libc::ESRCH) = std::io::Error::last_os_error().raw_os_error()
            {
                killed = true;
                break;
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }

        assert!(killed, "grandchild process with pid {pid} is still alive");
        Ok(())
    }

    #[tokio::test]
    async fn process_exec_tool_call_respects_cancellation_token() -> Result<()> {
        let command = long_running_command();
        let cwd = std::env::current_dir()?;
        let env: HashMap<String, String> = std::env::vars().collect();
        let cancel_token = CancellationToken::new();
        let cancel_tx = cancel_token.clone();
        let params = ExecParams {
            command,
            cwd: cwd.clone(),
            expiration: ExecExpiration::Cancellation(cancel_token),
            env,
            with_escalated_permissions: None,
            justification: None,
            arg0: None,
        };
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1_000)).await;
            cancel_tx.cancel();
        });
        let result = process_exec_tool_call(
            params,
            &SandboxPolicy::DangerFullAccess,
            cwd.as_path(),
            &None,
            None,
        )
        .await;
        let output = match result {
            Err(CodexErr::Sandbox(SandboxErr::Timeout { output })) => output,
            other => panic!("expected timeout error, got {other:?}"),
        };
        assert!(output.timed_out);
        assert_eq!(output.exit_code, EXEC_TIMEOUT_EXIT_CODE);
        Ok(())
    }

    #[cfg(unix)]
    fn long_running_command() -> Vec<String> {
        vec![
            "/bin/sh".to_string(),
            "-c".to_string(),
            "sleep 30".to_string(),
        ]
    }

    #[cfg(windows)]
    fn long_running_command() -> Vec<String> {
        vec![
            "powershell.exe".to_string(),
            "-NonInteractive".to_string(),
            "-NoLogo".to_string(),
            "-Command".to_string(),
            "Start-Sleep -Seconds 30".to_string(),
        ]
    }
}
