use super::CheckStatus;
use super::Evidence;
use super::classify_gatekeeper;
use super::classify_security_events;
use super::enforcement_check;
use pretty_assertions::assert_eq;
use std::os::unix::process::ExitStatusExt;
use std::process::ExitStatus;
use std::process::Output;

#[test]
fn only_matching_enforced_apple_security_events_are_failures() {
    for (event, expected) in [
        ("denied com.openai.codex", Evidence::Blocked),
        (
            "denied /Applications/Codex.app/Contents/MacOS/Codex",
            Evidence::Blocked,
        ),
        ("malware detected ChatGPT.app", Evidence::Malware),
        ("audit token blocked ChatGPT.app", Evidence::Blocked),
        (
            "audit token XP_MALWARE_DETECTED ChatGPT.app",
            Evidence::Malware,
        ),
        ("XP_MALWARE_REMEDIATED ChatGPT.app", Evidence::Malware),
        ("denied .plugin-appserver", Evidence::Blocked),
        ("audit would block ChatGPT.app", Evidence::Audit),
        ("OSStatus 100024 ChatGPT.app", Evidence::Exhausted),
        ("not blocked ChatGPT.app", Evidence::Clear),
        ("denied EMFILE com.example.other", Evidence::Clear),
        ("EMFILE\nmalware detected ChatGPT.app", Evidence::Malware),
    ] {
        assert_eq!(classify_security_events(event), expected, "{event}");
    }
}

#[test]
fn gatekeeper_failures_require_actionable_security_evidence() {
    for (message, expected) in [
        ("ChatGPT.app: rejected", Evidence::Blocked),
        ("operation not permitted", Evidence::Unavailable),
    ] {
        let output = Output {
            status: ExitStatus::from_raw(1 << 8),
            stdout: Vec::new(),
            stderr: message.as_bytes().to_vec(),
        };
        assert_eq!(classify_gatekeeper(Some(&output)), expected);
    }
}

#[test]
fn exhaustion_and_unavailable_history_have_actionable_warnings() {
    for (events, remedy) in [
        (Evidence::Exhausted, "restart"),
        (Evidence::Unavailable, "unified security logs"),
    ] {
        let check = enforcement_check(Evidence::Clear, events);
        assert_eq!(check.status, CheckStatus::Warning);
        assert!(
            check
                .remediation
                .as_deref()
                .is_some_and(|value| value.contains(remedy))
        );
    }
}
