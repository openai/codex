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

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::landlock::spawn_command_under_linux_sandbox;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::SandboxPolicy;
use crate::seatbelt::spawn_command_under_seatbelt;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

// Conventional bases and codes that are platform-agnostic.
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128; // conventional shell: 128 + signal
const EXEC_TIMEOUT_EXIT_CODE: i32 = 124; // conventional timeout exit code
const EXIT_NOT_EXECUTABLE_CODE: i32 = 126; // "found but not executable"/shebang/perms

// Unix signal constants via libc (grouped), including SIGSYS. These are only available on Unix.
#[cfg(target_family = "unix")]
mod unix_sig {
    pub const SIGINT_CODE: i32 = libc::SIGINT; // Ctrl-C / interrupt
    pub const SIGABRT_CODE: i32 = libc::SIGABRT; // abort
    pub const SIGBUS_CODE: i32 = libc::SIGBUS; // bus error
    pub const SIGFPE_CODE: i32 = libc::SIGFPE; // floating point exception
    pub const SIGSEGV_CODE: i32 = libc::SIGSEGV; // segmentation fault
    pub const SIGPIPE_CODE: i32 = libc::SIGPIPE; // broken pipe
    pub const SIGTERM_CODE: i32 = libc::SIGTERM; // termination
    pub const SIGKILL_CODE: i32 = libc::SIGKILL;
    pub const SIGSYS_CODE: i32 = libc::SIGSYS;
}
#[cfg(target_family = "unix")]
use unix_sig::*;

// On non-Unix (Windows), synthesize a "killed" signal using the conventional 9.
#[cfg(not(target_family = "unix"))]
const SIGKILL_CODE: i32 = 9;

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192; // bytes per read
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024; // 8 KiB

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Clone, Debug)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub timeout_ms: Option<u64>,
    pub env: HashMap<String, String>,
    pub with_escalated_permissions: Option<bool>,
    pub justification: Option<String>,
}

