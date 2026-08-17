use pretty_assertions::assert_eq;

use super::CHANNELS;
use super::CheckStatus;
use super::MAX_RENDERED_EVENTS;
use super::classify;
use super::parse_events;

#[cfg(not(target_os = "windows"))]
#[tokio::test]
async fn an_unavailable_reader_is_not_reported_as_clean() {
    assert_eq!(super::collect().await.status, CheckStatus::Warning);
}

#[test]
fn every_source_distinguishes_audits_from_blocks() {
    for (channel, audit, block) in [
        (0, 1122, 1121),
        (0, 1125, 1126),
        (1, 8003, 8004),
        (2, 8021, 8022),
        (3, 8006, 8007),
        (4, 3076, 3077),
    ] {
        for (id, expected) in [(audit, CheckStatus::Warning), (block, CheckStatus::Fail)] {
            let events = parse_events(&fixture(id, "codex.exe", &[]), CHANNELS[channel]);
            assert_eq!(events[0].0, expected, "misclassified event {id}");
        }
    }
}

#[test]
fn defender_distinguishes_detection_and_remediation() {
    for (id, action, expected) in [
        (1116, "", CheckStatus::Warning),
        (1117, "Allow", CheckStatus::Ok),
        (1117, "Quarantine", CheckStatus::Fail),
        (1117, "Remove", CheckStatus::Fail),
    ] {
        let events = parse_events(
            &fixture(id, "codex.exe", &[("Action Name", action)]),
            CHANNELS[0],
        );
        assert_eq!(classify(&[Some(events)]).status, expected);
    }
}

#[test]
fn only_trusted_codex_executables_are_reported() {
    for (path, expected) in [
        ("codex.exe", true),
        (r"OpenAI.Codex_2p2nqsd0c76g0\ChatGPT.exe", true),
        ("evil-codex.exe", false),
        (r"C:\Other\ChatGPT.exe", false),
        (r"OpenAI.CodexEvil_1\ChatGPT.exe", false),
    ] {
        let events = parse_events(&fixture(/*id*/ 1121, path, &[]), CHANNELS[0]);
        assert_eq!(!events.is_empty(), expected, "misclassified {path}");
    }
}

#[test]
fn evidence_is_bounded_redacted_and_correctly_classified() {
    assert_eq!(classify(&[None]).status, CheckStatus::Warning);
    assert_eq!(classify(&[Some(Vec::new()), None]).status, CheckStatus::Ok);
    let audits = fixture(/*id*/ 1122, "codex.exe", &[]).repeat(MAX_RENDERED_EVENTS);
    let secret = "private-customer-secret";
    let block = fixture(/*id*/ 1121, "codex.exe", &[("User", secret)]);
    let events = parse_events(&format!("{audits}{block}"), CHANNELS[0]);
    let check = classify(&[Some(events)]);
    assert_eq!(check.id, "desktop.security.enforcement");
    assert_eq!(check.status, CheckStatus::Fail);
    assert_eq!(check.details.len(), MAX_RENDERED_EVENTS);
    assert!(!serde_json::to_string(&check).unwrap().contains(secret));
}

fn fixture(id: u32, path: &str, fields: &[(&str, &str)]) -> String {
    let data = fields
        .iter()
        .map(|(name, value)| format!("<Data Name=\"{name}\">{value}</Data>"))
        .collect::<String>();
    format!(
        "<Event><System><EventID>{id}</EventID><TimeCreated SystemTime=\"2026-01-01T00:00:00Z\"/></System><EventData><Data Name=\"Path\">{path}</Data>{data}</EventData></Event>"
    )
}
