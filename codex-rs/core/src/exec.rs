#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;

use std::collections::BTreeSet;
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

use crate::sandboxing::ExecOptions;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxPermissions;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use codex_network_proxy::NetworkProxy;
use codex_protocol::config_types::WindowsSandboxLevel;
use codex_protocol::error::CodexErr;
use codex_protocol::error::Result;
use codex_protocol::error::SandboxErr;
use codex_protocol::exec_output::ExecToolCallOutput;
use codex_protocol::exec_output::StreamOutput;
use codex_protocol::permissions::FileSystemSandboxKind;
use codex_protocol::permissions::FileSystemSandboxPolicy;
use codex_protocol::permissions::NetworkSandboxPolicy;
use codex_protocol::protocol::Event;
use codex_protocol::protocol::EventMsg;
use codex_protocol::protocol::ExecCommandOutputDeltaEvent;
use codex_protocol::protocol::ExecOutputStream;
use codex_protocol::protocol::SandboxPolicy;
use codex_sandboxing::SandboxCommand;
use codex_sandboxing::SandboxManager;
use codex_sandboxing::SandboxTransformRequest;
use codex_sandboxing::SandboxType;
use codex_sandboxing::SandboxablePreference;
use codex_utils_absolute_path::AbsolutePathBuf;
use codex_utils_pty::DEFAULT_OUTPUT_BYTES_CAP;
use codex_utils_pty::process_group::kill_child_process_group;

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

/// Hard cap on bytes retained from exec stdout/stderr/aggregated output.
///
/// This mirrors unified exec's output cap so a single runaway command cannot
/// OOM the process by dumping huge amounts of data to stdout/stderr.
const EXEC_OUTPUT_MAX_BYTES: usize = DEFAULT_OUTPUT_BYTES_CAP;

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

// Wait for the stdout/stderr collection tasks but guard against them
// hanging forever. In the normal case, both pipes are closed once the child
// terminates so the tasks exit quickly. However, if the child process
// spawned grandchildren that inherited its stdout/stderr file descriptors
// those pipes may stay open after we `kill` the direct child on timeout.
// That would cause the `read_capped` tasks to block on `read()`
// indefinitely, effectively hanging the whole agent.
pub const IO_DRAIN_TIMEOUT_MS: u64 = 2_000; // 2 s should be plenty for local pipes

#[derive(Debug)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: AbsolutePathBuf,
    pub expiration: ExecExpiration,
    pub capture_policy: ExecCapturePolicy,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub sandbox_permissions: SandboxPermissions,
    pub windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
    pub windows_sandbox_private_desktop: bool,
    pub justification: Option<String>,
    pub arg0: Option<String>,
}

/// Resolved filesystem overrides for the Windows sandbox backends.
///
/// The unelevated restricted-token backend only consumes extra deny-write
/// carveouts on top of the legacy `WorkspaceWrite` allow set. The elevated
/// backend can also consume explicit read and write roots during setup/refresh.
/// Read-root overrides are layered on top of the baseline helper/platform roots
/// that the elevated setup path needs to launch the sandboxed command.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WindowsSandboxFilesystemOverrides {
    pub(crate) read_roots_override: Option<Vec<PathBuf>>,
    pub(crate) write_roots_override: Option<Vec<PathBuf>>,
    pub(crate) additional_deny_write_paths: Vec<AbsolutePathBuf>,
}

