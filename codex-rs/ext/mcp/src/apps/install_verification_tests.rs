use super::*;
use crate::apps::test_support::connector_tool;
use crate::apps::test_support::gmail_tool;
use crate::apps::test_support::test_apps;

#[tokio::test]
async fn every_declared_app_must_materialize() {
    let declared = vec!["gmail".to_string(), "calendar".to_string()];
    let partial = test_apps(vec![gmail_tool("GmailSearch", /*destructive*/ false)]).await;
    assert!(!all_declared_apps_materialized(
        &declared,
        &partial.snapshot()
    ));

    let mut synthetic_calendar = connector_tool(
        "calendar",
        "Calendar",
        "CalendarList",
        /*destructive*/ false,
    );
    synthetic_calendar
        .meta
        .as_mut()
        .expect("connector metadata")
        .insert(
            "_codex_apps".to_string(),
            serde_json::json!({"synthetic_link": true}),
        );
    let complete = test_apps(vec![
        gmail_tool("GmailSearch", /*destructive*/ false),
        synthetic_calendar,
    ])
    .await;
    assert_eq!(complete.snapshot().apps().len(), 1);
    assert_eq!(complete.snapshot().all_connectors().len(), 2);
    assert!(all_declared_apps_materialized(
        &declared,
        &complete.snapshot()
    ));

    partial.shutdown().await;
    complete.shutdown().await;
}
