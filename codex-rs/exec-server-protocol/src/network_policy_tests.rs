use pretty_assertions::assert_eq;

use super::ExecServerNetworkPolicyDecision;
use super::ExecServerNetworkPolicyRequest;
use super::ExecServerNetworkProtocol;
use super::NetworkPolicyDecisionNotification;
use super::NetworkPolicyRequestParams;
use super::NetworkPolicyRequestResponse;
use crate::ProcessId;

#[test]
fn network_policy_request_uses_stable_json_shapes() {
    let request = NetworkPolicyRequestParams {
        process_id: ProcessId::from("process-1"),
        request: ExecServerNetworkPolicyRequest {
            protocol: ExecServerNetworkProtocol::HttpsConnect,
            host: "example.com".to_string(),
            port: 443,
        },
    };
    let request_json = serde_json::json!({
        "processId": "process-1",
        "request": {
            "protocol": "https_connect",
            "host": "example.com",
            "port": 443,
        },
    });
    assert_eq!(
        serde_json::to_value(&request).expect("serialize policy request"),
        request_json
    );
    let decoded_request: NetworkPolicyRequestParams =
        serde_json::from_value(request_json.clone()).expect("deserialize policy request");
    assert_eq!(
        serde_json::to_value(decoded_request).expect("reserialize policy request"),
        request_json
    );

    let decision = NetworkPolicyRequestResponse {
        decision: ExecServerNetworkPolicyDecision::Allow,
    };
    let decision_json = serde_json::json!({
        "decision": {"type": "allow"},
    });
    assert_eq!(
        serde_json::to_value(&decision).expect("serialize policy decision"),
        decision_json
    );
    assert_eq!(
        serde_json::from_value::<NetworkPolicyRequestResponse>(decision_json)
            .expect("deserialize policy decision"),
        decision
    );

    for (decision, decision_json) in [
        (
            ExecServerNetworkPolicyDecision::Deny {
                reason: "not_allowed".to_string(),
            },
            serde_json::json!({"type": "deny", "reason": "not_allowed"}),
        ),
        (
            ExecServerNetworkPolicyDecision::Ask {
                reason: "not_allowed".to_string(),
            },
            serde_json::json!({"type": "ask", "reason": "not_allowed"}),
        ),
    ] {
        let response = NetworkPolicyRequestResponse { decision };
        assert_eq!(
            serde_json::to_value(&response).expect("serialize policy decision"),
            serde_json::json!({"decision": decision_json})
        );
    }
}

#[test]
fn network_policy_decision_notification_uses_stable_json_shape() {
    let notification = NetworkPolicyDecisionNotification {
        process_id: ProcessId::from("process-1"),
        timestamp: "2026-08-11T12:34:56.789Z".to_string(),
        scope: "domain".to_string(),
        decision: "deny".to_string(),
        source: "baseline_policy".to_string(),
        reason: "not_allowed".to_string(),
        protocol: ExecServerNetworkProtocol::HttpsConnect,
        host: "example.com".to_string(),
        port: 443,
        method: Some("CONNECT".to_string()),
        client: Some("127.0.0.1".to_string()),
        policy_override: false,
    };
    let expected = serde_json::json!({
        "processId": "process-1",
        "timestamp": "2026-08-11T12:34:56.789Z",
        "scope": "domain",
        "decision": "deny",
        "source": "baseline_policy",
        "reason": "not_allowed",
        "protocol": "https_connect",
        "host": "example.com",
        "port": 443,
        "method": "CONNECT",
        "client": "127.0.0.1",
        "policyOverride": false,
    });

    assert_eq!(
        serde_json::to_value(&notification).expect("serialize policy decision notification"),
        expected
    );
    assert_eq!(
        serde_json::from_value::<NetworkPolicyDecisionNotification>(expected)
            .expect("deserialize policy decision notification"),
        notification
    );
}
