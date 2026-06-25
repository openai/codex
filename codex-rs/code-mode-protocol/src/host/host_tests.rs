use std::fmt::Debug;

use pretty_assertions::assert_eq;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use serde_json::json;

use super::Capability;
use super::CapabilitySet;
use super::ClientHello;
use super::ClientToHost;
use super::DelegateRequest;
use super::DelegateRequestId;
use super::DelegateResponse;
use super::HandshakeRejectReason;
use super::HostHello;
use super::HostRequest;
use super::HostResponse;
use super::HostToClient;
use super::ProtocolVersion;
use super::RequestId;
use super::SessionId;
use super::SupportedProtocolVersions;
use super::WireCellId;
use super::WireContentItem;
use super::WireExecuteRequest;
use super::WireImageDetail;
use super::WireNestedToolCall;
use super::WireResult;
use super::WireRuntimeResponse;
use super::WireToolDefinition;
use super::WireToolKind;
use super::WireToolName;
use super::WireWaitOutcome;
use super::WireWaitRequest;
use crate::ExecuteRequest;

fn session_id() -> SessionId {
    SessionId::new("session-1").expect("valid session ID")
}

fn cell_id(value: &str) -> WireCellId {
    WireCellId::new(value)
}

fn request_id(value: i64) -> RequestId {
    RequestId::new(value)
}

fn delegate_request_id(value: i64) -> DelegateRequestId {
    DelegateRequestId::new(value)
}

fn capability(value: &str) -> Capability {
    Capability::new(value).expect("valid capability")
}

fn supported_versions() -> SupportedProtocolVersions {
    SupportedProtocolVersions::try_new([ProtocolVersion::V1])
        .expect("nonempty unique protocol versions")
}

fn assert_wire_round_trip<T>(message: T, encoded: Value)
where
    T: Debug + DeserializeOwned + PartialEq + Serialize,
{
    assert_eq!(serde_json::to_value(&message).expect("serialize"), encoded);
    assert_eq!(
        serde_json::from_value::<T>(encoded).expect("deserialize"),
        message
    );
}

fn execute_request() -> WireExecuteRequest {
    WireExecuteRequest {
        tool_call_id: "call-1".to_string(),
        enabled_tools: vec![
            WireToolDefinition {
                name: "function_tool".to_string(),
                tool_name: WireToolName {
                    name: "function_tool".to_string(),
                    namespace: None,
                },
                description: "function tool".to_string(),
                kind: WireToolKind::Function,
                input_schema: Some(json!({ "type": "object" })),
                output_schema: None,
            },
            WireToolDefinition {
                name: "freeform_tool".to_string(),
                tool_name: WireToolName {
                    name: "freeform_tool".to_string(),
                    namespace: Some("mcp__sample__".to_string()),
                },
                description: "freeform tool".to_string(),
                kind: WireToolKind::Freeform,
                input_schema: None,
                output_schema: Some(json!({ "type": "string" })),
            },
        ],
        source: "text('hello');".to_string(),
        yield_time_ms: Some(25),
        max_output_tokens: Some(100),
    }
}

fn content_items() -> Vec<WireContentItem> {
    vec![
        WireContentItem::InputText {
            text: "hello".to_string(),
        },
        WireContentItem::InputImage {
            image_url: "data:image/png;base64,none".to_string(),
            detail: None,
        },
        WireContentItem::InputImage {
            image_url: "data:image/png;base64,auto".to_string(),
            detail: Some(WireImageDetail::Auto),
        },
        WireContentItem::InputImage {
            image_url: "data:image/png;base64,low".to_string(),
            detail: Some(WireImageDetail::Low),
        },
        WireContentItem::InputImage {
            image_url: "data:image/png;base64,high".to_string(),
            detail: Some(WireImageDetail::High),
        },
        WireContentItem::InputImage {
            image_url: "data:image/png;base64,original".to_string(),
            detail: Some(WireImageDetail::Original),
        },
    ]
}

fn content_items_json() -> Value {
    json!([
        { "type": "input_text", "text": "hello" },
        { "type": "input_image", "image_url": "data:image/png;base64,none" },
        {
            "type": "input_image",
            "image_url": "data:image/png;base64,auto",
            "detail": "auto",
        },
        {
            "type": "input_image",
            "image_url": "data:image/png;base64,low",
            "detail": "low",
        },
        {
            "type": "input_image",
            "image_url": "data:image/png;base64,high",
            "detail": "high",
        },
        {
            "type": "input_image",
            "image_url": "data:image/png;base64,original",
            "detail": "original",
        },
    ])
}

