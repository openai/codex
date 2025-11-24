// cargo test --features "server client" --package rmcp test_logging
mod common;

use std::sync::{Arc, Mutex};

use common::handlers::{TestClientHandler, TestServer};
use rmcp::{
    ServiceExt,
    model::{LoggingLevel, LoggingMessageNotificationParam, SetLevelRequestParam},
};
use serde_json::json;
use tokio::sync::Notify;

#[tokio::test]
async fn test_logging_spec_compliance() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let receive_signal = Arc::new(Notify::new());
    let received_messages = Arc::new(Mutex::new(Vec::<LoggingMessageNotificationParam>::new()));

    // Start server in a separate task
    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;

        // Test server can send messages before level is set
        server
            .peer()
            .notify_logging_message(LoggingMessageNotificationParam {
                level: LoggingLevel::Info,
                data: serde_json::json!({
                    "message": "Server initiated message",
                    "timestamp": chrono::Utc::now().to_rfc3339(),
                }),
                logger: Some("test_server".to_string()),
            })
            .await?;

        server.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClientHandler::with_notification(
        true,
        true,
        receive_signal.clone(),
        received_messages.clone(),
    )
    .serve(client_transport)
    .await?;

    // Wait for the initial server message
    receive_signal.notified().await;
    {
        let mut messages = received_messages.lock().unwrap();
        assert_eq!(messages.len(), 1, "Should receive server-initiated message");
        messages.clear();
    }

    // Test level filtering and message format
    for level in [
        LoggingLevel::Emergency,
        LoggingLevel::Warning,
        LoggingLevel::Debug,
    ] {
        client
            .peer()
            .set_level(SetLevelRequestParam { level })
            .await?;

        // Wait for each message response
        receive_signal.notified().await;

        let mut messages = received_messages.lock().unwrap();
        let msg = messages.last().unwrap();

        // Verify required fields
        assert_eq!(msg.level, level);
        assert!(msg.data.is_object());

        // Verify data format
        let data = msg.data.as_object().unwrap();
        assert!(data.contains_key("message"));
        assert!(data.contains_key("timestamp"));

        // Verify timestamp
        let timestamp = data["timestamp"].as_str().unwrap();
        chrono::DateTime::parse_from_rfc3339(timestamp).expect("RFC3339 timestamp");

        messages.clear();
    }

    // Important: Cancel the client before ending the test
    client.cancel().await?;

    // Wait for server to complete
    server_handle.await??;

    Ok(())
}

#[tokio::test]
async fn test_logging_user_scenarios() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let receive_signal = Arc::new(Notify::new());
    let received_messages = Arc::new(Mutex::new(Vec::<LoggingMessageNotificationParam>::new()));

    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClientHandler::with_notification(
        true,
        true,
        receive_signal.clone(),
        received_messages.clone(),
    )
    .serve(client_transport)
    .await?;

    // Test 1: Error reporting scenario
    client
        .peer()
        .set_level(SetLevelRequestParam {
            level: LoggingLevel::Error,
        })
        .await?;
    receive_signal.notified().await; // Wait for response
    {
        let messages = received_messages.lock().unwrap();
        let msg = &messages[0];
        let data = msg.data.as_object().unwrap();
        assert!(
            data.contains_key("error_code"),
            "Error should have an error code"
        );
        assert!(
            data.contains_key("error_details"),
            "Error should have details"
        );
        assert!(
            data.contains_key("timestamp"),
            "Should know when error occurred"
        );
    }

    // Test 2: Debug scenario
    client
        .peer()
        .set_level(SetLevelRequestParam {
            level: LoggingLevel::Debug,
        })
        .await?;
    receive_signal.notified().await; // Wait for response
    {
        let messages = received_messages.lock().unwrap();
        let msg = messages.last().unwrap();
        let data = msg.data.as_object().unwrap();
        assert!(
            data.contains_key("function"),
            "Debug should show function name"
        );
        assert!(data.contains_key("line"), "Debug should show line number");
        assert!(
            data.contains_key("context"),
            "Debug should show execution context"
        );
    }

    // Test 3: Production monitoring scenario
    client
        .peer()
        .set_level(SetLevelRequestParam {
            level: LoggingLevel::Info,
        })
        .await?;
    receive_signal.notified().await; // Wait for response
    {
        let messages = received_messages.lock().unwrap();
        let msg = messages.last().unwrap();
        let data = msg.data.as_object().unwrap();
        assert!(data.contains_key("status"), "Should show system status");
        assert!(data.contains_key("metrics"), "Should include metrics");
    }

    // Important: Cancel client and wait for server before ending
    client.cancel().await?;
    server_handle.await??;

    Ok(())
}

