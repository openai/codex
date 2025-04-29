use std::io;
#[cfg(target_family = "unix")]
use std::os::unix::process::ExitStatusExt;
use std::path::PathBuf;
use std::process::ExitStatus;
use std::process::Stdio;
use std::sync::Arc;
use std::time::Duration;
use std::time::Instant;

use serde::Deserialize;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::io::BufReader;
use tokio::process::Command;
use tokio::sync::Notify;

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::protocol::SandboxPolicy;

// Maximum we send for each stream, which is either:
// - 10KiB OR
// - 256 lines
const MAX_STREAM_OUTPUT: usize = 10 * 1024;
const MAX_STREAM_OUTPUT_LINES: usize = 256;

const DEFAULT_TIMEOUT_MS: u64 = 10_000;

// Hardcode these since it does not seem worth including the libc crate just
// for these.
const SIGKILL_CODE: i32 = 9;
const TIMEOUT_CODE: i32 = 64;

const MACOS_SEATBELT_READONLY_POLICY: &str = include_str!("seatbelt_readonly_policy.sbpl");

/// When working with `sandbox-exec`, only consider `sandbox-exec` in `/usr/bin`
/// to defend against an attacker trying to inject a malicious version on the
/// PATH. If /usr/bin/sandbox-exec has been tampered with, then the attacker
/// already has root access.
const MACOS_PATH_TO_SEATBELT_EXECUTABLE: &str = "/usr/bin/sandbox-exec";

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
            let seatbelt_command = create_seatbelt_command(command, sandbox_policy, writable_roots);
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
            let stdout = String::from_utf8_lossy(&raw_output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&raw_output.stderr).to_string();

            #[cfg(target_family = "unix")]
            match raw_output.exit_status.signal() {
                Some(TIMEOUT_CODE) => return Err(CodexErr::Sandbox(SandboxErr::Timeout)),
                Some(signal) => {
                    return Err(CodexErr::Sandbox(SandboxErr::Signal(signal)));
                }
                None => {}
            }

            let exit_code = raw_output.exit_status.code().unwrap_or(-1);

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

pub fn create_seatbelt_command(
    command: Vec<String>,
    sandbox_policy: SandboxPolicy,
    writable_roots: &[PathBuf],
) -> Vec<String> {
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

    // TODO(ragona): The seatbelt policy should reflect the SandboxPolicy that
    // is passed, but everything is currently hardcoded to use
    // MACOS_SEATBELT_READONLY_POLICY.
    // TODO(mbolin): apply_patch calls must also honor the SandboxPolicy.
    if !matches!(sandbox_policy, SandboxPolicy::NetworkRestricted) {
        tracing::error!("specified sandbox policy {sandbox_policy:?} will not be honroed");
    }

    let full_policy = if policies.is_empty() {
        MACOS_SEATBELT_READONLY_POLICY.to_string()
    } else {
        let scoped_write_policy = format!("(allow file-write*\n{}\n)", policies.join(" "));
        format!("{MACOS_SEATBELT_READONLY_POLICY}\n{scoped_write_policy}")
    };

    let mut seatbelt_command: Vec<String> = vec![
        MACOS_PATH_TO_SEATBELT_EXECUTABLE.to_string(),
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

        // Do not create a file descriptor for stdin because otherwise some
        // commands may hang forever waiting for input. For example, ripgrep has
        // a heuristic where it may try to read from stdin as explained here:
        // https://github.com/BurntSushi/ripgrep/blob/e2362d4d5185d02fa857bf381e7bd52e66fafc73/crates/core/flags/hiargs.rs#L1101-L1103
        cmd.stdin(Stdio::null());

        cmd.stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?
    };

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(child.stdout.take().expect("stdout is not piped")),
        MAX_STREAM_OUTPUT,
        MAX_STREAM_OUTPUT_LINES,
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(child.stderr.take().expect("stderr is not piped")),
        MAX_STREAM_OUTPUT,
        MAX_STREAM_OUTPUT_LINES,
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
                    synthetic_exit_status(128 + TIMEOUT_CODE)
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

async fn read_capped<R: AsyncRead + Unpin>(
    mut reader: R,
    max_output: usize,
    max_lines: usize,
) -> io::Result<Vec<u8>> {
    let mut buf = Vec::with_capacity(max_output.min(8 * 1024));
    let mut tmp = [0u8; 8192];

    let mut remaining_bytes = max_output;
    let mut remaining_lines = max_lines;

    loop {
        let n = reader.read(&mut tmp).await?;
        if n == 0 {
            break;
        }

        // Copy into the buffer only while we still have byte and line budget.
        if remaining_bytes > 0 && remaining_lines > 0 {
            let mut copy_len = 0;
            for &b in &tmp[..n] {
                if remaining_bytes == 0 || remaining_lines == 0 {
                    break;
                }
                copy_len += 1;
                remaining_bytes -= 1;
                if b == b'\n' {
                    remaining_lines -= 1;
                }
            }
            buf.extend_from_slice(&tmp[..copy_len]);
        }
        // Continue reading to EOF to avoid back-pressure, but discard once caps are hit.
    }

    Ok(buf)
}

#[cfg(unix)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code.try_into().unwrap())
}