#[test]
fn handshake_v1_variants_are_pinned() {
    assert_wire_round_trip(
        ClientToHost::ClientHello(
            ClientHello::new(
                supported_versions(),
                CapabilitySet::try_new([capability("required")]).expect("valid required set"),
                CapabilitySet::try_new([capability("optional")]).expect("valid optional set"),
            )
            .expect("disjoint capabilities"),
        ),
        json!({
            "type": "connection/hello",
            "supportedVersions": [1],
            "requiredCapabilities": ["required"],
            "optionalCapabilities": ["optional"],
        }),
    );
    assert_wire_round_trip(
        HostToClient::HostHello(HostHello::new(
            ProtocolVersion::V1,
            CapabilitySet::try_new([capability("required")]).expect("valid capabilities"),
        )),
        json!({
            "type": "connection/ready",
            "selectedVersion": 1,
            "capabilities": ["required"],
        }),
    );
    for (reason, encoded) in [
        (
            HandshakeRejectReason::NoCompatibleVersion {
                supported_versions: supported_versions(),
            },
            json!({
                "type": "connection/rejected",
                "reason": {
                    "type": "noCompatibleVersion",
                    "supportedVersions": [1],
                },
            }),
        ),
        (
            HandshakeRejectReason::MissingRequiredCapability {
                capability: capability("required"),
            },
            json!({
                "type": "connection/rejected",
                "reason": {
                    "type": "missingRequiredCapability",
                    "capability": "required",
                },
            }),
        ),
        (
            HandshakeRejectReason::InvalidHello {
                message: "invalid hello".to_string(),
            },
            json!({
                "type": "connection/rejected",
                "reason": {
                    "type": "invalidHello",
                    "message": "invalid hello",
                },
            }),
        ),
    ] {
        assert_wire_round_trip(HostToClient::HandshakeRejected { reason }, encoded);
    }
}

#[test]
fn client_to_host_v1_variants_are_pinned() {
    let execute_request = execute_request();
    for (id, request, encoded_request) in [
        (
            request_id(/*value*/ 1),
            HostRequest::OpenSession {
                session_id: session_id(),
            },
            json!({ "method": "session/open", "sessionId": "session-1" }),
        ),
        (
            request_id(/*value*/ 2),
            HostRequest::Execute {
                session_id: session_id(),
                request: execute_request,
            },
            json!({
                "method": "session/execute",
                "sessionId": "session-1",
                "request": {
                    "tool_call_id": "call-1",
                    "enabled_tools": [
                        {
                            "name": "function_tool",
                            "tool_name": { "name": "function_tool", "namespace": null },
                            "description": "function tool",
                            "kind": "function",
                            "input_schema": { "type": "object" },
                            "output_schema": null,
                        },
                        {
                            "name": "freeform_tool",
                            "tool_name": {
                                "name": "freeform_tool",
                                "namespace": "mcp__sample__",
                            },
                            "description": "freeform tool",
                            "kind": "freeform",
                            "input_schema": null,
                            "output_schema": { "type": "string" },
                        },
                    ],
                    "source": "text('hello');",
                    "yield_time_ms": 25,
                    "max_output_tokens": 100,
                },
            }),
        ),
        (
            request_id(/*value*/ 3),
            HostRequest::Wait {
                session_id: session_id(),
                request: WireWaitRequest {
                    cell_id: cell_id("cell-1"),
                    yield_time_ms: 50,
                },
            },
            json!({
                "method": "session/wait",
                "sessionId": "session-1",
                "request": { "cell_id": "cell-1", "yield_time_ms": 50 },
            }),
        ),
        (
            request_id(/*value*/ 4),
            HostRequest::Terminate {
                session_id: session_id(),
                cell_id: cell_id("cell-1"),
            },
            json!({
                "method": "session/terminate",
                "sessionId": "session-1",
                "cellId": "cell-1",
            }),
        ),
        (
            request_id(/*value*/ 5),
            HostRequest::ShutdownSession {
                session_id: session_id(),
            },
            json!({ "method": "session/shutdown", "sessionId": "session-1" }),
        ),
    ] {
        assert_wire_round_trip(
            ClientToHost::Request { id, request },
            json!({
                "type": "operation/request",
                "id": id,
                "request": encoded_request,
            }),
        );
    }

    for (id, result, encoded_result) in [
        (
            delegate_request_id(/*value*/ 6),
            WireResult::Ok {
                value: DelegateResponse::ToolResult {
                    result: json!({ "answer": 42 }),
                },
            },
            json!({
                "status": "ok",
                "value": { "type": "tool/result", "result": { "answer": 42 } },
            }),
        ),
        (
            delegate_request_id(/*value*/ 7),
            WireResult::Ok {
                value: DelegateResponse::NotificationDelivered,
            },
            json!({
                "status": "ok",
                "value": { "type": "notification/delivered" },
            }),
        ),
        (
            delegate_request_id(/*value*/ 8),
            WireResult::Err {
                message: "delegate failed".to_string(),
            },
            json!({ "status": "error", "message": "delegate failed" }),
        ),
    ] {
        assert_wire_round_trip(
            ClientToHost::DelegateResponse { id, result },
            json!({
                "type": "delegate/response",
                "id": id,
                "result": encoded_result,
            }),
        );
    }
}

