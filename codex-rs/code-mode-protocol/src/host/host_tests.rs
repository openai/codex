use pretty_assertions::assert_eq;
use serde_json::json;

use super::Capability;
use super::CapabilitySet;
use super::ClientHello;
use super::ClientToHost;
use super::DelegateRequest;
use super::DelegateResponse;
use super::HandshakeRejectReason;
use super::HostHello;
use super::HostRequest;
use super::HostResponse;
use super::HostToClient;
use super::ProtocolVersion;
use super::SessionId;
use super::SupportedProtocolVersions;
use super::WireResult;
use crate::CellId;

fn session_id() -> SessionId {
    SessionId::new("session-1").expect("valid session ID")
}

fn capability(value: &str) -> Capability {
    Capability::new(value).expect("valid capability")
}

fn supported_versions() -> SupportedProtocolVersions {
    SupportedProtocolVersions::try_new([ProtocolVersion::V1])
        .expect("nonempty unique protocol versions")
}

#[test]
fn handshake_wire_contract_is_explicit_and_round_trips() {
    let client_hello = ClientToHost::ClientHello(
        ClientHello::new(
            supported_versions(),
            CapabilitySet::try_new([capability("required")]).expect("valid required set"),
            CapabilitySet::try_new([capability("optional")]).expect("valid optional set"),
        )
        .expect("disjoint capabilities"),
    );
    let client_hello_json = json!({
        "type": "connection/hello",
        "supportedVersions": [1],
        "requiredCapabilities": ["required"],
        "optionalCapabilities": ["optional"],
    });
    assert_eq!(
        serde_json::to_value(&client_hello).expect("serialize"),
        client_hello_json
    );
    assert_eq!(
        serde_json::from_value::<ClientToHost>(client_hello_json).expect("deserialize"),
        client_hello
    );

    let host_hello = HostToClient::HostHello(HostHello::new(
        ProtocolVersion::V1,
        CapabilitySet::try_new([capability("required")]).expect("valid capabilities"),
    ));
    let host_hello_json = json!({
        "type": "connection/ready",
        "selectedVersion": 1,
        "capabilities": ["required"],
    });
    assert_eq!(
        serde_json::to_value(&host_hello).expect("serialize"),
        host_hello_json
    );
    assert_eq!(
        serde_json::from_value::<HostToClient>(host_hello_json).expect("deserialize"),
        host_hello
    );

    let rejected = HostToClient::HandshakeRejected {
        reason: HandshakeRejectReason::NoCompatibleVersion {
            supported_versions: supported_versions(),
        },
    };
    let rejected_json = json!({
        "type": "connection/rejected",
        "reason": {
            "type": "noCompatibleVersion",
            "supportedVersions": [1],
        },
    });
    assert_eq!(
        serde_json::to_value(&rejected).expect("serialize"),
        rejected_json
    );
    assert_eq!(
        serde_json::from_value::<HostToClient>(rejected_json).expect("deserialize"),
        rejected
    );
}

#[test]
fn session_lifecycle_wire_contract_is_explicit_and_round_trips() {
    let client_messages = [
        (
            ClientToHost::Request {
                id: 7,
                request: HostRequest::OpenSession {
                    session_id: session_id(),
                },
            },
            json!({
                "type": "operation/request",
                "id": 7,
                "request": {
                    "method": "session/open",
                    "sessionId": "session-1",
                },
            }),
        ),
        (
            ClientToHost::Request {
                id: 8,
                request: HostRequest::ShutdownSession {
                    session_id: session_id(),
                },
            },
            json!({
                "type": "operation/request",
                "id": 8,
                "request": {
                    "method": "session/shutdown",
                    "sessionId": "session-1",
                },
            }),
        ),
    ];
    for (message, encoded) in client_messages {
        assert_eq!(serde_json::to_value(&message).expect("serialize"), encoded);
        assert_eq!(
            serde_json::from_value::<ClientToHost>(encoded).expect("deserialize"),
            message
        );
    }

    let host_messages = [
        (
            HostToClient::Response {
                id: 7,
                result: WireResult::Ok {
                    value: HostResponse::SessionReady {
                        session_id: session_id(),
                    },
                },
            },
            json!({
                "type": "operation/response",
                "id": 7,
                "result": {
                    "status": "ok",
                    "value": {
                        "type": "session/ready",
                        "sessionId": "session-1",
                    },
                },
            }),
        ),
        (
            HostToClient::Response {
                id: 8,
                result: WireResult::Ok {
                    value: HostResponse::SessionClosed {
                        session_id: session_id(),
                    },
                },
            },
            json!({
                "type": "operation/response",
                "id": 8,
                "result": {
                    "status": "ok",
                    "value": {
                        "type": "session/closed",
                        "sessionId": "session-1",
                    },
                },
            }),
        ),
    ];
    for (message, encoded) in host_messages {
        assert_eq!(serde_json::to_value(&message).expect("serialize"), encoded);
        assert_eq!(
            serde_json::from_value::<HostToClient>(encoded).expect("deserialize"),
            message
        );
    }
}