fn windows_sandbox_uses_elevated_backend(
    sandbox_level: WindowsSandboxLevel,
    proxy_enforced: bool,
) -> bool {
    // Windows firewall enforcement is tied to the logon-user sandbox identities, so
    // proxy-enforced sessions must use that backend even when the configured mode is
    // the default restricted-token sandbox.
    proxy_enforced || matches!(sandbox_level, WindowsSandboxLevel::Elevated)
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ExecCapturePolicy {
    /// Shell-like execs keep the historical output cap and timeout behavior.
    #[default]
    ShellTool,
    /// Trusted internal helpers can buffer the full child output in memory
    /// without the shell-oriented output cap or exec-expiration behavior.
    FullBuffer,
}

fn select_process_exec_tool_sandbox_type(
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
    enforce_managed_network: bool,
) -> SandboxType {
    SandboxManager::new().select_initial(
        file_system_sandbox_policy,
        network_sandbox_policy,
        SandboxablePreference::Auto,
        windows_sandbox_level,
        enforce_managed_network,
    )
}

/// Mechanism to terminate an exec invocation before it finishes naturally.
#[derive(Clone, Debug)]
pub enum ExecExpiration {
    Timeout(Duration),
    DefaultTimeout,
    Cancellation(CancellationToken),
    TimeoutOrCancellation {
        timeout: Duration,
        cancellation: CancellationToken,
    },
}

/// Why an `ExecExpiration` completed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecExpirationOutcome {
    /// The configured timeout elapsed.
    TimedOut,
    /// The cancellation token was cancelled.
    Cancelled,
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
    /// Waits for this expiration and reports whether it timed out or was cancelled.
    pub async fn wait_with_outcome(self) -> ExecExpirationOutcome {
        match self {
            ExecExpiration::Timeout(duration) => {
                tokio::time::sleep(duration).await;
                ExecExpirationOutcome::TimedOut
            }
            ExecExpiration::DefaultTimeout => {
                tokio::time::sleep(Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS)).await;
                ExecExpirationOutcome::TimedOut
            }
            ExecExpiration::Cancellation(cancel) => {
                cancel.cancelled().await;
                ExecExpirationOutcome::Cancelled
            }
            ExecExpiration::TimeoutOrCancellation {
                timeout,
                cancellation,
            } => {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => ExecExpirationOutcome::Cancelled,
                    _ = tokio::time::sleep(timeout) => ExecExpirationOutcome::TimedOut,
                }
            }
        }
    }

    /// If ExecExpiration is a timeout, returns the timeout in milliseconds.
    pub(crate) fn timeout_ms(&self) -> Option<u64> {
        match self {
            ExecExpiration::Timeout(duration) => Some(duration.as_millis() as u64),
            ExecExpiration::DefaultTimeout => Some(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
            ExecExpiration::Cancellation(_) => None,
            ExecExpiration::TimeoutOrCancellation { timeout, .. } => {
                Some(timeout.as_millis() as u64)
            }
        }
    }

    #[cfg_attr(not(target_os = "windows"), allow(dead_code))]
    pub(crate) fn cancellation_token(&self) -> Option<CancellationToken> {
        match self {
            ExecExpiration::Timeout(_) | ExecExpiration::DefaultTimeout => None,
            ExecExpiration::Cancellation(cancellation)
            | ExecExpiration::TimeoutOrCancellation { cancellation, .. } => {
                Some(cancellation.clone())
            }
        }
    }

    pub(crate) fn with_cancellation(self, cancellation: CancellationToken) -> Self {
        match self {
            ExecExpiration::Timeout(timeout) => ExecExpiration::TimeoutOrCancellation {
                timeout,
                cancellation,
            },
            ExecExpiration::DefaultTimeout => ExecExpiration::TimeoutOrCancellation {
                timeout: Duration::from_millis(DEFAULT_EXEC_COMMAND_TIMEOUT_MS),
                cancellation,
            },
            ExecExpiration::Cancellation(existing) => {
                ExecExpiration::Cancellation(cancel_when_either(existing, cancellation))
            }
            ExecExpiration::TimeoutOrCancellation {
                timeout,
                cancellation: existing,
            } => ExecExpiration::TimeoutOrCancellation {
                timeout,
                cancellation: cancel_when_either(existing, cancellation),
            },
        }
    }
}

pub(crate) fn cancel_when_either(
    first: CancellationToken,
    second: CancellationToken,
) -> CancellationToken {
    let combined = CancellationToken::new();
    let cancel = combined.clone();
    tokio::spawn(async move {
        tokio::select! {
            _ = first.cancelled() => {}
            _ = second.cancelled() => {}
        }
        cancel.cancel();
    });
    combined
}

impl ExecCapturePolicy {
    fn retained_bytes_cap(self) -> Option<usize> {
        match self {
            Self::ShellTool => Some(EXEC_OUTPUT_MAX_BYTES),
            Self::FullBuffer => None,
        }
    }

    fn io_drain_timeout(self) -> Duration {
        Duration::from_millis(IO_DRAIN_TIMEOUT_MS)
    }

    fn uses_expiration(self) -> bool {
        match self {
            Self::ShellTool => true,
            Self::FullBuffer => false,
        }
    }
}

#[derive(Clone)]
pub struct StdoutStream {
    pub sub_id: String,
    pub call_id: String,
    pub tx_event: Sender<Event>,
}

#[allow(clippy::too_many_arguments)]
pub async fn process_exec_tool_call(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_cwd: &AbsolutePathBuf,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    use_legacy_landlock: bool,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let exec_req = build_exec_request(
        params,
        sandbox_policy,
        file_system_sandbox_policy,
        network_sandbox_policy,
        sandbox_cwd,
        codex_linux_sandbox_exe,
        use_legacy_landlock,
    )?;

    // Route through the sandboxing module for a single, unified execution path.
    crate::sandboxing::execute_env(exec_req, stdout_stream).await
}

