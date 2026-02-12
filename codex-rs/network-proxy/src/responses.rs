use crate::network_policy::NetworkDecisionSource;
use crate::network_policy::NetworkPolicyDecision;
use crate::network_policy::NetworkProtocol;
use crate::reasons::REASON_DENIED;
use crate::reasons::REASON_METHOD_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED;
use crate::reasons::REASON_NOT_ALLOWED_LOCAL;
use rama_http::Body;
use rama_http::Response;
use rama_http::StatusCode;
use serde::Serialize;
use tracing::error;

const NETWORK_POLICY_DECISION_PREFIX: &str = "CODEX_NETWORK_POLICY_DECISION";

pub struct PolicyDecisionDetails<'a> {
    pub decision: NetworkPolicyDecision,
    pub reason: &'a str,
    pub source: NetworkDecisionSource,
    pub protocol: NetworkProtocol,
    pub host: &'a str,
    pub port: u16,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyDecisionPayload<'a> {
    decision: &'a str,
    reason: &'a str,
    source: &'a str,
    protocol: &'a str,
    host: &'a str,
    port: u16,
}

pub fn text_response(status: StatusCode, body: &str) -> Response {
    Response::builder()
        .status(status)
        .header("content-type", "text/plain")
        .body(Body::from(body.to_string()))
        .unwrap_or_else(|_| Response::new(Body::from(body.to_string())))
}

pub fn json_response<T: Serialize>(value: &T) -> Response {
    let body = match serde_json::to_string(value) {
        Ok(body) => body,
        Err(err) => {
            error!("failed to serialize JSON response: {err}");
            "{}".to_string()
        }
    };
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from(body))
        .unwrap_or_else(|err| {
            error!("failed to build JSON response: {err}");
            Response::new(Body::from("{}"))
        })
}

pub fn blocked_header_value(reason: &str) -> &'static str {
    match reason {
        REASON_NOT_ALLOWED | REASON_NOT_ALLOWED_LOCAL => "blocked-by-allowlist",
        REASON_DENIED => "blocked-by-denylist",
        REASON_METHOD_NOT_ALLOWED => "blocked-by-method-policy",
        _ => "blocked-by-policy",
    }
}

pub fn blocked_message(reason: &str) -> &'static str {
    match reason {
        REASON_NOT_ALLOWED => "Codex blocked this request: domain not in allowlist.",
        REASON_NOT_ALLOWED_LOCAL => {
            "Codex blocked this request: local/private addresses not allowed."
        }
        REASON_DENIED => "Codex blocked this request: domain denied by policy.",
        REASON_METHOD_NOT_ALLOWED => {
            "Codex blocked this request: method not allowed in limited mode."
        }
        _ => "Codex blocked this request by network policy.",
    }
}

pub fn policy_decision_prefix(details: &PolicyDecisionDetails<'_>) -> String {
    let payload = PolicyDecisionPayload {
        decision: details.decision.as_str(),
        reason: details.reason,
        source: details.source.as_str(),
        protocol: details.protocol.as_policy_protocol(),
        host: details.host,
        port: details.port,
    };
    let payload_json = match serde_json::to_string(&payload) {
        Ok(json) => json,
        Err(err) => {
            error!("failed to serialize policy decision payload: {err}");
            "{}".to_string()
        }
    };
    format!("{NETWORK_POLICY_DECISION_PREFIX} {payload_json}")
}

pub fn blocked_message_with_policy(reason: &str, details: &PolicyDecisionDetails<'_>) -> String {
    format!(
        "{}\n{}",
        policy_decision_prefix(details),
        blocked_message(reason)
    )
}

pub fn blocked_text_response_with_policy(
    reason: &str,
    details: &PolicyDecisionDetails<'_>,
) -> Response {
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "text/plain")
        .header("x-proxy-error", blocked_header_value(reason))
        .body(Body::from(blocked_message_with_policy(reason, details)))
        .unwrap_or_else(|_| Response::new(Body::from("blocked")))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reasons::REASON_NOT_ALLOWED;
    use pretty_assertions::assert_eq;

    #[test]
    fn policy_decision_prefix_serializes_expected_payload() {
        let details = PolicyDecisionDetails {
            decision: NetworkPolicyDecision::Ask,
            reason: REASON_NOT_ALLOWED,
            source: NetworkDecisionSource::Decider,
            protocol: NetworkProtocol::HttpsConnect,
            host: "api.example.com",
            port: 443,
        };

        let line = policy_decision_prefix(&details);
        assert_eq!(
            line,
            r#"CODEX_NETWORK_POLICY_DECISION {"decision":"ask","reason":"not_allowed","source":"decider","protocol":"https_connect","host":"api.example.com","port":443}"#
        );
    }

    #[test]
    fn blocked_message_with_policy_includes_prefix_and_human_message() {
        let details = PolicyDecisionDetails {
            decision: NetworkPolicyDecision::Deny,
            reason: REASON_NOT_ALLOWED,
            source: NetworkDecisionSource::BaselinePolicy,
            protocol: NetworkProtocol::Http,
            host: "api.example.com",
            port: 80,
        };

        let message = blocked_message_with_policy(REASON_NOT_ALLOWED, &details);
        assert_eq!(
            message,
            r#"CODEX_NETWORK_POLICY_DECISION {"decision":"deny","reason":"not_allowed","source":"baseline_policy","protocol":"http","host":"api.example.com","port":80}
Codex blocked this request: domain not in allowlist."#
        );
    }
}
