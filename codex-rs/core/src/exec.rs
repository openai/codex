#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::time::Duration;
use std::time::Instant;

use async_channel::Sender;
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
use serde_bytes::ByteBuf;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

// Hardcode these since it does not seem worth including the libc crate just
// for these.
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;
const EXIT_CODE_SIGNAL_BASE: i32 = 128; // conventional shell: 128 + signal

// I/O buffer sizing
const READ_CHUNK_SIZE: usize = 8192; // bytes per read
const AGGREGATE_BUFFER_INITIAL_CAPACITY: usize = 8 * 1024; // 8 KiB

// (no per-stream truncation limits here; formatting limits are handled elsewhere)

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug, Clone)]
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
    codex_linux_sandbox_exe: &Option<PathBuf>,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let start = Instant::now();

    let raw_output_result: std::result::Result<RawExecToolCallOutput, CodexErr> = match sandbox_type
    {
        SandboxType::None => exec(params, sandbox_policy, stdout_stream.clone()).await,
        SandboxType::MacosSeatbelt => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;
            let child = spawn_command_under_seatbelt(
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;
            consume_truncated_output(child, timeout, stdout_stream.clone()).await
        }
        SandboxType::LinuxSeccomp => {
            let timeout = params.timeout_duration();
            let ExecParams {
                command, cwd, env, ..
            } = params;

            let codex_linux_sandbox_exe = codex_linux_sandbox_exe
                .as_ref()
                .ok_or(CodexErr::LandlockSandboxExecutableNotProvided)?;
            let child = spawn_command_under_linux_sandbox(
                codex_linux_sandbox_exe,
                command,
                sandbox_policy,
                cwd,
                StdioPolicy::RedirectForShellTool,
                env,
            )
            .await?;

            consume_truncated_output(child, timeout, stdout_stream).await
        }
    };
    let duration = start.elapsed();
    match raw_output_result {
        Ok(raw_output) => {
            let stdout = raw_output.stdout.from_utf8_lossy();
            let stderr = raw_output.stderr.from_utf8_lossy();

            #[cfg(target_family = "unix")]
            match raw_output.exit_status.signal() {
                Some(TIMEOUT_CODE) => return Err(CodexErr::Sandbox(SandboxErr::Timeout)),
                Some(signal) => {
                    return Err(CodexErr::Sandbox(SandboxErr::Signal(signal)));
                }
                None => {}
            }

            let exit_code = raw_output.exit_status.code().unwrap_or(-1);

            if exit_code != 0 && is_likely_sandbox_denied(sandbox_type, exit_code) {
                return Err(CodexErr::Sandbox(SandboxErr::Denied(
                    exit_code,
                    stdout.text,
                    stderr.text,
                )));
            }

            Ok(ExecToolCallOutput {
                exit_code,
                stdout,
                stderr,
                aggregated_output: raw_output.aggregated_output.from_utf8_lossy(),
                duration,
            })
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

/// We don't have a fully deterministic way to tell if our command failed
/// because of the sandbox - a command in the user's zshrc file might hit an
/// error, but the command itself might fail or succeed for other reasons.
/// For now, we conservatively check for 'command not found' (exit code 127),
/// and can add additional cases as necessary.
fn is_likely_sandbox_denied(sandbox_type: SandboxType, exit_code: i32) -> bool {
    if sandbox_type == SandboxType::None {
        return false;
    }

    // Quick rejects: well-known non-sandbox shell exit codes
    // 127: command not found, 2: misuse of shell builtins
    if exit_code == 127 {
        return false;
    }

    // For all other cases, we assume the sandbox is the cause
    true
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

    // Interleave reads from stdout and stderr to preserve write order as much as possible.
    let mut stdout_reader = BufReader::new(stdout_reader);
    let mut stderr_reader = BufReader::new(stderr_reader);

    let mut out_stdout: Vec<u8> = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut out_stderr: Vec<u8> = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);
    let mut out_agg: Vec<u8> = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY);

    let mut tmp_stdout = [0u8; READ_CHUNK_SIZE];
    let mut tmp_stderr = [0u8; READ_CHUNK_SIZE];

    let mut stdout_open = true;
    let mut stderr_open = true;

    // Helper to emit a delta event, if enabled
    let mut emitted_deltas: usize = 0;
    let mut emit_delta = |chunk: &[u8], is_stderr: bool| {
        if let Some(stream) = &stdout_stream
            && emitted_deltas < MAX_EXEC_OUTPUT_DELTAS_PER_CALL
        {
            let msg = EventMsg::ExecCommandOutputDelta(ExecCommandOutputDeltaEvent {
                call_id: stream.call_id.clone(),
                stream: if is_stderr {
                    ExecOutputStream::Stderr
                } else {
                    ExecOutputStream::Stdout
                },
                chunk: ByteBuf::from(chunk.to_vec()),
            });
            let event = Event {
                id: stream.sub_id.clone(),
                msg,
            };
            let _ = stream.tx_event.try_send(event);
            emitted_deltas += 1;
        }
    };

    let mut child_finished = false;
    let mut exit_status: Option<ExitStatus> = None;
    let timeout_fut = tokio::time::sleep(timeout);
    tokio::pin!(timeout_fut);

    // Drive process, timeout, and both pipes concurrently to provide live streaming
    while (stdout_open || stderr_open) || !child_finished {
        tokio::select! {
            // Timeout
            _ = &mut timeout_fut, if exit_status.is_none() => {
                let _ = child.start_kill();
                exit_status = Some(synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE));
                child_finished = true;
            }

            // Process exit
            res = child.wait(), if !child_finished => {
                match res {
                    Ok(status) => exit_status = Some(status),
                    Err(e) => return Err(CodexErr::Io(e)),
                }
                child_finished = true;
            }

            // Stdout chunk
            read = stdout_reader.read(&mut tmp_stdout), if stdout_open => {
                match read {
                    Ok(0) => stdout_open = false,
                    Ok(n) => {
                        append_all(&mut out_stdout, &tmp_stdout[..n]);
                        append_all(&mut out_agg, &tmp_stdout[..n]);
                        emit_delta(&tmp_stdout[..n], false);
                    }
                    Err(e) => return Err(CodexErr::Io(e)),
                }
            }

            // Stderr chunk
            read = stderr_reader.read(&mut tmp_stderr), if stderr_open => {
                match read {
                    Ok(0) => stderr_open = false,
                    Ok(n) => {
                        append_all(&mut out_stderr, &tmp_stderr[..n]);
                        append_all(&mut out_agg, &tmp_stderr[..n]);
                        emit_delta(&tmp_stderr[..n], true);
                    }
                    Err(e) => return Err(CodexErr::Io(e)),
                }
            }

            // Ctrl-C termination
            _ = tokio::signal::ctrl_c() => {
                let _ = child.start_kill();
                exit_status = Some(synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE));
                child_finished = true;
            }
        }
    }

    // Ensure we have an exit status
    let exit_status = exit_status.unwrap_or_else(|| synthetic_exit_status(0));

    let stdout = StreamOutput {
        text: out_stdout,
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: out_stderr,
        truncated_after_lines: None,
    };
    let aggregated_output = StreamOutput {
        text: out_agg,
        truncated_after_lines: None,
    };

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
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
