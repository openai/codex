use super::*;
use pretty_assertions::assert_eq;
use std::sync::Arc;

#[test]
fn inbound_fallback_records_only_variant_names() {
    let directory = tempfile::tempdir().expect("temporary log directory");
    let path = directory.path().join("session.jsonl");
    let logger = SessionLogger::new();
    logger.open(path.clone()).expect("open session log");
    for event in [
        AppEvent::CopySelection {
            text: Arc::from("private plan without parentheses"),
            label: "private label".to_string(),
        },
        AppEvent::CopySelection {
            text: Arc::from("private code(with parentheses)"),
            label: "private label".to_string(),
        },
        AppEvent::ConsolidateProposedPlan("private tuple payload".to_string()),
        AppEvent::SettingsSelectionClosed,
    ] {
        log_inbound_app_event_with(&logger, &event);
    }
    let records = std::fs::read_to_string(path)
        .expect("read session log")
        .lines()
        .map(|line| {
            let mut record: serde_json::Value = serde_json::from_str(line).expect("valid log JSON");
            record.as_object_mut().expect("log object").remove("ts");
            record
        })
        .collect::<Vec<_>>();
    assert_eq!(
        records,
        [
            "CopySelection",
            "CopySelection",
            "ConsolidateProposedPlan",
            "SettingsSelectionClosed"
        ]
        .map(|variant| json!({"dir": "to_tui", "kind": "app_event", "variant": variant}))
    );
}
