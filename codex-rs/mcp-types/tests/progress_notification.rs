#![expect(clippy::expect_used)]
use mcp_types::JSONRPCMessage;
use mcp_types::ProgressNotificationParams;
use mcp_types::ProgressToken;
use mcp_types::ServerNotification;

#[test]
fn deserialize_progress_notification() {
    let raw = r#"{
        "jsonrpc": "2.0",
        "method": "notifications/progress",
        "params": {
            "message": "Half way there",
            "progress": 0.5,
            "progressToken": 99,
            "total": 1.0
        }
    }"#;

    // Deserialize full JSONRPCMessage first.
    let msg: JSONRPCMessage = serde_json::from_str(raw).expect("invalid JSONRPCMessage");

    // Extract the notification variant.
    let JSONRPCMessage::Notification(notif) = msg else {
        unreachable!()
    };

    // Convert via generated TryFrom.
    let server_notif: ServerNotification =
        ServerNotification::try_from(notif).expect("conversion must succeed");

    let ServerNotification::ProgressNotification(params) = server_notif else {
        unreachable!()
    };

    let expected_params = ProgressNotificationParams {
        message: Some("Half way there".into()),
        progress: 0.5,
        progress_token: ProgressToken::Integer(99),
        total: Some(1.0),
    };

    assert_eq!(params, expected_params);
}
