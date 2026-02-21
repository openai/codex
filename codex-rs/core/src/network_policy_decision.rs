use codex_network_proxy::BlockedRequest;
use codex_network_proxy::NetworkDecisionSource;
use codex_network_proxy::NetworkPolicyDecision;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecisionPayload {
    pub decision: NetworkPolicyDecision,
    pub source: NetworkDecisionSource,
    #[serde(default)]
    pub protocol: Option<NetworkApprovalProtocol>,
    pub host: Option<String>,
    pub reason: Option<String>,
    pub port: Option<u16>,
}

impl NetworkPolicyDecisionPayload {
    pub(crate) fn is_ask_from_decider(&self) -> bool {
        self.decision == NetworkPolicyDecision::Ask && self.source == NetworkDecisionSource::Decider
    }
}

fn parse_network_policy_decision(value: &str) -> Option<NetworkPolicyDecision> {
    match value {
        "deny" => Some(NetworkPolicyDecision::Deny),
        "ask" => Some(NetworkPolicyDecision::Ask),
        _ => None,
    }
}

pub(crate) fn network_approval_context_from_payload(
    payload: &NetworkPolicyDecisionPayload,
) -> Option<NetworkApprovalContext> {
    if !payload.is_ask_from_decider() {
        return None;
    }

    let protocol = payload.protocol?;

    let host = payload.host.as_deref()?.trim();
    if host.is_empty() {
        return None;
    }

    Some(NetworkApprovalContext {
        host: host.to_string(),
        protocol,
    })
}

pub(crate) fn denied_network_policy_message(blocked: &BlockedRequest) -> Option<String> {
    let decision = blocked
        .decision
        .as_deref()
        .and_then(parse_network_policy_decision);
    if decision != Some(NetworkPolicyDecision::Deny) {
        return None;
    }

    let host = blocked.host.trim();
    if host.is_empty() {
        return Some("Network access was blocked by policy.".to_string());
    }

    let detail = match blocked.reason.as_str() {
        "denied" => "domain is explicitly denied by policy and cannot be approved from this prompt",
        "not_allowed" => "domain is not on the allowlist for the current sandbox mode",
        "not_allowed_local" => "local/private network addresses are blocked by policy",
        "method_not_allowed" => "request method is blocked by the current network mode",
        "proxy_disabled" => "managed network proxy is disabled",
        _ => "request is blocked by network policy",
    };

    Some(format!(
        "Network access to \"{host}\" was blocked: {detail}."
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use codex_network_proxy::BlockedRequest;
    use pretty_assertions::assert_eq;

    #[test]
    fn network_approval_context_requires_ask_from_decider() {
        let payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Deny,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Https),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };

        assert_eq!(network_approval_context_from_payload(&payload), None);
    }

    #[test]
    fn network_approval_context_maps_http_https_and_socks_protocols() {
        let http_payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Ask,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Http),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(80),
        };
        assert_eq!(
            network_approval_context_from_payload(&http_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Http,
            })
        );

        let https_payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Ask,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Https),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };
        assert_eq!(
            network_approval_context_from_payload(&https_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            })
        );

        let http_connect_payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Ask,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Https),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };
        assert_eq!(
            network_approval_context_from_payload(&http_connect_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Https,
            })
        );

        let socks5_tcp_payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Ask,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Socks5Tcp),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };
        assert_eq!(
            network_approval_context_from_payload(&socks5_tcp_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Socks5Tcp,
            })
        );

        let socks5_udp_payload = NetworkPolicyDecisionPayload {
            decision: NetworkPolicyDecision::Ask,
            source: NetworkDecisionSource::Decider,
            protocol: Some(NetworkApprovalProtocol::Socks5Udp),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };
        assert_eq!(
            network_approval_context_from_payload(&socks5_udp_payload),
            Some(NetworkApprovalContext {
                host: "example.com".to_string(),
                protocol: NetworkApprovalProtocol::Socks5Udp,
            })
        );
    }

    #[test]
    fn network_policy_decision_payload_deserializes_proxy_protocol_aliases() {
        let payload: NetworkPolicyDecisionPayload = serde_json::from_str(
            r#"{
                "decision":"ask",
                "source":"decider",
                "protocol":"https_connect",
                "host":"example.com",
                "reason":"not_allowed",
                "port":443
            }"#,
        )
        .expect("payload should deserialize");
        assert_eq!(payload.protocol, Some(NetworkApprovalProtocol::Https));

        let payload: NetworkPolicyDecisionPayload = serde_json::from_str(
            r#"{
                "decision":"ask",
                "source":"decider",
                "protocol":"http-connect",
                "host":"example.com",
                "reason":"not_allowed",
                "port":443
            }"#,
        )
        .expect("payload should deserialize");
        assert_eq!(payload.protocol, Some(NetworkApprovalProtocol::Https));
    }

    #[test]
    fn denied_network_policy_message_requires_deny_decision() {
        let blocked = BlockedRequest {
            host: "example.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            decision: Some("ask".to_string()),
            source: Some("decider".to_string()),
            port: Some(80),
            timestamp: 0,
        };
        assert_eq!(denied_network_policy_message(&blocked), None);
    }

    #[test]
    fn denied_network_policy_message_for_denylist_block_is_explicit() {
        let blocked = BlockedRequest {
            host: "example.com".to_string(),
            reason: "denied".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            decision: Some("deny".to_string()),
            source: Some("baseline_policy".to_string()),
            port: Some(80),
            timestamp: 0,
        };
        assert_eq!(
            denied_network_policy_message(&blocked),
            Some(
                "Network access to \"example.com\" was blocked: domain is explicitly denied by policy and cannot be approved from this prompt.".to_string()
            )
        );
    }
}