#[test]
fn host_to_client_v1_variants_are_pinned() {
    for (id, response, encoded_response) in [
        (
            request_id(/*value*/ 1),
            HostResponse::SessionReady {
                session_id: session_id(),
            },
            json!({ "type": "session/ready", "sessionId": "session-1" }),
        ),
        (
            request_id(/*value*/ 2),
            HostResponse::ExecutionStarted {
                cell_id: cell_id("cell-1"),
            },
            json!({ "type": "execution/started", "cellId": "cell-1" }),
        ),
        (
            request_id(/*value*/ 3),
            HostResponse::WaitCompleted {
                outcome: WireWaitOutcome::LiveCell(WireRuntimeResponse::Yielded {
                    cell_id: cell_id("cell-1"),
                    content_items: content_items(),
                }),
            },
            json!({
                "type": "wait/completed",
                "outcome": {
                    "LiveCell": {
                        "Yielded": {
                            "cell_id": "cell-1",
                            "content_items": content_items_json(),
                        },
                    },
                },
            }),
        ),
        (
            request_id(/*value*/ 4),
            HostResponse::WaitCompleted {
                outcome: WireWaitOutcome::MissingCell(WireRuntimeResponse::Result {
                    cell_id: cell_id("missing-cell"),
                    content_items: Vec::new(),
                    error_text: Some("cell not found".to_string()),
                }),
            },
            json!({
                "type": "wait/completed",
                "outcome": {
                    "MissingCell": {
                        "Result": {
                            "cell_id": "missing-cell",
                            "content_items": [],
                            "error_text": "cell not found",
                        },
                    },
                },
            }),
        ),
        (
            request_id(/*value*/ 5),
            HostResponse::SessionClosed {
                session_id: session_id(),
            },
            json!({ "type": "session/closed", "sessionId": "session-1" }),
        ),
    ] {
        assert_wire_round_trip(
            HostToClient::Response {
                id,
                result: WireResult::Ok { value: response },
            },
            json!({
                "type": "operation/response",
                "id": id,
                "result": { "status": "ok", "value": encoded_response },
            }),
        );
    }
    assert_wire_round_trip(
        HostToClient::Response {
            id: request_id(/*value*/ 6),
            result: WireResult::Err {
                message: "operation failed".to_string(),
            },
        },
        json!({
            "type": "operation/response",
            "id": 6,
            "result": { "status": "error", "message": "operation failed" },
        }),
    );

    assert_wire_round_trip(
        HostToClient::InitialResponse {
            id: request_id(/*value*/ 7),
            result: WireResult::Ok {
                value: WireRuntimeResponse::Terminated {
                    cell_id: cell_id("cell-1"),
                    content_items: Vec::new(),
                },
            },
        },
        json!({
            "type": "execute/initialResponse",
            "id": 7,
            "result": {
                "status": "ok",
                "value": {
                    "Terminated": { "cell_id": "cell-1", "content_items": [] },
                },
            },
        }),
    );
    assert_wire_round_trip(
        HostToClient::InitialResponse {
            id: request_id(/*value*/ 8),
            result: WireResult::Err {
                message: "execution failed".to_string(),
            },
        },
        json!({
            "type": "execute/initialResponse",
            "id": 8,
            "result": { "status": "error", "message": "execution failed" },
        }),
    );

    assert_wire_round_trip(
        HostToClient::DelegateRequest {
            id: delegate_request_id(/*value*/ 9),
            session_id: session_id(),
            request: DelegateRequest::InvokeTool {
                invocation: WireNestedToolCall {
                    cell_id: cell_id("cell-1"),
                    runtime_tool_call_id: "runtime-call-1".to_string(),
                    tool_name: WireToolName {
                        name: "freeform_tool".to_string(),
                        namespace: Some("mcp__sample__".to_string()),
                    },
                    tool_kind: WireToolKind::Freeform,
                    input: Some(json!({ "value": 1 })),
                },
            },
        },
        json!({
            "type": "delegate/request",
            "id": 9,
            "sessionId": "session-1",
            "request": {
                "type": "tool/invoke",
                "invocation": {
                    "cell_id": "cell-1",
                    "runtime_tool_call_id": "runtime-call-1",
                    "tool_name": {
                        "name": "freeform_tool",
                        "namespace": "mcp__sample__",
                    },
                    "tool_kind": "freeform",
                    "input": { "value": 1 },
                },
            },
        }),
    );
    assert_wire_round_trip(
        HostToClient::DelegateRequest {
            id: delegate_request_id(/*value*/ 10),
            session_id: session_id(),
            request: DelegateRequest::Notify {
                call_id: "call-1".to_string(),
                cell_id: cell_id("cell-1"),
                text: "important".to_string(),
            },
        },
        json!({
            "type": "delegate/request",
            "id": 10,
            "sessionId": "session-1",
            "request": {
                "type": "notification/send",
                "callId": "call-1",
                "cellId": "cell-1",
                "text": "important",
            },
        }),
    );
    assert_wire_round_trip(
        HostToClient::CancelDelegateRequest {
            id: delegate_request_id(/*value*/ 11),
        },
        json!({ "type": "delegate/cancel", "id": 11 }),
    );
    assert_wire_round_trip(
        HostToClient::CellClosed {
            session_id: session_id(),
            cell_id: cell_id("cell-1"),
        },
        json!({
            "type": "cell/closed",
            "sessionId": "session-1",
            "cellId": "cell-1",
        }),
    );
}

