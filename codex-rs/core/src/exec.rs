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
use uuid::Uuid;

use crate::error::CodexErr;
use crate::error::Result;
use crate::error::SandboxErr;
use crate::get_platform_sandbox;
use crate::network_policy_decision::NetworkPolicyDecisionPayload;
use crate::protocol::Event;
use crate::protocol::EventMsg;
use crate::protocol::ExecCommandOutputDeltaEvent;
use crate::protocol::ExecOutputStream;
use crate::protocol::SandboxPolicy;
use crate::sandboxing::CommandSpec;
use crate::sandboxing::ExecRequest;
use crate::sandboxing::SandboxManager;
use crate::sandboxing::SandboxPermissions;
use crate::spawn::SpawnChildRequest;
use crate::spawn::StdioPolicy;
use crate::spawn::spawn_child_async;
use crate::text_encoding::bytes_to_string_smart;
use codex_network_proxy::NetworkProxy;
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
const EXEC_OUTPUT_MAX_BYTES: usize = 1024 * 1024; // 1 MiB

/// Limit the number of ExecCommandOutputDelta events emitted per exec call.
/// Aggregation still collects full output; only the live event stream is capped.
pub(crate) const MAX_EXEC_OUTPUT_DELTAS_PER_CALL: usize = 10_000;

#[derive(Debug)]
pub struct ExecParams {
    pub command: Vec<String>,
    pub cwd: PathBuf,
    pub expiration: ExecExpiration,
    pub env: HashMap<String, String>,
    pub network: Option<NetworkProxy>,
    pub network_attempt_id: Option<String>,
    pub sandbox_permissions: SandboxPermissions,
    pub windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel,
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

impl SandboxType {
    pub(crate) fn as_metric_tag(self) -> &'static str {
        match self {
            SandboxType::None => "none",
            SandboxType::MacosSeatbelt => "seatbelt",
            SandboxType::LinuxSeccomp => "seccomp",
            SandboxType::WindowsRestrictedToken => "windows_sandbox",
        }
    }
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
    use_linux_sandbox_bwrap: bool,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let windows_sandbox_level = params.windows_sandbox_level;
    let enforce_managed_network = params.network.is_some();
    let sandbox_type = match &sandbox_policy {
        SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. } => {
            if enforce_managed_network {
                get_platform_sandbox(
                    windows_sandbox_level
                        != codex_protocol::config_types::WindowsSandboxLevel::Disabled,
                )
                .unwrap_or(SandboxType::None)
            } else {
                SandboxType::None
            }
        }
        _ => get_platform_sandbox(
            windows_sandbox_level != codex_protocol::config_types::WindowsSandboxLevel::Disabled,
        )
        .unwrap_or(SandboxType::None),
    };
    tracing::debug!("Sandbox type: {sandbox_type:?}");

    let ExecParams {
        command,
        cwd,
        mut env,
        expiration,
        network,
        network_attempt_id,
        sandbox_permissions,
        windows_sandbox_level,
        justification,
        arg0: _,
    } = params;
    if let Some(network) = network.as_ref() {
        network.apply_to_env_for_attempt(&mut env, network_attempt_id.as_deref());
    }
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
        sandbox_permissions,
        justification,
    };

    let manager = SandboxManager::new();
    let exec_req = manager
        .transform(crate::sandboxing::SandboxTransformRequest {
            spec,
            policy: sandbox_policy,
            sandbox: sandbox_type,
            enforce_managed_network,
            network: network.as_ref(),
            sandbox_policy_cwd: sandbox_cwd,
            codex_linux_sandbox_exe: codex_linux_sandbox_exe.as_ref(),
            use_linux_sandbox_bwrap,
            windows_sandbox_level,
        })
        .map_err(CodexErr::from)?;

    // Route through the sandboxing module for a single, unified execution path.
    crate::sandboxing::execute_env(exec_req, sandbox_policy, stdout_stream).await
}

