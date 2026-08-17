use super::super::CheckStatus;
use super::super::DoctorCheck;
use super::platform::desktop_check;
use std::io;
use std::path::Path;
use std::process::Output;
use std::process::Stdio;
use std::time::Duration;
use tokio::io::AsyncRead;
use tokio::io::AsyncReadExt;
use tokio::process::Command;
use tokio::time::timeout;

const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_OUTPUT_BYTES: usize = 256 * 1024;
const SECURITY_LOG_PREDICATE: &str = concat!(
    "(process == 'syspolicyd' OR process == 'AppleSystemPolicy' ",
    "OR process BEGINSWITH 'XProtect' OR subsystem BEGINSWITH 'com.apple.syspolicy' ",
    "OR subsystem CONTAINS[c] 'XProtect' ",
    "OR subsystem == 'com.apple.security.assessment') AND ",
    "(eventMessage CONTAINS[c] 'codex' OR eventMessage CONTAINS[c] 'chatgpt' ",
    "OR eventMessage CONTAINS[c] '.plugin-appserver' ",
    "OR eventMessage CONTAINS[c] '100024' OR eventMessage CONTAINS[c] 'EMFILE' ",
    "OR eventMessage CONTAINS[c] 'ENFILE' ",
    "OR eventMessage CONTAINS[c] 'too many open files' ",
    "OR eventMessage CONTAINS[c] 'Unexpected Xprotect assessment')"
);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum Evidence {
    Clear,
    Audit,
    Exhausted,
    Blocked,
    Malware,
    Unavailable,
}

pub(super) async fn collect(bundle: &Path) -> DoctorCheck {
    let (gatekeeper, events) = tokio::join!(inspect_gatekeeper(bundle), inspect_security_events());
    enforcement_check(gatekeeper, events)
}

async fn inspect_gatekeeper(bundle: &Path) -> Evidence {
    let mut command = Command::new("/usr/sbin/spctl");
    command
        .args(["--assess", "--type", "execute", "--verbose=2"])
        .arg(bundle);
    classify_gatekeeper(run_command(&mut command).await.as_ref())
}

fn classify_gatekeeper(output: Option<&Output>) -> Evidence {
    let Some(output) = output.filter(|output| !is_truncated(output)) else {
        return Evidence::Unavailable;
    };
    if output.status.success() {
        return Evidence::Clear;
    }
    let error = String::from_utf8_lossy(&output.stderr).to_ascii_lowercase();
    if error.contains("rejected") || error.contains("no usable signature") {
        Evidence::Blocked
    } else {
        Evidence::Unavailable
    }
}

async fn inspect_security_events() -> Evidence {
    let mut command = Command::new("/usr/bin/log");
    command.args([
        "show",
        "--last",
        "30m",
        "--info",
        "--style",
        "compact",
        "--predicate",
        SECURITY_LOG_PREDICATE,
    ]);
    let Some(output) = run_command(&mut command)
        .await
        .filter(|output| output.status.success())
    else {
        return Evidence::Unavailable;
    };
    let evidence = classify_security_events(&String::from_utf8_lossy(&output.stdout));
    if is_truncated(&output) && evidence < Evidence::Exhausted {
        Evidence::Unavailable
    } else {
        evidence
    }
}

fn classify_security_events(output: &str) -> Evidence {
    let mut evidence = Evidence::Clear;
    for line in output.lines().map(str::to_ascii_lowercase) {
        let contains = |values: &[&str]| values.iter().any(|value| line.contains(value));
        if !contains(&[
            "com.openai.codex",
            "codex.app",
            "chatgpt.app",
            ".plugin-appserver",
            "codex-command-runner",
        ]) || contains(&["not blocked", "not denied"])
        {
            continue;
        }
        let event = if contains(&["100024", "emfile", "enfile", "too many open files"]) {
            Evidence::Exhausted
        } else if contains(&["would block", "would deny"]) {
            Evidence::Audit
        } else if contains(&[
            "xp_malware_detected",
            "xp_malware_remediated",
            "malware detected",
            "malware blocked",
            "malware removed",
            "remediat",
        ]) {
            Evidence::Malware
        } else if contains(&[
            "blocked",
            "denied",
            "rejected",
            "notarization failed",
            "signature invalid",
            "execution prevented",
            "unexpected xprotect assessment",
            "damaged and",
        ]) {
            Evidence::Blocked
        } else if contains(&["audit"]) {
            Evidence::Audit
        } else {
            Evidence::Clear
        };
        evidence = evidence.max(event);
    }
    evidence
}

fn enforcement_check(gatekeeper: Evidence, events: Evidence) -> DoctorCheck {
    let id = "desktop.security.enforcement";
    let (status, summary, remedy) = if events == Evidence::Malware {
        (
            CheckStatus::Fail,
            "macos XProtect blocked or remediated the desktop application",
            "collect the XProtect detection and ask your security administrator to review the official Codex installation",
        )
    } else if gatekeeper == Evidence::Blocked {
        (
            CheckStatus::Fail,
            "macos gatekeeper rejected the desktop application",
            "ask your security administrator to review the application policy",
        )
    } else if events == Evidence::Blocked {
        (
            CheckStatus::Fail,
            "a recent macos security event blocked the desktop application",
            "ask your security administrator to review the matching prevention event",
        )
    } else if events == Evidence::Exhausted {
        (
            CheckStatus::Warning,
            "macos system-policy diagnostics indicate file descriptor exhaustion",
            "restart your Mac, retry Codex once, and contact support if the problem returns",
        )
    } else if events == Evidence::Audit {
        (
            CheckStatus::Warning,
            "recent desktop security events are audit-only",
            "ask your security administrator to verify the application policy",
        )
    } else if gatekeeper == Evidence::Unavailable {
        (
            CheckStatus::Warning,
            "the desktop security assessment was unavailable",
            "check access to macos gatekeeper diagnostics",
        )
    } else if events == Evidence::Unavailable {
        (
            CheckStatus::Warning,
            "recent macos security enforcement history was unavailable",
            "check access to macos unified security logs and rerun codex doctor",
        )
    } else {
        return desktop_check(
            id,
            CheckStatus::Ok,
            "the desktop application passed available macos security assessments",
        )
        .detail("gatekeeper: accepted");
    };
    desktop_check(id, status, summary).remediation(remedy)
}

fn is_truncated(output: &Output) -> bool {
    output.stdout.len() > MAX_OUTPUT_BYTES || output.stderr.len() > MAX_OUTPUT_BYTES
}

async fn run_command(command: &mut Command) -> Option<Output> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let mut child = command.spawn().ok()?;
    let (stdout, stderr) = (child.stdout.take()?, child.stderr.take()?);
    timeout(PROBE_TIMEOUT, async {
        let (stdout, stderr, status) =
            tokio::join!(read_bounded(stdout), read_bounded(stderr), child.wait());
        Some(Output {
            status: status.ok()?,
            stdout: stdout.ok()?,
            stderr: stderr.ok()?,
        })
    })
    .await
    .ok()
    .flatten()
}

async fn read_bounded<R: AsyncRead + Unpin>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    (&mut reader)
        .take(MAX_OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .await?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        tokio::io::copy(&mut reader, &mut tokio::io::sink()).await?;
    }
    Ok(bytes)
}

#[cfg(test)]
#[path = "macos_security_tests.rs"]
mod tests;