#[test]
fn delegate_wire_contract_is_explicit_and_round_trips() {
    let request = HostToClient::DelegateRequest {
        id: 11,
        session_id: session_id(),
        request: DelegateRequest::Notify {
            call_id: "call-1".to_string(),
            cell_id: CellId::new("cell-1".to_string()),
            text: "hello".to_string(),
        },
    };
    let request_json = json!({
        "type": "delegate/request",
        "id": 11,
        "sessionId": "session-1",
        "request": {
            "type": "notification/send",
            "callId": "call-1",
            "cellId": "cell-1",
            "text": "hello",
        },
    });
    assert_eq!(
        serde_json::to_value(&request).expect("serialize"),
        request_json
    );
    assert_eq!(
        serde_json::from_value::<HostToClient>(request_json).expect("deserialize"),
        request
    );

    let response = ClientToHost::DelegateResponse {
        id: 11,
        result: WireResult::Ok {
            value: DelegateResponse::NotificationDelivered,
        },
    };
    let response_json = json!({
        "type": "delegate/response",
        "id": 11,
        "result": {
            "status": "ok",
            "value": { "type": "notification/delivered" },
        },
    });
    assert_eq!(
        serde_json::to_value(&response).expect("serialize"),
        response_json
    );
    assert_eq!(
        serde_json::from_value::<ClientToHost>(response_json).expect("deserialize"),
        response
    );
}

#[test]
fn invalid_protocol_states_cannot_be_constructed_or_decoded() {
    assert!(SessionId::new("").is_err());
    assert!(Capability::new("   ").is_err());
    assert!(ProtocolVersion::new(/*value*/ 0).is_none());
    assert!(SupportedProtocolVersions::try_new([]).is_err());
    assert!(
        SupportedProtocolVersions::try_new([ProtocolVersion::V1, ProtocolVersion::V1]).is_err()
    );
    assert!(CapabilitySet::try_new([capability("same"), capability("same")]).is_err());

    let version_two = ProtocolVersion::new(/*value*/ 2).expect("valid protocol version");
    let versions = SupportedProtocolVersions::try_new([ProtocolVersion::V1, version_two])
        .expect("valid versions");
    assert!(versions.contains(ProtocolVersion::V1));
    assert_eq!(
        versions.iter().collect::<Vec<_>>(),
        vec![ProtocolVersion::V1, version_two]
    );

    let overlapping = capability("overlapping");
    assert!(
        ClientHello::new(
            supported_versions(),
            CapabilitySet::try_new([overlapping.clone()]).expect("valid required set"),
            CapabilitySet::try_new([overlapping]).expect("valid optional set"),
        )
        .is_err()
    );

    for invalid in [
        json!({
            "type": "operation/request",
            "id": 1,
            "request": { "method": "session/open", "sessionId": "" },
        }),
        json!({
            "type": "connection/hello",
            "supportedVersions": [],
            "requiredCapabilities": [],
            "optionalCapabilities": [],
        }),
        json!({
            "type": "connection/hello",
            "supportedVersions": [1],
            "requiredCapabilities": ["overlapping"],
            "optionalCapabilities": ["overlapping"],
        }),
    ] {
        assert!(serde_json::from_value::<ClientToHost>(invalid).is_err());
    }
}

#[test]
fn unknown_fields_are_rejected() {
    assert!(
        serde_json::from_value::<ClientToHost>(json!({
            "type": "operation/request",
            "id": 1,
            "request": { "method": "session/open", "sessionId": "session-1" },
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<HostToClient>(json!({
            "type": "operation/response",
            "id": 1,
            "result": {
                "status": "ok",
                "value": { "type": "session/ready", "sessionId": "session-1" },
            },
            "unexpected": true,
        }))
        .is_err()
    );
}
