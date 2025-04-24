use std::io;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use serde::Deserialize;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Notify;

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::protocol::SandboxPolicy;

/// Maximum we keep for each stream (100 KiB).
/// TODO(ragona) this should be reduced
const MAX_STREAM_OUTPUT: usize = 100 * 1024;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

/// Hardcode this since it does not seem worth including the libc craate just
/// for this.
const SIGKILL_CODE: i32 = 9;

const MACOS_SEATBELT_READONLY_POLICY: &str = include_str!("seatbelt_readonly_policy.sbpl");

#[derive(Deserialize, Debug, Clone)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub workdir: Option<String>,

    /// This is the maximum time in seconds that the command is allowed to run.
    #[serde(rename = "timeout")]
    // The wire format uses `timeout`, which has ambiguous units, so we use
    // `timeout_ms` as the field name so it is clear in code.
    pub timeout_ms: Option<u64>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SandboxType {
    None,

    /// Only available on macOS.
    MacosSeatbelt,

    /// Only available on Linux.
    LinuxSeccomp,
}

#[cfg(target_os = "linux")]
async fn exec_linux(
    params: ExecParams,
    writable_roots: &[PathBuf],
    ctrl_c: Arc<Notify>,
    sandbox_policy: SandboxPolicy,
) -> Result<RawExecToolCallOutput> {
    crate::linux::exec_linux(params, writable_roots, ctrl_c, sandbox_policy).await
}

#[cfg(not(target_os = "linux"))]
async fn exec_linux(
    _params: ExecParams,
    _writable_roots: &[PathBuf],
    _ctrl_c: Arc<Notify>,
    _sandbox_policy: SandboxPolicy,
) -> Result<RawExecToolCallOutput> {
    Err(CodexErr::Io(io::Error::new(
        io::ErrorKind::InvalidInput,
        "linux sandbox is not supported on this platform",
    )))
}

pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_type: SandboxType,
    writable_roots: &[PathBuf],
    ctrl_c: Arc<Notify>,
    sandbox_policy: SandboxPolicy,
) -> Result<ExecToolCallOutput> {
    let start = Instant::now();

    let raw_output_result = match sandbox_type {
        SandboxType::None => exec(params, ctrl_c).await,
        SandboxType::MacosSeatbelt => {
            let ExecParams {
                command,
                workdir,
                timeout_ms,
            } = params;
            let seatbelt_command = create_seatbelt_command(command, writable_roots);
            exec(
                ExecParams {
                    command: seatbelt_command,
                    workdir,
                    timeout_ms,
                },
                ctrl_c,
            )
            .await
        }
        SandboxType::LinuxSeccomp => {
            exec_linux(params, writable_roots, ctrl_c, sandbox_policy).await
        }
    };
    let duration = start.elapsed();
    match raw_output_result {
        Ok(raw_output) => {
            let exit_code = raw_output.exit_status.code().unwrap_or(-1);
            let stdout = String::from_utf8_lossy(&raw_output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&raw_output.stderr).to_string();

            // NOTE(ragona): This is much less restrictive than the previous check. If we exec
            // a command, and it returns anything other than success, we assume that it may have
            // been a sandboxing error and allow the user to retry. (The user of course may choose
            // not to retry, or in a non-interactive mode, would automatically reject the approval.)
            if exit_code != 0 && sandbox_type != SandboxType::None {
                return Err(CodexErr::Sandbox(SandboxErr::Denied(
                    exit_code, stdout, stderr,
                )));
            }

            Ok(ExecToolCallOutput {
                exit_code,
                stdout,
                stderr,
                duration,
            })
        }
        Err(err) => {
            tracing::error!("exec error: {err}");
            Err(err)
        }
    }
}

pub fn create_seatbelt_command(command: Vec<String>, writable_roots: &[PathBuf]) -> Vec<String> {
    let (policies, cli_args): (Vec<String>, Vec<String>) = writable_roots
        .iter()
        .enumerate()
        .map(|(index, root)| {
            let param_name = format!("WRITABLE_ROOT_{index}");
            let policy: String = format!("(subpath (param \"{param_name}\"))");
            let cli_arg = format!("-D{param_name}={}", root.to_string_lossy());
            (policy, cli_arg)
        })
        .unzip();

    let full_policy = if policies.is_empty() {
        MACOS_SEATBELT_READONLY_POLICY.to_string()
    } else {
        let scoped_write_policy = format!("(allow file-write*\n{}\n)", policies.join(" "));
        format!("{MACOS_SEATBELT_READONLY_POLICY}\n{scoped_write_policy}")
    };

    let mut seatbelt_command: Vec<String> = vec![
        "sandbox-exec".to_string(),
        "-p".to_string(),
        full_policy.to_string(),
    ];
    seatbelt_command.extend(cli_args);
    seatbelt_command.push("--".to_string());
    seatbelt_command.extend(command);
    seatbelt_command
}

#[derive(Debug)]
pub struct RawExecToolCallOutput {
    pub exit_status: ExitStatus,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

#[derive(Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
}

pub async fn exec(
    ExecParams {
        command,
        workdir,
        timeout_ms,
    }: ExecParams,
    ctrl_c: Arc<Notify>,
) -> Result<RawExecToolCallOutput> {
    let mut child = {
        if command.is_empty() {
            return Err(CodexErr::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "command args are empty",
            )));
        }

        let mut cmd = Command::new(&command[0]);
        if command.len() > 1 {
            cmd.args(&command[1..]);
        }
        if let Some(dir) = &workdir {
            cmd.current_dir(dir);
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
        cmd.kill_on_drop(true);
        cmd.spawn()?
    };

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(child.stdout.take().expect("stdout is not piped")),
        MAX_STREAM_OUTPUT,
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(child.stderr.take().expect("stderr is not piped")),
        MAX_STREAM_OUTPUT,
    ));

    let interrupted = ctrl_c.notified();
    let timeout = Duration::from_millis(timeout_ms.unwrap_or(DEFAULT_TIMEOUT_MS));
    let exit_status = tokio::select! {
        result = tokio::time::timeout(timeout, child.wait()) => {
            match result {
                Ok(Ok(exit_status)) => exit_status,
                Ok(e) => e?,
                Err(_) => {
                    // timeout
                    child.start_kill()?;
                    // Debatable whether `child.wait().await` should be called here.
                    synthetic_exit_status(128 + SIGKILL_CODE)
                }
            }
        }
        _ = interrupted => {
            child.start_kill()?;
            synthetic_exit_status(128 + SIGKILL_CODE)
        }
    };

    let stdout = stdout_handle.await??;
    let stderr = stderr_handle.await??;

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
    })
}

async fn read_capped<R: AsyncReadExt + Unpin>(
    mut reader: R,
    max_output: usize,
) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(max_output.min(8 * 1024));
    let mut tmp = [0u8; 8192];

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }
        if buf.len() < max_output {
            let remaining = max_output - buf.len();
            buf.extend_from_slice(&tmp[..remaining.min(n)]);
        }
    }
    Ok(buf)
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}