pub(crate) async fn execute_exec_env(
    env: ExecRequest,
    sandbox_policy: &SandboxPolicy,
    stdout_stream: Option<StdoutStream>,
) -> Result<ExecToolCallOutput> {
    let ExecRequest {
        command,
        cwd,
        env,
        network,
        network_attempt_id,
        expiration,
        sandbox,
        windows_sandbox_level,
        sandbox_permissions,
        justification,
        arg0,
    } = env;

    let network_attempt_id =
        network_attempt_id.or_else(|| network.as_ref().map(|_| Uuid::new_v4().to_string()));
    let blocked_cursor = match network.as_ref() {
        Some(network) => match network.blocked_requests_cursor().await {
            Ok(cursor) => Some(cursor),
            Err(err) => {
                tracing::debug!("failed to read blocked telemetry cursor before exec: {err:#}");
                None
            }
        },
        None => None,
    };

    let params = ExecParams {
        command,
        cwd,
        expiration,
        env,
        network: network.clone(),
        network_attempt_id: network_attempt_id.clone(),
        sandbox_permissions,
        windows_sandbox_level,
        justification,
        arg0,
    };

    let start = Instant::now();
    let raw_output_result = exec(params, sandbox, sandbox_policy, stdout_stream).await;
    let duration = start.elapsed();
    let finalized = finalize_exec_result(raw_output_result, sandbox, duration);
    let network_policy_decision_for_exec = match network.as_ref() {
        Some(network) => {
            blocking_network_policy_decision_from_attempt_or_cursor(
                network,
                network_attempt_id.as_deref(),
                blocked_cursor,
                sandbox_policy,
            )
            .await
        }
        None => None,
    };
    match finalized {
        Ok(exec_output) => {
            if let Some(policy_decision) = network_policy_decision_for_exec {
                tracing::debug!(
                    "promoting successful exec result to sandbox denied based on structured network telemetry (decision={}, source={}, host={:?}, protocol={:?}, port={:?})",
                    policy_decision.decision,
                    policy_decision.source,
                    policy_decision.host,
                    policy_decision.protocol,
                    policy_decision.port
                );
                return Err(CodexErr::Sandbox(SandboxErr::Denied {
                    output: Box::new(exec_output),
                    network_policy_decision: Some(policy_decision),
                }));
            }
            Ok(exec_output)
        }
        Err(CodexErr::Sandbox(SandboxErr::Denied {
            output,
            network_policy_decision,
        })) => {
            let merged_decision = network_policy_decision.or(network_policy_decision_for_exec);
            if let Some(payload) = merged_decision.as_ref() {
                tracing::debug!(
                    "sandbox-denied exec result includes structured network decision (decision={}, source={}, host={:?}, protocol={:?}, port={:?})",
                    payload.decision,
                    payload.source,
                    payload.host,
                    payload.protocol,
                    payload.port
                );
            } else {
                tracing::debug!(
                    "sandbox-denied exec result had no structured network decision payload"
                );
            }
            Err(CodexErr::Sandbox(SandboxErr::Denied {
                output,
                network_policy_decision: merged_decision,
            }))
        }
        Err(err) => Err(err),
    }
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
    if let Some(metrics) = codex_otel::metrics::global() {
        let _ = metrics.counter(
            "codex.windows_sandbox.createprocessasuserw_failed",
            1,
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
) -> Result<RawExecToolCallOutput> {
    use crate::config::find_codex_home;
    use codex_protocol::config_types::WindowsSandboxLevel;
    use codex_windows_sandbox::run_windows_sandbox_capture;
    use codex_windows_sandbox::run_windows_sandbox_capture_elevated;

    let ExecParams {
        command,
        cwd,
        mut env,
        network,
        network_attempt_id,
        expiration,
        windows_sandbox_level,
        ..
    } = params;
    if let Some(network) = network.as_ref() {
        network.apply_to_env_for_attempt(&mut env, network_attempt_id.as_deref());
    }

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
    let command_path = command.first().cloned();
    let sandbox_level = windows_sandbox_level;
    let use_elevated = matches!(sandbox_level, WindowsSandboxLevel::Elevated);
    let spawn_res = tokio::task::spawn_blocking(move || {
        if use_elevated {
            run_windows_sandbox_capture_elevated(
                policy_str.as_str(),
                &sandbox_cwd,
                codex_home.as_ref(),
                command,
                &cwd,
                env,
                timeout_ms,
            )
        } else {
            run_windows_sandbox_capture(
                policy_str.as_str(),
                &sandbox_cwd,
                codex_home.as_ref(),
                command,
                &cwd,
                env,
                timeout_ms,
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
    if stdout_text.len() > EXEC_OUTPUT_MAX_BYTES {
        stdout_text.truncate(EXEC_OUTPUT_MAX_BYTES);
    }
    let mut stderr_text = capture.stderr;
    if stderr_text.len() > EXEC_OUTPUT_MAX_BYTES {
        stderr_text.truncate(EXEC_OUTPUT_MAX_BYTES);
    }
    let stdout = StreamOutput {
        text: stdout_text,
        truncated_after_lines: None,
    };
    let stderr = StreamOutput {
        text: stderr_text,
        truncated_after_lines: None,
    };
    let aggregated_output = aggregate_output(&stdout, &stderr);

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
    if sandbox_type == SandboxType::None {
        return false;
    }

    if exec_output.exit_code == 0 {
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

pub(crate) async fn blocking_network_policy_decision_from_attempt(
    network: &NetworkProxy,
    attempt_id: &str,
    sandbox_policy: &SandboxPolicy,
) -> Option<NetworkPolicyDecisionPayload> {
    let entry = match network.latest_blocked_request_for_attempt(attempt_id).await {
        Ok(entry) => entry,
        Err(err) => {
            tracing::debug!(
                "failed to load blocked telemetry for network attempt {attempt_id}: {err:#}"
            );
            return None;
        }
    };
    let payload = entry
        .as_ref()
        .and_then(|entry| network_policy_decision_from_blocked_entry(entry, sandbox_policy));
    if let Some(payload) = payload.as_ref() {
        tracing::debug!(
            "selected telemetry network decision for attempt {attempt_id} (decision={}, source={}, host={:?}, protocol={:?}, port={:?})",
            payload.decision,
            payload.source,
            payload.host,
            payload.protocol,
            payload.port
        );
    } else {
        tracing::debug!("no blocked telemetry entry found for network attempt {attempt_id}");
    }
    payload
}

pub(crate) async fn blocking_network_policy_decision_from_attempt_or_cursor(
    network: &NetworkProxy,
    attempt_id: Option<&str>,
    blocked_cursor: Option<u64>,
    sandbox_policy: &SandboxPolicy,
) -> Option<NetworkPolicyDecisionPayload> {
    if let Some(attempt_id) = attempt_id
        && let Some(payload) =
            blocking_network_policy_decision_from_attempt(network, attempt_id, sandbox_policy).await
    {
        return Some(payload);
    }

    let blocked_cursor = blocked_cursor?;
    blocking_network_policy_decision_from_blocked_queue(network, blocked_cursor, sandbox_policy)
        .await
}

pub(crate) async fn blocking_network_policy_decision_from_blocked_queue(
    network: &NetworkProxy,
    cursor: u64,
    sandbox_policy: &SandboxPolicy,
) -> Option<NetworkPolicyDecisionPayload> {
    let entries = match network.blocked_requests_since(cursor).await {
        Ok(entries) => entries,
        Err(err) => {
            tracing::debug!(
                "failed to load blocked telemetry entries since cursor {cursor}: {err:#}"
            );
            return None;
        }
    };
    select_network_policy_decision_from_blocked_entries(&entries, sandbox_policy)
}

fn select_network_policy_decision_from_blocked_entries(
    entries: &[codex_network_proxy::BlockedRequest],
    sandbox_policy: &SandboxPolicy,
) -> Option<NetworkPolicyDecisionPayload> {
    let mapped: Vec<NetworkPolicyDecisionPayload> = entries
        .iter()
        .filter_map(|entry| network_policy_decision_from_blocked_entry(entry, sandbox_policy))
        .collect();
    mapped
        .iter()
        .rev()
        .find(|payload| payload.is_ask_from_decider())
        .cloned()
        .or_else(|| mapped.last().cloned())
}

fn network_policy_decision_from_blocked_entry(
    entry: &codex_network_proxy::BlockedRequest,
    sandbox_policy: &SandboxPolicy,
) -> Option<NetworkPolicyDecisionPayload> {
    let (decision, source) = derive_network_policy_decision(
        entry.reason.as_str(),
        entry.source.as_deref(),
        sandbox_policy,
    )?;
    let protocol = match entry.protocol.as_str() {
        "http-connect" => Some("https_connect".to_string()),
        "socks5" => Some("socks5_tcp".to_string()),
        "socks5-udp" => Some("socks5_udp".to_string()),
        "http" | "https" | "https_connect" | "socks5_tcp" | "socks5_udp" => {
            Some(entry.protocol.clone())
        }
        _ => None,
    }?;
    Some(NetworkPolicyDecisionPayload {
        decision,
        source,
        protocol: Some(protocol),
        host: Some(entry.host.clone()),
        reason: Some(entry.reason.clone()),
        port: entry.port,
    })
}

fn derive_network_policy_decision(
    reason: &str,
    source: Option<&str>,
    sandbox_policy: &SandboxPolicy,
) -> Option<(String, String)> {
    const REASON_NOT_ALLOWED: &str = "not_allowed";
    const REASON_NOT_ALLOWED_LOCAL: &str = "not_allowed_local";
    const REASON_DENIED: &str = "denied";
    const REASON_METHOD_NOT_ALLOWED: &str = "method_not_allowed";

    let source_or = |fallback: &str| source.unwrap_or(fallback).to_string();

    match reason {
        REASON_NOT_ALLOWED if should_ask_on_allowlist_miss(sandbox_policy) => {
            Some(("ask".to_string(), "decider".to_string()))
        }
        REASON_NOT_ALLOWED
        | REASON_NOT_ALLOWED_LOCAL
        | REASON_DENIED
        | REASON_METHOD_NOT_ALLOWED => Some(("deny".to_string(), source_or("baseline_policy"))),
        _ => Some(("deny".to_string(), source_or("proxy_state"))),
    }
}

fn should_ask_on_allowlist_miss(sandbox_policy: &SandboxPolicy) -> bool {
    matches!(
        sandbox_policy,
        SandboxPolicy::ReadOnly { .. } | SandboxPolicy::WorkspaceWrite { .. }
    )
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
) -> StreamOutput<Vec<u8>> {
    let total_len = stdout.text.len().saturating_add(stderr.text.len());
    let max_bytes = EXEC_OUTPUT_MAX_BYTES;
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

#[derive(Clone, Debug)]
pub struct ExecToolCallOutput {
    pub exit_code: i32,
    pub stdout: StreamOutput<String>,
    pub stderr: StreamOutput<String>,
    pub aggregated_output: StreamOutput<String>,
    pub duration: Duration,
    pub timed_out: bool,
}

impl Default for ExecToolCallOutput {
    fn default() -> Self {
        Self {
            exit_code: 0,
            stdout: StreamOutput::new(String::new()),
            stderr: StreamOutput::new(String::new()),
            aggregated_output: StreamOutput::new(String::new()),
            duration: Duration::ZERO,
            timed_out: false,
        }
    }
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
        && !matches!(
            sandbox_policy,
            SandboxPolicy::DangerFullAccess | SandboxPolicy::ExternalSandbox { .. }
        )
    {
        return exec_windows_sandbox(params, sandbox_policy).await;
    }
    let ExecParams {
        command,
        cwd,
        mut env,
        network,
        network_attempt_id,
        arg0,
        expiration,
        windows_sandbox_level: _,
        ..
    } = params;

    if let Some(network) = network.as_ref() {
        network.apply_to_env_for_attempt(&mut env, network_attempt_id.as_deref());
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
        sandbox_policy,
        network: None,
        stdio_policy: StdioPolicy::RedirectForShellTool,
        env,
    })
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

    let stdout_handle = tokio::spawn(read_capped(
        BufReader::new(stdout_reader),
        stdout_stream.clone(),
        false,
    ));
    let stderr_handle = tokio::spawn(read_capped(
        BufReader::new(stderr_reader),
        stdout_stream.clone(),
        true,
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
    let aggregated_output = aggregate_output(&stdout, &stderr);

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
) -> io::Result<StreamOutput<Vec<u8>>> {
    let mut buf = Vec::with_capacity(AGGREGATE_BUFFER_INITIAL_CAPACITY.min(EXEC_OUTPUT_MAX_BYTES));
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

        append_capped(&mut buf, &tmp[..n], EXEC_OUTPUT_MAX_BYTES);
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

#[cfg(test)]
mod tests {
    use super::*;
    use codex_network_proxy::BlockedRequest;
    use codex_protocol::protocol::ReadOnlyAccess;
    use pretty_assertions::assert_eq;
    use std::time::Duration;
    use tokio::io::AsyncWriteExt;

    fn restricted_sandbox_policy() -> SandboxPolicy {
        SandboxPolicy::WorkspaceWrite {
            writable_roots: vec![],
            read_only_access: ReadOnlyAccess::FullAccess,
            network_access: true,
            exclude_tmpdir_env_var: false,
            exclude_slash_tmp: false,
        }
    }

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
    fn sandbox_detection_ignores_network_policy_text_in_non_sandbox_mode() {
        let output = make_exec_output(
            0,
            "",
            "",
            r#"CODEX_NETWORK_POLICY_DECISION {"decision":"ask","reason":"not_allowed","source":"decider","protocol":"http","host":"google.com","port":80}"#,
        );
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

    #[test]
    fn sandbox_detection_ignores_network_policy_text_with_zero_exit_code() {
        let output = make_exec_output(
            0,
            "",
            "",
            r#"CODEX_NETWORK_POLICY_DECISION {"decision":"ask","source":"decider","protocol":"http","host":"google.com","port":80}"#,
        );

        assert!(!is_likely_sandbox_denied(
            SandboxType::LinuxSeccomp,
            &output
        ));
    }

    #[test]
    fn network_policy_decision_maps_fresh_entries() {
        let entry = BlockedRequest {
            host: "google.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http-connect".to_string(),
            attempt_id: None,
            decision: Some("ask".to_string()),
            source: Some("decider".to_string()),
            port: Some(443),
            timestamp: 200,
        };

        let payload =
            network_policy_decision_from_blocked_entry(&entry, &restricted_sandbox_policy())
                .expect("blocked entry should map to structured decision");
        assert_eq!(
            payload,
            NetworkPolicyDecisionPayload {
                decision: "ask".to_string(),
                source: "decider".to_string(),
                protocol: Some("https_connect".to_string()),
                host: Some("google.com".to_string()),
                reason: Some("not_allowed".to_string()),
                port: Some(443),
            }
        );
    }

    #[test]
    fn network_policy_decision_ignores_allow_entries() {
        let entry = BlockedRequest {
            host: "google.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            attempt_id: None,
            decision: Some("allow".to_string()),
            source: Some("decider".to_string()),
            port: Some(80),
            timestamp: 200,
        };

        let payload =
            network_policy_decision_from_blocked_entry(&entry, &restricted_sandbox_policy())
                .expect("restricted sandbox should derive ask decision on allowlist miss");
        assert_eq!(payload.decision, "ask");
        assert_eq!(payload.source, "decider");
    }

    #[test]
    fn network_policy_decision_derives_source_when_missing() {
        let entry = BlockedRequest {
            host: "google.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            attempt_id: None,
            decision: Some("ask".to_string()),
            source: None,
            port: Some(80),
            timestamp: 100,
        };

        let payload =
            network_policy_decision_from_blocked_entry(&entry, &restricted_sandbox_policy())
                .expect("missing source should still map to structured decision");
        assert_eq!(payload.decision, "ask");
        assert_eq!(payload.source, "decider");
    }

    #[test]
    fn yolo_sandbox_denies_allowlist_miss() {
        let entry = BlockedRequest {
            host: "google.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            attempt_id: None,
            decision: Some("ask".to_string()),
            source: Some("decider".to_string()),
            port: Some(80),
            timestamp: 100,
        };

        let payload =
            network_policy_decision_from_blocked_entry(&entry, &SandboxPolicy::DangerFullAccess)
                .expect("allowlist miss should still map to structured decision in yolo");
        assert_eq!(
            payload,
            NetworkPolicyDecisionPayload {
                decision: "deny".to_string(),
                source: "decider".to_string(),
                protocol: Some("http".to_string()),
                host: Some("google.com".to_string()),
                reason: Some("not_allowed".to_string()),
                port: Some(80),
            }
        );
    }

    #[test]
    fn select_network_policy_decision_prefers_ask_from_decider() {
        let entries = vec![
            BlockedRequest {
                host: "google.com".to_string(),
                reason: "not_allowed".to_string(),
                client: None,
                method: Some("GET".to_string()),
                mode: None,
                protocol: "http".to_string(),
                attempt_id: Some("attempt-1".to_string()),
                decision: Some("ask".to_string()),
                source: Some("decider".to_string()),
                port: Some(80),
                timestamp: 100,
            },
            BlockedRequest {
                host: "example.com".to_string(),
                reason: "denied".to_string(),
                client: None,
                method: Some("GET".to_string()),
                mode: None,
                protocol: "http".to_string(),
                attempt_id: Some("attempt-2".to_string()),
                decision: Some("deny".to_string()),
                source: Some("baseline_policy".to_string()),
                port: Some(80),
                timestamp: 200,
            },
        ];

        let selected = select_network_policy_decision_from_blocked_entries(
            &entries,
            &restricted_sandbox_policy(),
        )
        .expect("expected a structured decision from blocked entries");
        assert_eq!(selected.decision, "ask");
        assert_eq!(selected.source, "decider");
        assert_eq!(selected.host.as_deref(), Some("google.com"));
    }

    #[test]
    fn select_network_policy_decision_uses_newest_when_no_ask() {
        let entries = vec![
            BlockedRequest {
                host: "old.example.com".to_string(),
                reason: "denied".to_string(),
                client: None,
                method: Some("GET".to_string()),
                mode: None,
                protocol: "http".to_string(),
                attempt_id: None,
                decision: Some("deny".to_string()),
                source: Some("baseline_policy".to_string()),
                port: Some(80),
                timestamp: 100,
            },
            BlockedRequest {
                host: "new.example.com".to_string(),
                reason: "method_not_allowed".to_string(),
                client: None,
                method: Some("CONNECT".to_string()),
                mode: None,
                protocol: "http-connect".to_string(),
                attempt_id: None,
                decision: Some("deny".to_string()),
                source: Some("mode_guard".to_string()),
                port: Some(443),
                timestamp: 200,
            },
        ];

        let selected = select_network_policy_decision_from_blocked_entries(
            &entries,
            &SandboxPolicy::DangerFullAccess,
        )
        .expect("expected a structured decision from blocked entries");
        assert_eq!(selected.decision, "deny");
        assert_eq!(selected.host.as_deref(), Some("new.example.com"));
        assert_eq!(selected.protocol.as_deref(), Some("https_connect"));
    }

    #[tokio::test]
    async fn read_capped_limits_retained_bytes() {
        let (mut writer, reader) = tokio::io::duplex(1024);
        let bytes = vec![b'a'; EXEC_OUTPUT_MAX_BYTES.saturating_add(128 * 1024)];
        tokio::spawn(async move {
            writer.write_all(&bytes).await.expect("write");
        });

        let out = read_capped(reader, None, false).await.expect("read");
        assert_eq!(out.text.len(), EXEC_OUTPUT_MAX_BYTES);
    }

    #[test]
    fn aggregate_output_prefers_stderr_on_contention() {
        let stdout = StreamOutput {
            text: vec![b'a'; EXEC_OUTPUT_MAX_BYTES],
            truncated_after_lines: None,
        };
        let stderr = StreamOutput {
            text: vec![b'b'; EXEC_OUTPUT_MAX_BYTES],
            truncated_after_lines: None,
        };

        let aggregated = aggregate_output(&stdout, &stderr);
        let stdout_cap = EXEC_OUTPUT_MAX_BYTES / 3;
        let stderr_cap = EXEC_OUTPUT_MAX_BYTES.saturating_sub(stdout_cap);

        assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
        assert_eq!(aggregated.text[..stdout_cap], vec![b'a'; stdout_cap]);
        assert_eq!(aggregated.text[stdout_cap..], vec![b'b'; stderr_cap]);
    }

    #[test]
    fn aggregate_output_fills_remaining_capacity_with_stderr() {
        let stdout_len = EXEC_OUTPUT_MAX_BYTES / 10;
        let stdout = StreamOutput {
            text: vec![b'a'; stdout_len],
            truncated_after_lines: None,
        };
        let stderr = StreamOutput {
            text: vec![b'b'; EXEC_OUTPUT_MAX_BYTES],
            truncated_after_lines: None,
        };

        let aggregated = aggregate_output(&stdout, &stderr);
        let stderr_cap = EXEC_OUTPUT_MAX_BYTES.saturating_sub(stdout_len);

        assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
        assert_eq!(aggregated.text[..stdout_len], vec![b'a'; stdout_len]);
        assert_eq!(aggregated.text[stdout_len..], vec![b'b'; stderr_cap]);
    }

    #[test]
    fn aggregate_output_rebalances_when_stderr_is_small() {
        let stdout = StreamOutput {
            text: vec![b'a'; EXEC_OUTPUT_MAX_BYTES],
            truncated_after_lines: None,
        };
        let stderr = StreamOutput {
            text: vec![b'b'; 1],
            truncated_after_lines: None,
        };

        let aggregated = aggregate_output(&stdout, &stderr);
        let stdout_len = EXEC_OUTPUT_MAX_BYTES.saturating_sub(1);

        assert_eq!(aggregated.text.len(), EXEC_OUTPUT_MAX_BYTES);
        assert_eq!(aggregated.text[..stdout_len], vec![b'a'; stdout_len]);
        assert_eq!(aggregated.text[stdout_len..], vec![b'b'; 1]);
    }

    #[test]
    fn aggregate_output_keeps_stdout_then_stderr_when_under_cap() {
        let stdout = StreamOutput {
            text: vec![b'a'; 4],
            truncated_after_lines: None,
        };
        let stderr = StreamOutput {
            text: vec![b'b'; 3],
            truncated_after_lines: None,
        };

        let aggregated = aggregate_output(&stdout, &stderr);
        let mut expected = Vec::new();
        expected.extend_from_slice(&stdout.text);
        expected.extend_from_slice(&stderr.text);

        assert_eq!(aggregated.text, expected);
        assert_eq!(aggregated.truncated_after_lines, None);
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
            network: None,
            network_attempt_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel::Disabled,
            justification: None,
            arg0: None,
        };

        let output = exec(
            params,
            SandboxType::None,
            &SandboxPolicy::new_read_only_policy(),
            None,
        )
        .await?;
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
            network: None,
            network_attempt_id: None,
            sandbox_permissions: SandboxPermissions::UseDefault,
            windows_sandbox_level: codex_protocol::config_types::WindowsSandboxLevel::Disabled,
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
            false,
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
