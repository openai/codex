use codex_network_proxy::BlockedRequest;
use codex_protocol::approvals::NetworkApprovalContext;
use codex_protocol::approvals::NetworkApprovalProtocol;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NetworkPolicyDecisionPayload {
    pub decision: String,
    pub source: String,
    pub protocol: Option<String>,
    pub host: Option<String>,
    pub reason: Option<String>,
    pub port: Option<u16>,
}

impl NetworkPolicyDecisionPayload {
    pub(crate) fn is_ask_from_decider(&self) -> bool {
        self.decision.eq_ignore_ascii_case("ask") && self.source.eq_ignore_ascii_case("decider")
    }
}

pub(crate) fn network_approval_context_from_payload(
    payload: &NetworkPolicyDecisionPayload,
) -> Option<NetworkApprovalContext> {
    if !payload.is_ask_from_decider() {
        return None;
    }

    let protocol = match payload.protocol.as_deref() {
        Some("http") => NetworkApprovalProtocol::Http,
        Some("https") | Some("https_connect") => NetworkApprovalProtocol::Https,
        _ => return None,
    };

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
    if !blocked
        .decision
        .as_deref()
        .is_some_and(|decision| decision.eq_ignore_ascii_case("deny"))
    {
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
            decision: "deny".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
            host: Some("example.com".to_string()),
            reason: Some("not_allowed".to_string()),
            port: Some(443),
        };

        assert_eq!(network_approval_context_from_payload(&payload), None);
    }

    #[test]
    fn network_approval_context_maps_http_and_https_protocols() {
        let http_payload = NetworkPolicyDecisionPayload {
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("http".to_string()),
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
            decision: "ask".to_string(),
            source: "decider".to_string(),
            protocol: Some("https_connect".to_string()),
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
    }

    #[test]
    fn denied_network_policy_message_requires_deny_decision() {
        let blocked = BlockedRequest {
            host: "google.com".to_string(),
            reason: "not_allowed".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            attempt_id: Some("attempt-1".to_string()),
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
            host: "google.com".to_string(),
            reason: "denied".to_string(),
            client: None,
            method: Some("GET".to_string()),
            mode: None,
            protocol: "http".to_string(),
            attempt_id: Some("attempt-1".to_string()),
            decision: Some("deny".to_string()),
            source: Some("baseline_policy".to_string()),
            port: Some(80),
            timestamp: 0,
        };
        assert_eq!(
            denied_network_policy_message(&blocked),
            Some(
                "Network access to \"google.com\" was blocked: domain is explicitly denied by policy and cannot be approved from this prompt.".to_string()
            )
        );
    }
}
