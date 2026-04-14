use crate::SandboxType;
use codex_network_proxy::BlockedRequest;
use codex_protocol::exec_output::ExecToolCallOutput;
use tracing::warn;

const EXIT_CODE_SIGNAL_BASE: i32 = 128;
const OUTPUT_SNIPPET_MAX_CHARS: usize = 512;

const SANDBOX_DENIED_KEYWORDS: [(FileSystemSandboxViolationReason, &str); 7] = [
    (
        FileSystemSandboxViolationReason::OperationNotPermitted,
        "operation not permitted",
    ),
    (
        FileSystemSandboxViolationReason::PermissionDenied,
        "permission denied",
    ),
    (
        FileSystemSandboxViolationReason::ReadOnlyFileSystem,
        "read-only file system",
    ),
    (FileSystemSandboxViolationReason::Seccomp, "seccomp"),
    (FileSystemSandboxViolationReason::Sandbox, "sandbox"),
    (FileSystemSandboxViolationReason::Landlock, "landlock"),
    (
        FileSystemSandboxViolationReason::FailedToWriteFile,
        "failed to write file",
    ),
];

// Quick rejects: well-known non-sandbox shell exit codes.
// 2: misuse of shell builtins
// 126: permission denied
// 127: command not found
const QUICK_REJECT_EXIT_CODES: [i32; 3] = [2, 126, 127];

/// A normalized sandbox violation observed by Codex sandbox enforcement.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SandboxViolationEvent {
    FileSystem(FileSystemSandboxViolation),
    Network(NetworkSandboxViolation),
}

/// A filesystem sandbox denial inferred from a sandboxed process result.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileSystemSandboxViolation {
    pub sandbox_type: SandboxType,
    pub reason: FileSystemSandboxViolationReason,
    pub path: Option<String>,
    pub output_snippet: String,
}

/// Normalized reasons used when classifying filesystem sandbox denials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FileSystemSandboxViolationReason {
    OperationNotPermitted,
    PermissionDenied,
    ReadOnlyFileSystem,
    Seccomp,
    Sandbox,
    Landlock,
    FailedToWriteFile,
    SignalSyscall,
}

impl FileSystemSandboxViolationReason {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OperationNotPermitted => "operation_not_permitted",
            Self::PermissionDenied => "permission_denied",
            Self::ReadOnlyFileSystem => "read_only_file_system",
            Self::Seccomp => "seccomp",
            Self::Sandbox => "sandbox",
            Self::Landlock => "landlock",
            Self::FailedToWriteFile => "failed_to_write_file",
            Self::SignalSyscall => "sigsys",
        }
    }
}

/// A network sandbox denial reported by the managed network proxy.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NetworkSandboxViolation {
    pub host: String,
    pub reason: String,
    pub client: Option<String>,
    pub method: Option<String>,
    pub protocol: String,
    pub decision: Option<String>,
    pub source: Option<String>,
    pub port: Option<u16>,
    pub timestamp: i64,
}

impl NetworkSandboxViolation {
    pub fn from_blocked_request(blocked: &BlockedRequest) -> Self {
        Self {
            host: blocked.host.clone(),
            reason: blocked.reason.clone(),
            client: blocked.client.clone(),
            method: blocked.method.clone(),
            protocol: blocked.protocol.clone(),
            decision: blocked.decision.clone(),
            source: blocked.source.clone(),
            port: blocked.port,
            timestamp: blocked.timestamp,
        }
    }
}

/// Classify a sandboxed process result as a filesystem sandbox violation.
pub fn classify_filesystem_sandbox_violation(
    sandbox_type: SandboxType,
    exec_output: &ExecToolCallOutput,
) -> Option<FileSystemSandboxViolation> {
    if sandbox_type == SandboxType::None || exec_output.exit_code == 0 {
        return None;
    }

    if let Some(reason) = filesystem_reason_from_output(exec_output) {
        return Some(FileSystemSandboxViolation {
            sandbox_type,
            reason,
            path: extract_denied_path(exec_output),
            output_snippet: output_snippet(exec_output),
        });
    }

    if QUICK_REJECT_EXIT_CODES.contains(&exec_output.exit_code) {
        return None;
    }

    #[cfg(unix)]
    {
        if sandbox_type == SandboxType::LinuxSeccomp
            && exec_output.exit_code == EXIT_CODE_SIGNAL_BASE + libc::SIGSYS
        {
            return Some(FileSystemSandboxViolation {
                sandbox_type,
                reason: FileSystemSandboxViolationReason::SignalSyscall,
                path: None,
                output_snippet: output_snippet(exec_output),
            });
        }
    }

    None
}