#[test]
fn test_logging_level_serialization() {
    // Test all levels match spec exactly
    let test_cases = [
        (LoggingLevel::Alert, "alert"),
        (LoggingLevel::Critical, "critical"),
        (LoggingLevel::Debug, "debug"),
        (LoggingLevel::Emergency, "emergency"),
        (LoggingLevel::Error, "error"),
        (LoggingLevel::Info, "info"),
        (LoggingLevel::Notice, "notice"),
        (LoggingLevel::Warning, "warning"),
    ];

    for (level, expected) in test_cases {
        let serialized = serde_json::to_string(&level).unwrap();
        // Remove quotes from serialized string
        let serialized = serialized.trim_matches('"');
        assert_eq!(
            serialized, expected,
            "LoggingLevel::{:?} should serialize to \"{}\"",
            level, expected
        );
    }

    // Test deserialization from spec strings
    for (level, spec_string) in test_cases {
        let deserialized: LoggingLevel =
            serde_json::from_str(&format!("\"{}\"", spec_string)).unwrap();
        assert_eq!(
            deserialized, level,
            "\"{}\" should deserialize to LoggingLevel::{:?}",
            spec_string, level
        );
    }
}

#[tokio::test]
async fn test_logging_edge_cases() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let receive_signal = Arc::new(Notify::new());
    let received_messages = Arc::new(Mutex::new(Vec::<LoggingMessageNotificationParam>::new()));

    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;
        server.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClientHandler::with_notification(
        true,
        true,
        receive_signal.clone(),
        received_messages.clone(),
    )
    .serve(client_transport)
    .await?;

    // Test all logging levels from spec
    for level in [
        LoggingLevel::Alert,
        LoggingLevel::Critical,
        LoggingLevel::Notice, // These weren't tested before
    ] {
        client
            .peer()
            .set_level(SetLevelRequestParam { level })
            .await?;
        receive_signal.notified().await;

        let messages = received_messages.lock().unwrap();
        let msg = messages.last().unwrap();
        assert_eq!(msg.level, level);
    }

    client.cancel().await?;
    server_handle.await??;
    Ok(())
}

#[tokio::test]
async fn test_logging_optional_fields() -> anyhow::Result<()> {
    let (server_transport, client_transport) = tokio::io::duplex(4096);
    let receive_signal = Arc::new(Notify::new());
    let received_messages = Arc::new(Mutex::new(Vec::<LoggingMessageNotificationParam>::new()));

    let server_handle = tokio::spawn(async move {
        let server = TestServer::new().serve(server_transport).await?;

        // Test message with and without optional logger field
        for (level, has_logger) in [(LoggingLevel::Info, true), (LoggingLevel::Debug, false)] {
            server
                .peer()
                .notify_logging_message(LoggingMessageNotificationParam {
                    level,
                    data: json!({"test": "data"}),
                    logger: has_logger.then(|| "test_logger".to_string()),
                })
                .await?;
        }

        server.waiting().await?;
        anyhow::Ok(())
    });

    let client = TestClientHandler::with_notification(
        true,
        true,
        receive_signal.clone(),
        received_messages.clone(),
    )
    .serve(client_transport)
    .await?;

    // Wait for the initial server message
    receive_signal.notified().await;
    {
        let mut messages = received_messages.lock().unwrap();
        assert_eq!(messages.len(), 2, "Should receive two messages");
        messages.clear();
    }

    // Test level filtering and message format
    for level in [LoggingLevel::Info, LoggingLevel::Debug] {
        client
            .peer()
            .set_level(SetLevelRequestParam { level })
            .await?;

        // Wait for each message response
        receive_signal.notified().await;

        let mut messages = received_messages.lock().unwrap();
        let msg = messages.last().unwrap();

        // Verify required fields
        assert_eq!(msg.level, level);
        assert!(msg.data.is_object());

        // Verify data format
        let data = msg.data.as_object().unwrap();
        assert!(data.contains_key("message"));
        assert!(data.contains_key("timestamp"));

        // Verify timestamp
        let timestamp = data["timestamp"].as_str().unwrap();
        chrono::DateTime::parse_from_rfc3339(timestamp).expect("RFC3339 timestamp");

        messages.clear();
    }

    // Important: Cancel the client before ending the test
    client.cancel().await?;

    // Wait for server to complete
    server_handle.await??;

    Ok(())
}