/// Transform a portable exec request into the concrete argv/env that should be
/// spawned under the requested sandbox policy.
pub fn build_exec_request(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_cwd: &AbsolutePathBuf,
    codex_linux_sandbox_exe: &Option<PathBuf>,
    use_legacy_landlock: bool,
) -> Result<ExecRequest> {
    let ExecParams {
        command,
        cwd,
        mut env,
        expiration,
        capture_policy,
        network,
        windows_sandbox_level,
        windows_sandbox_private_desktop,

        // TODO: Should arg0 be set on the ExecRequest that is returned?
        arg0: _,
        // These fields are related to approvals, so can be ignored here.
        justification: _,
        sandbox_permissions: _,
    } = params;

    let enforce_managed_network = network.is_some();
    let sandbox_type = select_process_exec_tool_sandbox_type(
        file_system_sandbox_policy,
        network_sandbox_policy,
        windows_sandbox_level,
        enforce_managed_network,
    );
    tracing::debug!("Sandbox type: {sandbox_type:?}");

    if let Some(network) = network.as_ref() {
        network.apply_to_env(&mut env);
    }
    let (program, args) = command.split_first().ok_or_else(|| {
        CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;

    let manager = SandboxManager::new();
    let command = SandboxCommand {
        program: program.clone().into(),
        args: args.to_vec(),
        cwd,
        env,
        additional_permissions: None,
    };
    let options = ExecOptions {
        expiration,
        capture_policy,
    };
    let mut exec_req = manager
        .transform(SandboxTransformRequest {
            command,
            policy: sandbox_policy,
            file_system_policy: file_system_sandbox_policy,
            network_policy: network_sandbox_policy,
            sandbox: sandbox_type,
            enforce_managed_network,
            network: network.as_ref(),
            sandbox_policy_cwd: sandbox_cwd,
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.as_deref(),
            use_legacy_landlock,
            windows_sandbox_level,
            windows_sandbox_private_desktop,
        })
        .map(|request| {
            let windows_sandbox_policy_cwd = AbsolutePathBuf::try_from(sandbox_cwd.to_path_buf())
                .unwrap_or_else(|_| request.cwd.clone());
            ExecRequest::from_sandbox_exec_request(request, options, windows_sandbox_policy_cwd)
        })
        .map_err(CodexErr::from)?;
    let use_windows_elevated_backend = windows_sandbox_uses_elevated_backend(
        exec_req.windows_sandbox_level,
        exec_req.network.is_some(),
    );
    exec_req.windows_sandbox_filesystem_overrides = if use_windows_elevated_backend {
        resolve_windows_elevated_filesystem_overrides(
            exec_req.sandbox,
            &exec_req.sandbox_policy,
            &exec_req.file_system_sandbox_policy,
            exec_req.network_sandbox_policy,
            sandbox_cwd,
            use_windows_elevated_backend,
        )
    } else {
        resolve_windows_restricted_token_filesystem_overrides(
            exec_req.sandbox,
            &exec_req.sandbox_policy,
            &exec_req.file_system_sandbox_policy,
            exec_req.network_sandbox_policy,
            sandbox_cwd,
            exec_req.windows_sandbox_level,
        )
    }
    .map_err(CodexErr::UnsupportedOperation)?;
    Ok(exec_req)
}

pub(crate) async fn execute_exec_request(
    exec_request: ExecRequest,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> Result<ExecToolCallOutput> {
    let ExecRequest {
        command,
        cwd,
        env,
        exec_server_env_config: _,
        network,
        expiration,
        capture_policy,
        sandbox,
        windows_sandbox_policy_cwd: _,
        windows_sandbox_level,
        windows_sandbox_private_desktop,
        sandbox_policy,
        // TODO(mbolin): Use file_system_sandbox_policy instead of sandbox_policy.
        file_system_sandbox_policy: _,
        network_sandbox_policy,
        windows_sandbox_filesystem_overrides,
        arg0,
    } = exec_request;

    let params = ExecParams {
        command,
        cwd,
        expiration,
        capture_policy,
        env,
        network: network.clone(),
        sandbox_permissions: SandboxPermissions::UseDefault,
        windows_sandbox_level,
        windows_sandbox_private_desktop,
        justification: None,
        arg0,
    };

    let start = Instant::now();
    let raw_output_result = get_raw_output_result(
        params,
        network_sandbox_policy,
        stdout_stream,
        after_spawn,
        sandbox,
        &sandbox_policy,
        windows_sandbox_filesystem_overrides.as_ref(),
    )
    .await;
    let duration = start.elapsed();
    finalize_exec_result(raw_output_result, sandbox, duration)
}

async fn get_raw_output_result(
    params: ExecParams,
    network_sandbox_policy: NetworkSandboxPolicy,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
    #[cfg_attr(not(windows), allow(unused_variables))] sandbox: SandboxType,
    #[cfg_attr(not(windows), allow(unused_variables))] sandbox_policy: &SandboxPolicy,
    #[cfg_attr(not(windows), allow(unused_variables))] windows_sandbox_filesystem_overrides: Option<
        &WindowsSandboxFilesystemOverrides,
    >,
) -> Result<RawExecToolCallOutput> {
    #[cfg(target_os = "windows")]
    if sandbox == SandboxType::WindowsRestrictedToken {
        return exec_windows_sandbox(params, sandbox_policy, windows_sandbox_filesystem_overrides)
            .await;
    }

    exec(params, network_sandbox_policy, stdout_stream, after_spawn).await
}

#[cfg(target_os = "windows")]
fn extract_create_process_as_user_error_code(err: &str) -> Option<String> {
    let marker = "CreateProcessAsUserW failed: ";
    let start = err.find(marker)? + marker.len();
    let tail = &err[start..];
    let digits: String = tail.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

#[cfg(target_os = "windows")]
fn windowsapps_path_kind(path: &str) -> &'static str {
    let lower = path.to_ascii_lowercase();
    if lower.contains("\\program files\\windowsapps\\") {
        return "windowsapps_package";
    }
    if lower.contains("\\appdata\\local\\microsoft\\windowsapps\\") {
        return "windowsapps_alias";
    }
    if lower.contains("\\windowsapps\\") {
        return "windowsapps_other";
    }
    "other"
}

#[cfg(target_os = "windows")]
fn record_windows_sandbox_spawn_failure(
    command_path: Option<&str>,
    windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
    err: &str,
) {
    let Some(error_code) = extract_create_process_as_user_error_code(err) else {
        return;
    };
    let path = command_path.unwrap_or("unknown");
    let exe = Path::new(path)
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("unknown")
        .to_ascii_lowercase();
    let path_kind = windowsapps_path_kind(path);
    let level = if matches!(
        windows_sandbox_level,
        codex_protocol::config_types::WindowsSandboxLevel::Elevated
    ) {
        "elevated"
    } else {
        "legacy"
    };
    if let Some(metrics) = codex_otel::global() {
        let _ = metrics.counter(
            "codex.windows_sandbox.createprocessasuserw_failed",
            /*inc*/ 1,
            &[
                ("error_code", error_code.as_str()),
                ("path_kind", path_kind),
                ("exe", exe.as_str()),
                ("level", level),
            ],
        );
    }
}

#[cfg(target_os = "windows")]
async fn exec_windows_sandbox(
    params: ExecParams,
    sandbox_policy: &SandboxPolicy,
    windows_sandbox_filesystem_overrides: Option<&WindowsSandboxFilesystemOverrides>,
) -> Result<RawExecToolCallOutput> {
    use crate::config::find_codex_home;
    use codex_windows_sandbox::run_windows_sandbox_capture_elevated;
    use codex_windows_sandbox::run_windows_sandbox_capture_with_extra_deny_write_paths;

    let ExecParams {
        command,
        cwd,
        mut env,
        network,
        expiration,
        capture_policy,
        windows_sandbox_level,
        windows_sandbox_private_desktop,
        ..
    } = params;
    if let Some(network) = network.as_ref() {
        network.apply_to_env(&mut env);
    }

    // Windows sandbox capture still receives timeout and cancellation separately.
    let timeout_ms = if capture_policy.uses_expiration() {
        expiration.timeout_ms()
    } else {
        None
    };
    let cancellation = if capture_policy.uses_expiration() {
        expiration.cancellation_token().map(|token| {
            codex_windows_sandbox::WindowsSandboxCancellationToken::new(move || {
                token.is_cancelled()
            })
        })
    } else {
        None
    };

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
    let command_path = command.first().cloned();
    let sandbox_level = windows_sandbox_level;
    let proxy_enforced = network.is_some();
    let use_elevated = windows_sandbox_uses_elevated_backend(sandbox_level, proxy_enforced);
    let additional_deny_write_paths = windows_sandbox_filesystem_overrides
        .map(|overrides| {
            overrides
                .additional_deny_write_paths
                .iter()
                .map(AbsolutePathBuf::to_path_buf)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let elevated_read_roots_override = windows_sandbox_filesystem_overrides
        .and_then(|overrides| overrides.read_roots_override.clone());
    let elevated_write_roots_override = windows_sandbox_filesystem_overrides
        .and_then(|overrides| overrides.write_roots_override.clone());
    let elevated_deny_write_paths = windows_sandbox_filesystem_overrides
        .map(|overrides| {
            overrides
                .additional_deny_write_paths
                .iter()
                .map(AbsolutePathBuf::to_path_buf)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let spawn_res = tokio::task::spawn_blocking(move || {
        if use_elevated {
            run_windows_sandbox_capture_elevated(
                codex_windows_sandbox::ElevatedSandboxCaptureRequest {
                    policy_json_or_preset: policy_str.as_str(),
                    sandbox_policy_cwd: &sandbox_cwd,
                    codex_home: codex_home.as_ref(),
                    command,
                    cwd: &cwd,
                    env_map: env,
                    timeout_ms,
                    cancellation,
                    use_private_desktop: windows_sandbox_private_desktop,
                    proxy_enforced,
                    read_roots_override: elevated_read_roots_override.as_deref(),
                    write_roots_override: elevated_write_roots_override.as_deref(),
                    deny_write_paths_override: &elevated_deny_write_paths,
                },
            )
        } else {
            run_windows_sandbox_capture_with_extra_deny_write_paths(
                policy_str.as_str(),
                &sandbox_cwd,
                codex_home.as_ref(),
                command,
                &cwd,
                env,
                timeout_ms,
                cancellation,
                &additional_deny_write_paths,
                windows_sandbox_private_desktop,
            )
        }
    })
    .await;

    let capture = match spawn_res {
        Ok(Ok(v)) => v,
        Ok(Err(err)) => {
            record_windows_sandbox_spawn_failure(
                command_path.as_deref(),
                sandbox_level,
                &err.to_string(),
            );
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
    let mut stdout_text = capture.stdout;
    if let Some(max_bytes) = capture_policy.retained_bytes_cap()
        && stdout_text.len() > max_bytes
    {
        stdout_text.truncate(max_bytes);
    }
    let mut stderr_text = capture.stderr;
    if let Some(max_bytes) = capture_policy.retained_bytes_cap()
        && stderr_text.len() > max_bytes
    {
        stderr_text.truncate(max_bytes);
    }
    let stdout = StreamOutput {
        text: stdout_text,
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: stderr_text,
        truncated_after_lines: None,
    };
    let aggregated_output = aggregate_output(&stdout, &stderr, capture_policy.retained_bytes_cap());

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
                    network_policy_decision: None,
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

#[derive(Debug)]
struct RawExecToolCallOutput {
    pub exit_status: ExitStatus,
    pub stdout: StreamOutput<Vec<u8>>,
    pub stderr: StreamOutput<Vec<u8>>,
    pub aggregated_output: StreamOutput<Vec<u8>>,
    pub timed_out: bool,
}

#[inline]
fn append_capped(dst: &mut Vec<u8>, src: &[u8], max_bytes: usize) {
    if dst.len() >= max_bytes {
        return;
    }
    let remaining = max_bytes.saturating_sub(dst.len());
    let take = remaining.min(src.len());
    dst.extend_from_slice(&src[..take]);
}

fn aggregate_output(
    stdout: &StreamOutput<Vec<u8>>,
    stderr: &StreamOutput<Vec<u8>>,
    max_bytes: Option<usize>,
) -> StreamOutput<Vec<u8>> {
    let Some(max_bytes) = max_bytes else {
        let total_len = stdout.text.len().saturating_add(stderr.text.len());
        let mut aggregated = Vec::with_capacity(total_len);
        aggregated.extend_from_slice(&stdout.text);
        aggregated.extend_from_slice(&stderr.text);
        return StreamOutput {
            text: aggregated,
            truncated_after_lines: None,
        };
    };

    let total_len = stdout.text.len().saturating_add(stderr.text.len());
    let mut aggregated = Vec::with_capacity(total_len.min(max_bytes));

    if total_len <= max_bytes {
        aggregated.extend_from_slice(&stdout.text);
        aggregated.extend_from_slice(&stderr.text);
        return StreamOutput {
            text: aggregated,
            truncated_after_lines: None,
        };
    }

    // Under contention, reserve 1/3 for stdout and 2/3 for stderr; rebalance unused stderr to stdout.
    let want_stdout = stdout.text.len().min(max_bytes / 3);
    let want_stderr = stderr.text.len();
    let stderr_take = want_stderr.min(max_bytes.saturating_sub(want_stdout));
    let remaining = max_bytes.saturating_sub(want_stdout + stderr_take);
    let stdout_take = want_stdout + remaining.min(stdout.text.len().saturating_sub(want_stdout));

    aggregated.extend_from_slice(&stdout.text[..stdout_take]);
    aggregated.extend_from_slice(&stderr.text[..stderr_take]);

    StreamOutput {
        text: aggregated,
        truncated_after_lines: None,
    }
}

/// This is a general-purpose function for executing a command specified by
/// [ExecParams]. Events are reported via `stdout_stream`, if specified, and
/// `after_spawn` is invoked once the child process has been spawned, before
/// output consumption begins.
///
/// `network_sandbox_policy` is used to determine whether
/// CODEX_SANDBOX_NETWORK_DISABLED=1 is added to the environment of the spawned
/// process.
///
/// Note this command does not apply any sandboxing logic. The caller is
/// responsible for constructing [ExecParams::command] to include any sandboxing
/// wrapper args, as appropriate.
async fn exec(
    params: ExecParams,
    network_sandbox_policy: NetworkSandboxPolicy,
    stdout_stream: Option<StdoutStream>,
    after_spawn: Option<Box<dyn FnOnce() + Send>>,
) -> Result<RawExecToolCallOutput> {
    let ExecParams {
        command,
        cwd,
        mut env,
        network,
        arg0,
        expiration,
        capture_policy,

        // If applicable, these fields should have been honored upstream of
        // this exec call.
        windows_sandbox_level: _,
        windows_sandbox_private_desktop: _,
        // These fields are related to approvals, so can be ignored here.
        sandbox_permissions: _,
        justification: _,
    } = params;
    if let Some(network) = network.as_ref() {
        network.apply_to_env(&mut env);
    }

    let (program, args) = command.split_first().ok_or_else(|| {
        CodexErr::Io(io::Error::new(
            io::ErrorKind::InvalidInput,
            "command args are empty",
        ))
    })?;
    let arg0_ref = arg0.as_deref();
    let child = spawn_child_async(SpawnChildRequest {
        program: PathBuf::from(program),
        args: args.into(),
        arg0: arg0_ref,
        cwd,
        network_sandbox_policy,
        // The environment already has attempt-scoped proxy settings from
        // apply_to_env_for_attempt above. Passing network here would reapply
        // non-attempt proxy vars and drop attempt correlation metadata.
        network: None,
        stdio_policy: StdioPolicy::RedirectForShellTool,
        env,
    })
    .await?;
    if let Some(after_spawn) = after_spawn {
        after_spawn();
    }
    consume_output(child, expiration, capture_policy, stdout_stream).await
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn should_use_windows_restricted_token_sandbox(
    sandbox: SandboxType,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
) -> bool {
    sandbox == SandboxType::WindowsRestrictedToken
        && file_system_sandbox_policy.kind == FileSystemSandboxKind::Restricted
        && !matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
}

#[cfg_attr(not(test), allow(dead_code))]
pub(crate) fn unsupported_windows_restricted_token_sandbox_reason(
    sandbox: SandboxType,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_policy_cwd: &AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> Option<String> {
    if windows_sandbox_level == WindowsSandboxLevel::Elevated {
        resolve_windows_elevated_filesystem_overrides(
            sandbox,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            sandbox_policy_cwd,
            windows_sandbox_level == WindowsSandboxLevel::Elevated,
        )
        .err()
    } else {
        resolve_windows_restricted_token_filesystem_overrides(
            sandbox,
            sandbox_policy,
            file_system_sandbox_policy,
            network_sandbox_policy,
            sandbox_policy_cwd,
            windows_sandbox_level,
        )
        .err()
    }
}

pub(crate) fn resolve_windows_restricted_token_filesystem_overrides(
    sandbox: SandboxType,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_policy_cwd: &AbsolutePathBuf,
    windows_sandbox_level: WindowsSandboxLevel,
) -> std::result::Result<Option<WindowsSandboxFilesystemOverrides>, String> {
    if sandbox != SandboxType::WindowsRestrictedToken
        || windows_sandbox_level == WindowsSandboxLevel::Elevated
    {
        return Ok(None);
    }

    let needs_direct_runtime_enforcement = file_system_sandbox_policy
        .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd);

    if should_use_windows_restricted_token_sandbox(
        sandbox,
        sandbox_policy,
        file_system_sandbox_policy,
    ) && !needs_direct_runtime_enforcement
    {
        return Ok(None);
    }

    if !should_use_windows_restricted_token_sandbox(
        sandbox,
        sandbox_policy,
        file_system_sandbox_policy,
    ) {
        return Err(format!(
            "windows sandbox backend cannot enforce file_system={:?}, network={network_sandbox_policy:?}, legacy_policy={sandbox_policy:?}; refusing to run unsandboxed",
            file_system_sandbox_policy.kind,
        ));
    }

    if !file_system_sandbox_policy.has_full_disk_read_access() {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce split filesystem read restrictions directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    if !file_system_sandbox_policy
        .get_unreadable_roots_with_cwd(sandbox_policy_cwd)
        .is_empty()
        || !file_system_sandbox_policy
            .get_unreadable_globs_with_cwd(sandbox_policy_cwd)
            .is_empty()
    {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce unreadable split filesystem carveouts directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let legacy_writable_roots = sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
    let split_writable_roots =
        file_system_sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
    let legacy_root_paths: BTreeSet<PathBuf> = legacy_writable_roots
        .iter()
        .map(|root| normalize_windows_override_path(root.root.as_path()))
        .collect::<std::result::Result<_, _>>()?;
    let split_root_paths: BTreeSet<PathBuf> = split_writable_roots
        .iter()
        .map(|root| normalize_windows_override_path(root.root.as_path()))
        .collect::<std::result::Result<_, _>>()?;

    if legacy_root_paths != split_root_paths {
        return Err(
            "windows unelevated restricted-token sandbox cannot enforce split writable root sets directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    for writable_root in &split_writable_roots {
        for read_only_subpath in &writable_root.read_only_subpaths {
            if split_writable_roots.iter().any(|candidate| {
                candidate.root.as_path() != writable_root.root.as_path()
                    && candidate
                        .root
                        .as_path()
                        .starts_with(read_only_subpath.as_path())
            }) {
                return Err(
                    "windows unelevated restricted-token sandbox cannot reopen writable descendants under read-only carveouts directly; refusing to run unsandboxed"
                        .to_string(),
                );
            }
        }
    }

    let mut additional_deny_write_paths = BTreeSet::new();
    for split_root in &split_writable_roots {
        let split_root_path = normalize_windows_override_path(split_root.root.as_path())?;
        let Some(legacy_root) = legacy_writable_roots.iter().find(|candidate| {
            normalize_windows_override_path(candidate.root.as_path())
                .is_ok_and(|candidate_path| candidate_path == split_root_path)
        }) else {
            return Err(
                "windows unelevated restricted-token sandbox cannot enforce split writable root sets directly; refusing to run unsandboxed"
                    .to_string(),
            );
        };

        for read_only_subpath in &split_root.read_only_subpaths {
            if !legacy_root
                .read_only_subpaths
                .iter()
                .any(|candidate| candidate == read_only_subpath)
            {
                additional_deny_write_paths.insert(normalize_windows_override_path(
                    read_only_subpath.as_path(),
                )?);
            }
        }
    }

    if additional_deny_write_paths.is_empty() {
        return Ok(None);
    }

    Ok(Some(WindowsSandboxFilesystemOverrides {
        read_roots_override: None,
        write_roots_override: None,
        additional_deny_write_paths: additional_deny_write_paths
            .into_iter()
            .map(|path| AbsolutePathBuf::from_absolute_path(path).map_err(|err| err.to_string()))
            .collect::<std::result::Result<_, _>>()?,
    }))
}

fn normalize_windows_override_path(path: &Path) -> std::result::Result<PathBuf, String> {
    AbsolutePathBuf::from_absolute_path(dunce::simplified(path))
        .map(AbsolutePathBuf::into_path_buf)
        .map_err(|err| err.to_string())
}

pub(crate) fn resolve_windows_elevated_filesystem_overrides(
    sandbox: SandboxType,
    sandbox_policy: &SandboxPolicy,
    file_system_sandbox_policy: &FileSystemSandboxPolicy,
    network_sandbox_policy: NetworkSandboxPolicy,
    sandbox_policy_cwd: &AbsolutePathBuf,
    use_windows_elevated_backend: bool,
) -> std::result::Result<Option<WindowsSandboxFilesystemOverrides>, String> {
    if sandbox != SandboxType::WindowsRestrictedToken || !use_windows_elevated_backend {
        return Ok(None);
    }

    if !should_use_windows_restricted_token_sandbox(
        sandbox,
        sandbox_policy,
        file_system_sandbox_policy,
    ) {
        return Err(format!(
            "windows sandbox backend cannot enforce file_system={:?}, network={network_sandbox_policy:?}, legacy_policy={sandbox_policy:?}; refusing to run unsandboxed",
            file_system_sandbox_policy.kind,
        ));
    }

    if !file_system_sandbox_policy
        .get_unreadable_roots_with_cwd(sandbox_policy_cwd)
        .is_empty()
        || !file_system_sandbox_policy
            .get_unreadable_globs_with_cwd(sandbox_policy_cwd)
            .is_empty()
    {
        return Err(
            "windows elevated sandbox cannot enforce unreadable split filesystem carveouts directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let split_writable_roots =
        file_system_sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
    if has_reopened_writable_descendant(&split_writable_roots) {
        return Err(
            "windows elevated sandbox cannot reopen writable descendants under read-only carveouts directly; refusing to run unsandboxed"
                .to_string(),
        );
    }

    let needs_direct_runtime_enforcement = file_system_sandbox_policy
        .needs_direct_runtime_enforcement(network_sandbox_policy, sandbox_policy_cwd);
    let normalize_path = |path: PathBuf| dunce::canonicalize(&path).unwrap_or(path);
    let legacy_writable_roots = sandbox_policy.get_writable_roots_with_cwd(sandbox_policy_cwd);
    let legacy_readable_root_set: BTreeSet<PathBuf> = sandbox_policy
        .get_readable_roots_with_cwd(sandbox_policy_cwd)
        .into_iter()
        .map(codex_utils_absolute_path::AbsolutePathBuf::into_path_buf)
        .map(&normalize_path)
        .collect();
    let legacy_root_paths: BTreeSet<PathBuf> = legacy_writable_roots
        .iter()
        .map(|root| normalize_path(root.root.to_path_buf()))
        .collect();
    let split_readable_roots: Vec<PathBuf> = file_system_sandbox_policy
        .get_readable_roots_with_cwd(sandbox_policy_cwd)
        .into_iter()
        .map(codex_utils_absolute_path::AbsolutePathBuf::into_path_buf)
        .map(&normalize_path)
        .collect();
    let split_readable_root_set: BTreeSet<PathBuf> = split_readable_roots.iter().cloned().collect();
    let split_root_paths: Vec<PathBuf> = split_writable_roots
        .iter()
        .map(|root| normalize_path(root.root.to_path_buf()))
        .collect();
    let split_root_path_set: BTreeSet<PathBuf> = split_root_paths.iter().cloned().collect();

    let matches_legacy_read_access = file_system_sandbox_policy.has_full_disk_read_access()
        == sandbox_policy.has_full_disk_read_access();
    let read_roots_override = if matches_legacy_read_access
        && (file_system_sandbox_policy.has_full_disk_read_access()
            || split_readable_root_set == legacy_readable_root_set)
    {
        None
    } else {
        Some(split_readable_roots)
    };

    let write_roots_override = if split_root_path_set == legacy_root_paths {
        None
    } else {
        Some(split_root_paths)
    };

    let additional_deny_write_paths = if needs_direct_runtime_enforcement {
        let mut deny_paths = BTreeSet::new();
        for writable_root in &split_writable_roots {
            let writable_root_path = normalize_path(writable_root.root.to_path_buf());
            let legacy_root = legacy_writable_roots.iter().find(|candidate| {
                normalize_path(candidate.root.to_path_buf()) == writable_root_path
            });
            for read_only_subpath in &writable_root.read_only_subpaths {
                let read_only_subpath_suffix = read_only_subpath
                    .as_path()
                    .strip_prefix(writable_root.root.as_path())
                    .ok();
                let already_denied_by_legacy = legacy_root.is_some_and(|legacy_root| {
                    legacy_root.read_only_subpaths.iter().any(|candidate| {
                        candidate
                            .as_path()
                            .strip_prefix(legacy_root.root.as_path())
                            .ok()
                            == read_only_subpath_suffix
                    })
                });
                if !already_denied_by_legacy {
                    deny_paths.insert(normalize_path(read_only_subpath.to_path_buf()));
                }
            }
        }
        deny_paths
            .into_iter()
            .map(|path| AbsolutePathBuf::from_absolute_path(path).map_err(|err| err.to_string()))
            .collect::<std::result::Result<_, _>>()?
    } else {
        Vec::new()
    };

    if read_roots_override.is_none()
        && write_roots_override.is_none()
        && additional_deny_write_paths.is_empty()
    {
        return Ok(None);
    }

    Ok(Some(WindowsSandboxFilesystemOverrides {
        read_roots_override,
        write_roots_override,
        additional_deny_write_paths,
    }))
}

fn has_reopened_writable_descendant(
    writable_roots: &[codex_protocol::protocol::WritableRoot],
) -> bool {
    writable_roots.iter().any(|writable_root| {
        writable_root
            .read_only_subpaths
            .iter()
            .any(|read_only_subpath| {
                writable_roots.iter().any(|candidate| {
                    candidate.root.as_path() != writable_root.root.as_path()
                        && candidate
                            .root
                            .as_path()
                            .starts_with(read_only_subpath.as_path())
                })
            })
    })
}

/// Consumes the output of a child process according to the configured capture
/// policy.
async fn consume_output(
    mut child: Child,
    expiration: ExecExpiration,
    capture_policy: ExecCapturePolicy,
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

    let retained_bytes_cap = capture_policy.retained_bytes_cap();
    let stdout_handle = tokio::spawn(read_output(
        BufReader::new(stdout_reader),
        stdout_stream.clone(),
        /*is_stderr*/ false,
        retained_bytes_cap,
    ));
    let stderr_handle = tokio::spawn(read_output(
        BufReader::new(stderr_reader),
        stdout_stream.clone(),
        /*is_stderr*/ true,
        retained_bytes_cap,
    ));

    let expiration_wait = async {
        if capture_policy.uses_expiration() {
            Some(expiration.wait_with_outcome().await)
        } else {
            std::future::pending::<Option<ExecExpirationOutcome>>().await
        }
    };
    tokio::pin!(expiration_wait);
    let (exit_status, timed_out) = tokio::select! {
        status_result = child.wait() => {
            let exit_status = status_result?;
            (exit_status, false)
        }
        outcome = &mut expiration_wait => {
            kill_child_process_group(&mut child)?;
            child.start_kill()?;
            let timed_out = matches!(outcome, Some(ExecExpirationOutcome::TimedOut));
            let exit_status = if timed_out {
                synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + TIMEOUT_CODE)
            } else {
                synthetic_exit_status_for_code(/*code*/ 1)
            };
            (exit_status, timed_out)
        }
        _ = tokio::signal::ctrl_c() => {
            kill_child_process_group(&mut child)?;
            child.start_kill()?;
            (synthetic_exit_status(EXIT_CODE_SIGNAL_BASE + SIGKILL_CODE), false)
        }
    };

    // We need mutable bindings so we can `abort()` them on timeout.
    use tokio::task::JoinHandle;

    async fn await_output(
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

    let stdout = await_output(&mut stdout_handle, capture_policy.io_drain_timeout()).await?;
    let stderr = await_output(&mut stderr_handle, capture_policy.io_drain_timeout()).await?;
    let aggregated_output = aggregate_output(&stdout, &stderr, retained_bytes_cap);

    Ok(RawExecToolCallOutput {
        exit_status,
        stdout,
        stderr,
        aggregated_output,
        timed_out,
    })
}

async fn read_output<R: AsyncRead + Unpin + Send + 'static>(
    mut reader: R,
    stream: Option<StdoutStream>,
    is_stderr: bool,
    max_bytes: Option<usize>,
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(
        max_bytes.map_or(AGGREGATE_BUFFER_INITIAL_CAPACITY, |max_bytes| {
            AGGREGATE_BUFFER_INITIAL_CAPACITY.min(max_bytes)
        }),
    );
    let mut tmp = [0u8; READ_CHUNK_SIZE];
    let mut emitted_deltas: usize = 0;

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

        if let Some(max_bytes) = max_bytes {
            append_capped(&mut buf, &tmp[..n], max_bytes);
        } else {
            buf.extend_from_slice(&tmp[..n]);
        }
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

#[cfg(unix)]
fn synthetic_exit_status_for_code(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    std::process::ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
fn synthetic_exit_status(code: i32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    // On Windows the raw status is a u32. Use a direct cast to avoid
    // panicking on negative i32 values produced by prior narrowing casts.
    std::process::ExitStatus::from_raw(code as u32)
}

#[cfg(windows)]
fn synthetic_exit_status_for_code(code: i32) -> ExitStatus {
    synthetic_exit_status(code)
}

#[cfg(test)]
#[path = "exec_tests.rs"]
mod tests;