/// Preserve the legacy boolean sandbox-denial check for call sites that only need a retry decision.
///
/// We don't have a fully deterministic way to tell if a command failed because
/// of the sandbox: a command in the user's zshrc file might hit an error, but
/// the command itself might fail or succeed for other reasons. For now, we
/// conservatively check for well known command failure exit codes and also look
/// for common sandbox denial keywords in the command output.
pub fn is_likely_sandbox_denied(
    sandbox_type: SandboxType,
    exec_output: &ExecToolCallOutput,
) -> bool {
    classify_filesystem_sandbox_violation(sandbox_type, exec_output).is_some()
}

/// Record a filesystem sandbox violation, returning the classified event when one was found.
pub fn record_filesystem_sandbox_violation(
    sandbox_type: SandboxType,
    exec_output: &ExecToolCallOutput,
) -> Option<FileSystemSandboxViolation> {
    let violation = classify_filesystem_sandbox_violation(sandbox_type, exec_output)?;
    record_sandbox_violation(&SandboxViolationEvent::FileSystem(violation.clone()));
    Some(violation)
}

/// Record a network sandbox violation from a managed-proxy blocked request.
pub fn record_network_sandbox_violation(blocked: &BlockedRequest) -> NetworkSandboxViolation {
    let violation = NetworkSandboxViolation::from_blocked_request(blocked);
    record_sandbox_violation(&SandboxViolationEvent::Network(violation.clone()));
    violation
}

/// Emit a sandbox violation to the tracing stack.
pub fn record_sandbox_violation(event: &SandboxViolationEvent) {
    match event {
        SandboxViolationEvent::FileSystem(violation) => {
            let path = violation.path.as_deref().unwrap_or("unknown");
            warn!(
                "recorded sandbox violation: resource=filesystem sandbox={} reason={} path={}",
                violation.sandbox_type.as_metric_tag(),
                violation.reason.as_str(),
                path
            );
        }
        SandboxViolationEvent::Network(violation) => {
            warn!(
                "recorded sandbox violation: resource=network protocol={} host={} port={:?} reason={} method={:?} client={:?} decision={:?} source={:?}",
                violation.protocol,
                violation.host,
                violation.port,
                violation.reason,
                violation.method,
                violation.client,
                violation.decision,
                violation.source
            );
        }
    }
}

fn filesystem_reason_from_output(
    exec_output: &ExecToolCallOutput,
) -> Option<FileSystemSandboxViolationReason> {
    [
        &exec_output.stderr.text,
        &exec_output.stdout.text,
        &exec_output.aggregated_output.text,
    ]
    .into_iter()
    .find_map(|section| {
        let lower = section.to_lowercase();
        SANDBOX_DENIED_KEYWORDS
            .iter()
            .find_map(|(reason, needle)| lower.contains(needle).then_some(*reason))
    })
}

fn extract_denied_path(exec_output: &ExecToolCallOutput) -> Option<String> {
    [
        &exec_output.stderr.text,
        &exec_output.stdout.text,
        &exec_output.aggregated_output.text,
    ]
    .into_iter()
    .find_map(|section| extract_denied_path_from_text(section))
}

fn extract_denied_path_from_text(text: &str) -> Option<String> {
    const PATH_MARKERS: [&str; 3] = [
        ": operation not permitted",
        ": permission denied",
        ": read-only file system",
    ];

    for line in text.lines() {
        let lower = line.to_lowercase();
        for marker in PATH_MARKERS {
            let Some(marker_start) = lower.find(marker) else {
                continue;
            };
            let candidate_prefix = &line[..marker_start];
            let candidate = candidate_prefix
                .rsplit_once(": ")
                .map_or(candidate_prefix, |(_, path)| path)
                .trim()
                .trim_matches('"')
                .trim_matches('\'');
            if candidate.starts_with('/')
                || candidate.starts_with("./")
                || candidate.starts_with("../")
            {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

fn output_snippet(exec_output: &ExecToolCallOutput) -> String {
    [
        &exec_output.stderr.text,
        &exec_output.stdout.text,
        &exec_output.aggregated_output.text,
    ]
    .into_iter()
    .find_map(|section| {
        let trimmed = section.trim();
        (!trimmed.is_empty()).then(|| trimmed.chars().take(OUTPUT_SNIPPET_MAX_CHARS).collect())
    })
    .unwrap_or_default()
}

#[cfg(test)]
#[path = "violation_tests.rs"]
mod tests;