#[test]
fn execute_request_integer_bounds_are_enforced() {
    let wire_request = execute_request();
    let domain_request = ExecuteRequest::try_from(wire_request.clone())
        .expect("valid wire request converts to the domain");
    assert_eq!(
        WireExecuteRequest::try_from(domain_request.clone())
            .expect("valid domain request converts to the wire"),
        wire_request
    );

    let too_large = ExecuteRequest {
        max_output_tokens: Some(usize::try_from(i32::MAX).expect("i32::MAX fits usize") + 1),
        ..domain_request
    };
    assert!(WireExecuteRequest::try_from(too_large).is_err());

    let negative = WireExecuteRequest {
        max_output_tokens: Some(-1),
        ..wire_request
    };
    assert!(ExecuteRequest::try_from(negative).is_err());
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
fn every_nested_v1_object_rejects_unknown_fields() {
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
        serde_json::from_value::<HostRequest>(json!({
            "method": "session/open",
            "sessionId": "session-1",
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireExecuteRequest>(json!({
            "tool_call_id": "call-1",
            "enabled_tools": [],
            "source": "text('hello');",
            "yield_time_ms": null,
            "max_output_tokens": null,
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireToolDefinition>(json!({
            "name": "tool",
            "tool_name": { "name": "tool", "namespace": null },
            "description": "tool",
            "kind": "function",
            "input_schema": null,
            "output_schema": null,
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireToolName>(json!({
            "name": "tool",
            "namespace": null,
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireWaitRequest>(json!({
            "cell_id": "cell-1",
            "yield_time_ms": 50,
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireRuntimeResponse>(json!({
            "Yielded": {
                "cell_id": "cell-1",
                "content_items": [],
                "unexpected": true,
            },
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireContentItem>(json!({
            "type": "input_text",
            "text": "hello",
            "unexpected": true,
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<WireNestedToolCall>(json!({
            "cell_id": "cell-1",
            "runtime_tool_call_id": "runtime-call-1",
            "tool_name": { "name": "tool", "namespace": null },
            "tool_kind": "function",
            "input": null,
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
