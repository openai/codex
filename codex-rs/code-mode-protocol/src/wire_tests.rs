use std::io;

use pretty_assertions::assert_eq;
use serde_json::json;
use tokio::io::AsyncWriteExt;
use tokio::io::duplex;

use super::*;

#[test]
fn observe_request_has_a_stable_tagged_shape() {
    let message = ClientMessage::Request {
        id: 7,
        request: HostRequest::Observe {
            session_id: 3,
            cell_id: CellId::new("cell-9"),
            mode: ObserveMode::YieldAfter { duration_ms: 250 },
        },
    };

    assert_eq!(
        serde_json::to_value(message).unwrap(),
        json!({
            "type": "request",
            "id": 7,
            "request": {
                "method": "observe",
                "session_id": 3,
                "cell_id": "cell-9",
                "mode": {
                    "type": "yield_after",
                    "duration_ms": 250,
                },
            },
        })
    );
}

#[test]
fn cell_closed_notification_has_a_stable_tagged_shape() {
    let message = HostMessage::CellClosed {
        session_id: 3,
        cell_id: CellId::new("cell-9"),
    };

    assert_eq!(
        serde_json::to_value(message).unwrap(),
        json!({
            "type": "cell_closed",
            "session_id": 3,
            "cell_id": "cell-9",
        })
    );
}

#[test]
fn busy_observer_error_has_a_stable_tagged_shape() {
    let message = HostMessage::Response {
        id: 11,
        result: WireResult::Err {
            error: Error::BusyObserver {
                cell_id: CellId::new("cell-2"),
            },
        },
    };

    assert_eq!(
        serde_json::to_value(message).unwrap(),
        json!({
            "type": "response",
            "id": 11,
            "result": {
                "status": "err",
                "error": {
                    "code": "busy_observer",
                    "cell_id": "cell-2",
                },
            },
        })
    );
}

#[test]
fn callback_result_and_error_round_trip() {
    for response in [
        CallbackResponse::ToolResult {
            result: json!({"value": 42}),
        },
        CallbackResponse::ToolError {
            error_text: "tool failed".to_string(),
        },
        CallbackResponse::NotificationDelivered,
        CallbackResponse::NotificationError {
            error_text: "notify failed".to_string(),
        },
    ] {
        let message = ClientMessage::CallbackResponse { id: 19, response };
        let encoded = serde_json::to_vec(&message).unwrap();

        assert_eq!(
            serde_json::from_slice::<ClientMessage>(&encoded).unwrap(),
            message
        );
    }
}

#[tokio::test]
async fn frame_round_trip_preserves_message() {
    let (mut client, mut server) = duplex(1024);
    let message = ClientMessage::Request {
        id: 7,
        request: HostRequest::CreateSession,
    };

    write_frame(&mut client, &message).await.unwrap();

    assert_eq!(read_frame(&mut server).await.unwrap(), Some(message));
}

#[tokio::test]
async fn clean_eof_returns_none() {
    let (client, mut server) = duplex(16);
    drop(client);

    assert_eq!(
        read_frame::<_, ClientMessage>(&mut server).await.unwrap(),
        None
    );
}

#[tokio::test]
async fn partial_frame_header_is_rejected() {
    let (mut client, mut server) = duplex(16);
    client.write_all(&[0, 1]).await.unwrap();
    client.shutdown().await.unwrap();

    let error = read_frame::<_, ClientMessage>(&mut server)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
}

#[tokio::test]
async fn oversized_frame_is_rejected_before_allocation() {
    let (mut client, mut server) = duplex(16);
    let oversized_length = u32::try_from(MAX_FRAME_BYTES + 1).unwrap();
    client
        .write_all(&oversized_length.to_be_bytes())
        .await
        .unwrap();

    let error = read_frame::<_, ClientMessage>(&mut server)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::InvalidData);
}

#[tokio::test]
async fn malformed_json_frame_is_rejected() {
    let (mut client, mut server) = duplex(16);
    client.write_all(&1_u32.to_be_bytes()).await.unwrap();
    client.write_all(b"{").await.unwrap();

    let error = read_frame::<_, ClientMessage>(&mut server)
        .await
        .unwrap_err();

    assert_eq!(error.kind(), io::ErrorKind::Other);
}