impl ExecParams {
    pub fn timeout_duration(&self) -> Duration {
        Duration::from_millis(self.timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS))
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SandboxType {
    None,

    /// Only available on macOS.
    MacosSeatbelt,

    /// Only available on Linux.
    LinuxSeccomp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SandboxVerdict {
    LikelySandbox,
    LikelyNotSandbox,
    Unknown,
}

fn stderr_hints_sandbox(stderr: &str) -> bool {
    // Conservative: require explicit sandbox/seccomp phrasing likely to come from
    // a kernel/seatbelt/seccomp denial; generic perms errors are too noisy.
    let s = stderr.to_ascii_lowercase();
    s.contains("sandbox: deny")
        || s.contains("not permitted by sandbox")
        || s.contains("bad system call")
        || s.contains("seccomp")
        // Less explicit, but commonly emitted by shells and tools when a seatbelt
        // or seccomp policy blocks a write/open. Tests rely on these.
        || s.contains("operation not permitted")
        || s.contains("permission denied")
        || s.contains("read-only file system")
}

fn sandbox_verdict(sandbox_type: SandboxType, status: ExitStatus, stderr: &str) -> SandboxVerdict {
    if matches!(sandbox_type, SandboxType::None) {
        return SandboxVerdict::LikelyNotSandbox;
    }

    // Prefer the signal path when available.
    #[cfg(target_family = "unix")]
    {
        if let Some(sig) = status.signal() {
            if sig == SIGSYS_CODE {
                // SIGSYS under seccomp/seatbelt or invalid syscall is a strong tell.
                return SandboxVerdict::LikelySandbox;
            }
            // Common user/app signals -> not sandbox.
            match sig {
                SIGINT_CODE | SIGABRT_CODE | SIGBUS_CODE | SIGFPE_CODE | SIGKILL_CODE
                | SIGSEGV_CODE | SIGPIPE_CODE | SIGTERM_CODE => {
                    return SandboxVerdict::LikelyNotSandbox;
                }
                _ => { /* fall through */ }
            }
        }
    }

    let code = status.code().unwrap_or(-1);

    // If stderr strongly hints at sandbox denial, prefer that classification even
    // when the exit code is within generic BSD sysexits. This handles cases like
    // macOS seatbelt failing early with "Operation not permitted".
    if stderr_hints_sandbox(stderr) {
        return SandboxVerdict::LikelySandbox;
    }

    // Immediate NotSandbox codes
    // - 0: success
    // - 2: common "misuse of shell builtins"
    // - 124: conventional timeout wrapper exit code
    // - 127: command not found
    // - 64..=78: BSD sysexits range
    // - 128 + {SIGINT, SIGABRT, SIGBUS, SIGFPE, SIGKILL, SIGSEGV, SIGPIPE, SIGTERM}
    #[cfg(target_family = "unix")]
    {
        let sig_derived_not_sandbox = [
            EXIT_CODE_SIGNAL_BASE + SIGINT_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGABRT_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGBUS_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGFPE_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGSEGV_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGPIPE_CODE,
            EXIT_CODE_SIGNAL_BASE + SIGTERM_CODE,
        ];
        if code == 0
            || code == 2
            || code == EXEC_TIMEOUT_EXIT_CODE
            || code == 127
            || (64..=78).contains(&code)
            || sig_derived_not_sandbox.contains(&code)
        {
            return SandboxVerdict::LikelyNotSandbox;
        }
    }
    #[cfg(not(target_family = "unix"))]
    {
        if code == 0
            || code == 2
            || code == EXEC_TIMEOUT_EXIT_CODE
            || code == 127
            || (64..=78).contains(&code)
        {
            return SandboxVerdict::LikelyNotSandbox;
        }
    }

    // Shell-style signal encoding for SIGSYS (Unix).
    #[cfg(target_family = "unix")]
    {
        if code == EXIT_CODE_SIGNAL_BASE + SIGSYS_CODE {
            return SandboxVerdict::LikelySandbox;
        }
    }

    // 126 is often perms/shebang; upgrade only with explicit hints.
    if code == EXIT_NOT_EXECUTABLE_CODE {
        return if stderr_hints_sandbox(stderr) {
            SandboxVerdict::LikelySandbox
        } else {
            SandboxVerdict::Unknown
        };
    }

    if stderr_hints_sandbox(stderr) {
        return SandboxVerdict::LikelySandbox;
    }

    SandboxVerdict::Unknown
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_type: SandboxType,
    sandbox_policy: &SandboxPolicy,
    sandbox_cwd: &Path,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let start = Instant::now();

    let timeout_duration = params.timeout_duration();

    let raw_output_result: std::result::Result<RawExecToolCallOutput, CodexErr> = match sandbox_type
    {
        SandboxType::None => exec(params, sandbox_policy, stdout_stream.clone()).await,
        SandboxType::MacosSeatbelt => {
            let ExecParams {
                command,
                cwd: command_cwd,
                env,
                ..
            } = params;
            let child = spawn_command_under_seatbelt(
                command,
                command_cwd,
                sandbox_policy,
                sandbox_cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;
            consume_truncated_output(child, timeout_duration, stdout_stream.clone()).await
        }
        SandboxType::LinuxSeccomp => {
            let ExecParams {
                command,
                cwd: command_cwd,
                env,
                ..
            } = params;

            let codex_linux_sandbox_exe = codex_linux_sandbox_exe
                .as_ref()
                .ok_or(CodexErr::LandlockSandboxExecutableNotProvided)?;
            let child = spawn_command_under_linux_sandbox(
                codex_linux_sandbox_exe,
                command,
                command_cwd,
                sandbox_policy,
                sandbox_cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;

            consume_truncated_output(child, timeout_duration, stdout_stream).await
        }
    };
    let duration = start.elapsed();
    match raw_output_result {
        Ok(raw_output) => {
            #[allow(unused_mut)]
            let mut timed_out = raw_output.timed_out;
            // If the process was killed by a signal, handle timeouts distinctly and
            // defer SIGSYS (possible sandbox) until after we can examine stderr.
            let mut pending_signal: Option<i32> = None;

            #[cfg(target_family = "unix")]
            {
                if let Some(signal) = raw_output.exit_status.signal() {
                    if signal == TIMEOUT_CODE {
                        timed_out = true;
                    } else {
                        // Defer SIGSYS (possible sandbox) only when a sandbox was requested;
                        // otherwise, treat any non-timeout signal as an immediate error.
                        if signal == SIGSYS_CODE && !matches!(sandbox_type, SandboxType::None) {
                            pending_signal = Some(signal);
                        } else {
                            return Err(CodexErr::Sandbox(SandboxErr::Signal(signal)));
                        }
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

            let verdict = sandbox_verdict(
                sandbox_type,
                raw_output.exit_status,
                &exec_output.stderr.text,
            );
            tracing::debug!(
                target: "codex_core::exec",
                exit_code = exec_output.exit_code,
                ?pending_signal,
                verdict = ?verdict,
                "exec SandboxClassification"
            );
            if matches!(verdict, SandboxVerdict::LikelySandbox) {
                return Err(CodexErr::Sandbox(SandboxErr::Denied {
                    output: Box::new(exec_output),
                }));
            }
            if let Some(sig) = pending_signal {
                return Err(CodexErr::Sandbox(SandboxErr::Signal(sig)));
            }

            Ok(exec_output)
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

#[derive(Debug)]
pub struct StreamOutput<T> {
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
            text: String::from_utf8_lossy(&self.text).to_string(),
            truncated_after_lines: self.truncated_after_lines,
        }
    }
}

#[inline]
fn append_all(dst: &mut Vec<u8>, src: &[u8]) {
    dst.extend_from_slice(src);
}

#[derive(Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
    pub timed_out: bool,
}

async fn exec(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<RawExecToolCallOutput> {
    let timeout = params.timeout_duration();
    let ExecParams {
        command, cwd, env, ..
    } = params;

    let (program, args) = command.split_first().ok_or_else(|| {
        CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;
    let arg0 = None;
    let child = spawn_child_async(
        PathBuf::from(program),
        args.into(),
        arg0,
        cwd,
        sandbox_policy,
        StdioPolicy::RedirectForShellTool,
        env,
    )
    .await?;
    consume_truncated_output(child, timeout, stdout_stream).await
}

/// Consumes the output of a child process, truncating it so it is suitable for
/// use as the output of a `shell` tool call. Also enforces specified timeout.
async fn consume_truncated_output(
    mut child: Child,
    timeout: Duration,
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
        result = tokio::time::timeout(timeout, child.wait()) => {
            match result {
                Ok(status_result) => {
                    let exit_status = status_result?;
                    (exit_status, false)
                }
                Err(_) => {
                    // timeout
                    child.start_kill()?;
                    // Debatable whether `child.wait().await` should be called here.
                    (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE), true)
                }
            }
        }
        _ = tokio::signal::ctrl_c() => {
            child.start_kill()?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE), false)
        }
    };

    let stdout = stdout_handle.await??;
    let stderr = stderr_handle.await??;

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
    #[expect(clippy::unwrap_used)]
    std::process::ExitStatus::from_raw(code.try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::process::ExitStatusExt;

    #[cfg(unix)]
    fn status_from_code(c: i32) -> ExitStatus {
        // Synthesize a normal exit status with the given exit code.
        ExitStatus::from_raw(c << 8)
    }

    #[test]
    #[cfg(unix)]
    fn not_sandbox_obvious_codes() {
        assert!(matches!(
            sandbox_verdict(SandboxType::MacosSeatbelt, status_from_code(127), ""),
            SandboxVerdict::LikelyNotSandbox
        ));
        assert!(matches!(
            sandbox_verdict(SandboxType::LinuxSeccomp, status_from_code(124), ""),
            SandboxVerdict::LikelyNotSandbox
        ));
        assert!(matches!(
            sandbox_verdict(SandboxType::LinuxSeccomp, status_from_code(2), ""),
            SandboxVerdict::LikelyNotSandbox
        ));
    }

    #[test]
    #[cfg(target_family = "unix")]
    fn sandbox_sigsys_codepath() {
        let code = EXIT_CODE_SIGNAL_BASE + SIGSYS_CODE;
        assert!(matches!(
            sandbox_verdict(SandboxType::LinuxSeccomp, status_from_code(code), ""),
            SandboxVerdict::LikelySandbox
        ));
    }

    #[test]
    #[cfg(unix)]
    fn sandbox_stderr_hint() {
        assert!(matches!(
            sandbox_verdict(
                SandboxType::MacosSeatbelt,
                status_from_code(126),
                "Sandbox: deny file-read-data"
            ),
            SandboxVerdict::LikelySandbox
        ));
    }

    #[test]
    #[cfg(unix)]
    fn unknown_generic_failure() {
        assert!(matches!(
            sandbox_verdict(SandboxType::MacosSeatbelt, status_from_code(1), ""),
            SandboxVerdict::Unknown
        ));
    }
}
