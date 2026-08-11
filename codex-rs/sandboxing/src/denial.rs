use codex_protocol::exec_output::ExecToolCallOutput;

use crate::SandboxType;

/// We don't have a fully deterministic way to tell if our command failed
/// because of the sandbox - a command in the user's zshrc file might hit an
/// error, but the command itself might fail or succeed for other reasons.
/// For now, we conservatively check for well known command failure exit codes and
/// also look for common sandbox denial keywords in the command output.
///
/// This predicate is intentionally side-effect free. Callers that handle a
/// denial should record it where the relevant audit context is available.
pub fn is_likely_sandbox_denied(
    sandbox_type: SandboxType,
    exec_output: &ExecToolCallOutput,
) -> bool {
    if sandbox_type == SandboxType::None || exec_output.exit_code == 0 {
        return false;
    }

    if is_likely_executor_managed_sandbox_denied(exec_output) {
        return true;
    }

    const QUICK_REJECT_EXIT_CODES: [i32; 3] = [2, 126, 127];
    if QUICK_REJECT_EXIT_CODES.contains(&exec_output.exit_code) {
        return false;
    }

    #[cfg(unix)]
    {
        const EXIT_CODE_SIGNAL_BASE: i32 = 128;
        const SIGSYS_CODE: i32 = libc::SIGSYS;
        if sandbox_type == SandboxType::LinuxSeccomp
            && exec_output.exit_code == EXIT_CODE_SIGNAL_BASE + SIGSYS_CODE
        {
            return true;
        }
    }

    false
}

/// Detect executor-managed sandbox denials when its concrete backend is unknown.
pub fn is_likely_executor_managed_sandbox_denied(exec_output: &ExecToolCallOutput) -> bool {
    if exec_output.exit_code == 0 {
        return false;
    }

    const SANDBOX_DENIED_KEYWORDS: [&str; 7] = [
        "operation not permitted",
        "permission denied",
        "read-only file system",
        "seccomp",
        "sandbox",
        "landlock",
        "failed to write file",
    ];

    [
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
    })
}
